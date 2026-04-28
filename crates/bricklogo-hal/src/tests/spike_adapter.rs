use super::*;
use crate::adapter::{PortCommand, PortDirection};
use crate::scheduler;
use rust_spike::atlantis;
use rust_spike::cobs;
use rust_spike::protocol::{self, Event, Reply};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Mock Atlantis transport. Records incoming tunnel payloads (already
/// unwrapped from TunnelMessage framing) and auto-replies with plausible
/// agent responses.
struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

struct MockState {
    /// Raw bytes written by the adapter (exactly as sent to the wire).
    writes: Vec<Vec<u8>>,
    /// Decoded tunnel command payloads (opcode + args).
    commands: Vec<Vec<u8>>,
    /// Raw bytes queued for the adapter to read.
    outgoing: VecDeque<u8>,
    /// If set, the mock auto-generates agent replies for tunnel commands.
    auto_reply: bool,
    /// Optional override returning a specific reply for `read` ops.
    read_override: Option<Reply>,
    /// Accumulates partial frames from the adapter side.
    frame_buf: Vec<u8>,
}

impl SpikeTransport for MockTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        let mut state = self.state.lock().unwrap();
        if state.outgoing.is_empty() {
            return Ok(0);
        }
        let n = buf.len().min(state.outgoing.len());
        for slot in buf.iter_mut().take(n) {
            *slot = state.outgoing.pop_front().unwrap();
        }
        Ok(n)
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        state.writes.push(data.to_vec());
        state.frame_buf.extend_from_slice(data);

        loop {
            let pos = match state.frame_buf.iter().position(|&b| b == cobs::END_FRAME) {
                Some(p) => p,
                None => break,
            };
            let frame: Vec<u8> = state.frame_buf.drain(..=pos).collect();
            let body = match cobs::unpack(&frame[..frame.len() - 1]) {
                Ok(b) => b,
                Err(_) => continue,
            };
            // Host→hub TunnelMessages: [0x32, size_u16, payload].
            if body.first() == Some(&atlantis::ID_TUNNEL_MESSAGE) && body.len() >= 3 {
                let size = u16::from_le_bytes([body[1], body[2]]) as usize;
                let end = (3 + size).min(body.len());
                let payload = body[3..end].to_vec();
                handle_command(&mut state, &payload);
            }
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn handle_command(state: &mut MockState, payload: &[u8]) {
    if payload.len() < 3 {
        return;
    }
    state.commands.push(payload.to_vec());
    if !state.auto_reply {
        return;
    }
    let op = payload[0];
    let rid = u16::from_le_bytes([payload[1], payload[2]]);
    let reply: Reply = match op {
        protocol::OP_READ | protocol::OP_READ_HUB => state
            .read_override
            .clone()
            .unwrap_or(Reply::Int(0)),
        _ => Reply::Ok,
    };
    enqueue_reply(state, rid, reply);
}

fn enqueue_reply(state: &mut MockState, rid: u16, reply: Reply) {
    let mut payload = Vec::with_capacity(32);
    match reply {
        Reply::Ok => {
            payload.push(protocol::REPLY_OK);
            payload.extend_from_slice(&rid.to_le_bytes());
        }
        Reply::Int(v) => {
            payload.push(protocol::REPLY_INT);
            payload.extend_from_slice(&rid.to_le_bytes());
            payload.extend_from_slice(&v.to_le_bytes());
        }
        Reply::List(values) => {
            payload.push(protocol::REPLY_LIST);
            payload.extend_from_slice(&rid.to_le_bytes());
            payload.push(values.len() as u8);
            for v in values {
                payload.extend_from_slice(&v.to_le_bytes());
            }
        }
        Reply::Bool(b) => {
            payload.push(protocol::REPLY_BOOL);
            payload.extend_from_slice(&rid.to_le_bytes());
            payload.push(if b { 1 } else { 0 });
        }
        Reply::Error(msg) => {
            payload.push(protocol::REPLY_ERROR);
            payload.extend_from_slice(&rid.to_le_bytes());
            let bytes = msg.as_bytes();
            let len = bytes.len().min(255);
            payload.push(len as u8);
            payload.extend_from_slice(&bytes[..len]);
        }
        Reply::TypeList(values) => {
            payload.push(protocol::REPLY_TYPE_LIST);
            payload.extend_from_slice(&rid.to_le_bytes());
            payload.push(values.len() as u8);
            for v in values {
                payload.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    // Wrap in plain TunnelMessage then COBS-frame.
    let mut msg = Vec::with_capacity(3 + payload.len());
    msg.push(atlantis::ID_TUNNEL_MESSAGE);
    msg.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    msg.extend_from_slice(&payload);
    let framed = cobs::pack(&msg);
    state.outgoing.extend(framed);
}

fn make_adapter() -> (SpikeAdapter, Arc<Mutex<MockState>>) {
    make_adapter_with_override(None)
}

fn make_adapter_with_override(
    read_override: Option<Reply>,
) -> (SpikeAdapter, Arc<Mutex<MockState>>) {
    let state = Arc::new(Mutex::new(MockState {
        writes: Vec::new(),
        commands: Vec::new(),
        outgoing: VecDeque::new(),
        auto_reply: true,
        read_override,
        frame_buf: Vec::new(),
    }));
    let transport: Box<dyn SpikeTransport> = Box::new(MockTransport { state: state.clone() });
    let (tx, rx) = std::sync::mpsc::channel();
    let alive_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    // Seed all ports with a tacho motor type so existing tests get the
    // motor.run / motor.stop dispatch path. Tests that need a different
    // device type override via `make_adapter_with_types`.
    let port_types = Arc::new(Mutex::new(
        [rust_spike::constants::DEVICE_LARGE_ANGULAR_MOTOR;
            rust_spike::constants::PORT_COUNT],
    ));
    let slot = SpikeSlot {
        transport,
        rx,
        framer: FrameReader::new(),
        pending: HashMap::new(),
        alive: true,
        alive_flag: alive_flag.clone(),
        last_heartbeat: Instant::now(),
        port_types: port_types.clone(),
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = SpikeAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    adapter.alive = alive_flag;
    adapter.port_types = port_types;
    (adapter, state)
}

/// Variant of `make_adapter_with_override` that pre-seeds the port-type
/// cache so `validate_output_port` sees specific devices on specific ports.
fn make_adapter_with_types(types: [u16; 6]) -> (SpikeAdapter, Arc<Mutex<MockState>>) {
    let (adapter, state) = make_adapter();
    *adapter.port_types.lock().unwrap() = types;
    (adapter, state)
}

// ── Helpers for inspecting binary commands ──────

fn parse_rid(cmd: &[u8]) -> u16 {
    u16::from_le_bytes([cmd[1], cmd[2]])
}

fn parse_op(cmd: &[u8]) -> u8 {
    cmd[0]
}

#[test]
fn test_start_port_sends_motor_run() {
    let (mut adapter, state) = make_adapter();
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_MOTOR_RUN);
    assert_eq!(cmd[3], 0); // port a
    assert_eq!(i16::from_le_bytes([cmd[4], cmd[5]]), 500);
}

#[test]
fn test_stop_port_sends_motor_stop() {
    let (mut adapter, state) = make_adapter();
    adapter.stop_port("b").unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_MOTOR_STOP);
    assert_eq!(cmd[3], 1); // port b
}

#[test]
fn test_direction_mapping() {
    let (mut adapter, state) = make_adapter();
    adapter.start_port("a", PortDirection::Even, 80).unwrap();
    adapter.start_port("b", PortDirection::Odd, 60).unwrap();
    let cmds = state.lock().unwrap().commands.clone();
    adapter.disconnect();
    assert_eq!(i16::from_le_bytes([cmds[0][4], cmds[0][5]]), 800);
    assert_eq!(i16::from_le_bytes([cmds[1][4], cmds[1][5]]), -600);
}

#[test]
fn test_rotate_sends_run_for_degrees() {
    let (mut adapter, state) = make_adapter();
    adapter
        .rotate_port_by_degrees("c", PortDirection::Even, 50, 360)
        .unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_MOTOR_RUN_FOR_DEGREES);
    assert_eq!(cmd[3], 2); // port c
    assert_eq!(i32::from_le_bytes([cmd[4], cmd[5], cmd[6], cmd[7]]), 360);
    assert_eq!(i16::from_le_bytes([cmd[8], cmd[9]]), 500);
}

#[test]
fn test_parallel_rotate_uses_parallel_op() {
    let (mut adapter, state) = make_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Odd, power: 50 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 360).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PARALLEL_RUN_FOR_DEGREES);
    assert_eq!(cmd[3], 2); // count
    assert_eq!(cmd[4], 0); // port a
    assert_eq!(i32::from_le_bytes([cmd[5], cmd[6], cmd[7], cmd[8]]), 360);
    assert_eq!(i16::from_le_bytes([cmd[9], cmd[10]]), 500);
    assert_eq!(cmd[11], 1); // port b
    assert_eq!(i32::from_le_bytes([cmd[12], cmd[13], cmd[14], cmd[15]]), 360);
    assert_eq!(i16::from_le_bytes([cmd[16], cmd[17]]), -500);
}

#[test]
fn test_parallel_onfor_uses_parallel_op() {
    let (mut adapter, state) = make_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 75 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 75 },
    ];
    adapter.run_ports_for_time(&commands, 10).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PARALLEL_RUN_FOR_TIME);
    assert_eq!(u32::from_le_bytes([cmd[3], cmd[4], cmd[5], cmd[6]]), 1000);
    assert_eq!(cmd[7], 2); // count
    assert_eq!(cmd[8], 0); // port a
    assert_eq!(i16::from_le_bytes([cmd[9], cmd[10]]), 750);
    assert_eq!(cmd[11], 1); // port b
    assert_eq!(i16::from_le_bytes([cmd[12], cmd[13]]), 750);
}

#[test]
fn test_parallel_rotate_to_abs_uses_parallel_op() {
    // Regression guard against the batch override being removed in favour
    // of the default per-port loop.
    let (mut adapter, state) = make_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Odd, power: 50 },
    ];
    adapter.rotate_ports_to_abs(&commands, 0).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PARALLEL_RUN_TO_ABS);
    assert_eq!(cmd[3], 2, "count of entries in parallel op");
}

#[test]
fn test_parallel_rotate_to_position_uses_parallel_op() {
    // `rotate_ports_to_position` reads the current rotation for each port
    // (so N read commands are emitted), then should dispatch ONE
    // `parallel_run_for_degrees` with every non-zero delta.
    let (mut adapter, state) = make_adapter_with_override(Some(Reply::Int(90)));
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_to_position(&commands, 0).unwrap();
    let cmds = state.lock().unwrap().commands.clone();
    adapter.disconnect();

    // Commands emitted: read(a), read(b), parallel_run_for_degrees.
    let parallel = cmds
        .iter()
        .find(|c| parse_op(c) == protocol::OP_PARALLEL_RUN_FOR_DEGREES)
        .expect("expected one parallel_run_for_degrees command");
    assert_eq!(parallel[3], 2, "two ports in the parallel op");
    let per_port_count = cmds
        .iter()
        .filter(|c| parse_op(c) == protocol::OP_MOTOR_RUN_FOR_DEGREES)
        .count();
    assert_eq!(
        per_port_count, 0,
        "expected no per-port MOTOR_RUN_FOR_DEGREES (that would mean sequential fallback)"
    );
}

#[test]
fn test_reset_zero() {
    let (mut adapter, state) = make_adapter();
    adapter.reset_port_zero("d").unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_MOTOR_RESET);
    assert_eq!(cmd[3], 3); // port d
    assert_eq!(i32::from_le_bytes([cmd[4], cmd[5], cmd[6], cmd[7]]), 0);
}

#[test]
fn test_rotate_to_abs() {
    let (mut adapter, state) = make_adapter();
    adapter.rotate_to_abs("e", PortDirection::Even, 50, 90).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_MOTOR_RUN_TO_ABS);
    assert_eq!(cmd[3], 4); // port e
    assert_eq!(i32::from_le_bytes([cmd[4], cmd[5], cmd[6], cmd[7]]), 90);
    assert_eq!(i16::from_le_bytes([cmd[8], cmd[9]]), 500);
    assert_eq!(cmd[10], 0); // CW
}

#[test]
fn test_rotate_to_abs_counterclockwise() {
    let (mut adapter, state) = make_adapter();
    adapter.rotate_to_abs("f", PortDirection::Odd, 50, 90).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(cmd[10], 1); // CCW
}

#[test]
fn test_read_sensor_returns_value() {
    let (mut adapter, _state) = make_adapter_with_override(Some(Reply::Int(42)));
    let result = adapter.read_sensor("a", Some("rotation")).unwrap();
    adapter.disconnect();
    assert_eq!(result, Some(LogoValue::Number(42.0)));
}

#[test]
fn test_read_sensor_list_value() {
    let (mut adapter, _state) =
        make_adapter_with_override(Some(Reply::List(vec![10, 20, 30])));
    let result = adapter.read_sensor("a", Some("rotation")).unwrap();
    adapter.disconnect();
    assert_eq!(
        result,
        Some(LogoValue::List(vec![
            LogoValue::Number(10.0),
            LogoValue::Number(20.0),
            LogoValue::Number(30.0),
        ]))
    );
}

#[test]
fn test_validate_output_port_unknown_letter() {
    let (adapter, _) = make_adapter();
    assert!(adapter.validate_output_port("g").is_err());
}

#[test]
fn test_validate_output_port_unknown_type_is_permissive() {
    // PORT_TYPE_UNKNOWN (0xFFFF) means snapshot didn't resolve — adapter
    // defers to firmware so we don't lock users out on an unrecognised
    // firmware revision.
    let (adapter, _) = make_adapter_with_types([0xFFFF; 6]);
    assert!(adapter.validate_output_port("a").is_ok());
    assert!(adapter.validate_output_port("f").is_ok());
}

#[test]
fn test_validate_output_port_accepts_motor() {
    let (adapter, _) = make_adapter_with_types([
        rust_spike::constants::DEVICE_LARGE_ANGULAR_MOTOR,
        0, 0, 0, 0, 0,
    ]);
    assert!(adapter.validate_output_port("a").is_ok());
}

#[test]
fn test_validate_output_port_accepts_light() {
    let (adapter, _) = make_adapter_with_types([
        0, 0, rust_spike::constants::DEVICE_LIGHT, 0, 0, 0,
    ]);
    assert!(adapter.validate_output_port("c").is_ok());
}

#[test]
fn test_validate_output_port_rejects_sensor() {
    let (adapter, _) = make_adapter_with_types([
        0, rust_spike::constants::DEVICE_COLOR_SENSOR, 0, 0, 0, 0,
    ]);
    let err = adapter.validate_output_port("b").unwrap_err();
    assert!(
        err.contains("not a motor or light"),
        "unexpected wording: {}",
        err
    );
}

#[test]
fn test_validate_output_port_no_device() {
    let (adapter, _) = make_adapter_with_types([0; 6]);
    let err = adapter.validate_output_port("d").unwrap_err();
    assert!(
        err.contains("No device on port"),
        "unexpected wording: {}",
        err
    );
}

#[test]
fn test_validate_sensor_ports() {
    let (adapter, _) = make_adapter();
    assert!(adapter.validate_sensor_port("a", Some("rotation")).is_ok());
    assert!(adapter.validate_sensor_port("z", None).is_err());
}

#[test]
fn test_writes_are_framed() {
    let (mut adapter, state) = make_adapter();
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    adapter.disconnect();
    let writes = &state.lock().unwrap().writes;
    for w in writes {
        assert_eq!(*w.last().unwrap(), cobs::END_FRAME);
    }
}

#[test]
fn test_command_rids_are_unique() {
    let (mut adapter, state) = make_adapter();
    adapter.start_port("a", PortDirection::Even, 10).unwrap();
    adapter.start_port("b", PortDirection::Even, 20).unwrap();
    adapter.start_port("c", PortDirection::Even, 30).unwrap();
    let cmds = state.lock().unwrap().commands.clone();
    adapter.disconnect();
    let mut rids: Vec<u16> = cmds.iter().map(|c| parse_rid(c)).collect();
    let total = rids.len();
    rids.sort();
    rids.dedup();
    assert_eq!(rids.len(), total);
    assert_eq!(rids.len(), 3);
}

#[test]
fn test_reply_to_logo_bool() {
    assert_eq!(
        reply_to_logo(Reply::Bool(true)),
        LogoValue::Word("true".to_string())
    );
    assert_eq!(
        reply_to_logo(Reply::Bool(false)),
        LogoValue::Word("false".to_string())
    );
}

#[test]
fn test_start_port_on_led_uses_port_pwm() {
    // LED on port A (DEVICE_LIGHT) — `motor.run` would ENODEV, so the
    // adapter must dispatch to direct PWM instead.
    let (mut adapter, state) = make_adapter_with_types([
        rust_spike::constants::DEVICE_LIGHT,
        0, 0, 0, 0, 0,
    ]);
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PORT_PWM);
    assert_eq!(cmd[3], 0); // port a
    assert_eq!(cmd[4] as i8, 50);
}

#[test]
fn test_start_port_on_passive_motor_uses_port_pwm() {
    // DEVICE_PASSIVE_MOTOR has no tacho — same dispatch as LED.
    let (mut adapter, state) = make_adapter_with_types([
        0, 0, rust_spike::constants::DEVICE_PASSIVE_MOTOR, 0, 0, 0,
    ]);
    adapter.start_port("c", PortDirection::Odd, 75).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PORT_PWM);
    assert_eq!(cmd[3], 2); // port c
    assert_eq!(cmd[4] as i8, -75); // Odd direction → negative
}

#[test]
fn test_stop_port_on_led_uses_port_pwm_zero() {
    let (mut adapter, state) = make_adapter_with_types([
        rust_spike::constants::DEVICE_LIGHT,
        0, 0, 0, 0, 0,
    ]);
    adapter.stop_port("a").unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PORT_PWM);
    assert_eq!(cmd[3], 0);
    assert_eq!(cmd[4] as i8, 0);
}

#[test]
fn test_start_port_unknown_type_uses_pwm() {
    // PORT_TYPE_UNKNOWN = 0xFFFF (snapshot didn't resolve). PWM is the safe
    // fallback — works on any output device, including tacho motors.
    let (mut adapter, state) = make_adapter_with_types([0xFFFF; 6]);
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PORT_PWM);
}

#[test]
fn test_start_port_on_linear_actuator_uses_port_pwm() {
    // DEVICE_MEDIUM_LINEAR_MOTOR (38) has a tacho but no absolute encoder.
    // SPIKE 3's `motor` module ENODEVs on it — adapter must treat it like
    // a passive motor and dispatch to `device.set_duty_cycle` via OP_PORT_PWM.
    let (mut adapter, state) = make_adapter_with_types([
        rust_spike::constants::DEVICE_MEDIUM_LINEAR_MOTOR,
        0, 0, 0, 0, 0,
    ]);
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PORT_PWM);
}

#[test]
fn test_rotate_by_degrees_rejects_passive_motor() {
    let (mut adapter, _) = make_adapter_with_types([
        rust_spike::constants::DEVICE_PASSIVE_MOTOR,
        0, 0, 0, 0, 0,
    ]);
    let err = adapter
        .rotate_port_by_degrees("a", PortDirection::Even, 50, 90)
        .unwrap_err();
    adapter.disconnect();
    assert!(
        err.contains("absolute-position encoder"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_rotate_by_degrees_rejects_tacho_only_motor() {
    // Tacho-only types (38/46/47) have relative encoders but no absolute.
    // SPIKE 3 firmware's `motor.run_for_degrees` ENODEVs on these —
    // confirmed empirically on Hub OS 3.4.0 with a 88008 medium linear.
    let (mut adapter, _) = make_adapter_with_types([
        rust_spike::constants::DEVICE_MEDIUM_LINEAR_MOTOR,
        0, 0, 0, 0, 0,
    ]);
    let err = adapter
        .rotate_port_by_degrees("a", PortDirection::Even, 50, 90)
        .unwrap_err();
    adapter.disconnect();
    assert!(
        err.contains("absolute-position encoder"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_rotate_by_degrees_rejects_light() {
    let (mut adapter, _) = make_adapter_with_types([
        rust_spike::constants::DEVICE_LIGHT,
        0, 0, 0, 0, 0,
    ]);
    let err = adapter
        .rotate_port_by_degrees("a", PortDirection::Even, 50, 90)
        .unwrap_err();
    adapter.disconnect();
    assert!(err.contains("absolute-position encoder"));
}

#[test]
fn test_reset_zero_rejects_passive_motor() {
    let (mut adapter, _) = make_adapter_with_types([
        rust_spike::constants::DEVICE_PASSIVE_MOTOR,
        0, 0, 0, 0, 0,
    ]);
    let err = adapter.reset_port_zero("a").unwrap_err();
    adapter.disconnect();
    assert!(err.contains("absolute-position encoder"));
}

#[test]
fn test_reset_zero_rejects_tacho_only_motor() {
    // motor.reset_relative_position requires absolute encoder on SPIKE 3
    // (resets the absolute reference). Tacho-only motors are rejected.
    let (mut adapter, _) = make_adapter_with_types([
        rust_spike::constants::DEVICE_MEDIUM_LINEAR_MOTOR,
        0, 0, 0, 0, 0,
    ]);
    let err = adapter.reset_port_zero("a").unwrap_err();
    adapter.disconnect();
    assert!(err.contains("absolute-position encoder"));
}

#[test]
fn test_rotate_to_abs_rejects_passive_motor() {
    let (mut adapter, _) = make_adapter_with_types([
        rust_spike::constants::DEVICE_PASSIVE_MOTOR,
        0, 0, 0, 0, 0,
    ]);
    let err = adapter
        .rotate_to_abs("a", PortDirection::Even, 50, 90)
        .unwrap_err();
    adapter.disconnect();
    assert!(err.contains("absolute-position encoder"));
}

#[test]
fn test_run_ports_for_time_mixed_falls_back_to_per_port() {
    // Mixed group: A is LED, B is tacho motor. The parallel op would route
    // both through `motor.run`, ENODEV-ing on A. Adapter must split and
    // dispatch per-port via start_port/stop_port.
    let (mut adapter, state) = make_adapter_with_types([
        rust_spike::constants::DEVICE_LIGHT,
        rust_spike::constants::DEVICE_LARGE_ANGULAR_MOTOR,
        0, 0, 0, 0,
    ]);
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap(); // 100ms
    let cmds = state.lock().unwrap().commands.clone();
    adapter.disconnect();

    // No PARALLEL_RUN_FOR_TIME — instead two starts (one PWM, one MOTOR_RUN)
    // and two stops (PWM 0 and MOTOR_STOP).
    let parallel_count = cmds
        .iter()
        .filter(|c| parse_op(c) == protocol::OP_PARALLEL_RUN_FOR_TIME)
        .count();
    assert_eq!(parallel_count, 0, "expected no parallel op for mixed group");

    let pwm_count = cmds
        .iter()
        .filter(|c| parse_op(c) == protocol::OP_PORT_PWM)
        .count();
    let motor_run_count = cmds
        .iter()
        .filter(|c| parse_op(c) == protocol::OP_MOTOR_RUN)
        .count();
    let motor_stop_count = cmds
        .iter()
        .filter(|c| parse_op(c) == protocol::OP_MOTOR_STOP)
        .count();
    assert_eq!(pwm_count, 2, "LED gets PWM start + PWM stop");
    assert_eq!(motor_run_count, 1, "motor gets motor_run start");
    assert_eq!(motor_stop_count, 1, "motor gets motor_stop");
}

#[test]
fn test_run_ports_for_time_all_tacho_uses_parallel_op() {
    // Sanity: when every port is a tacho motor, the parallel batch is still
    // used (it's the whole point of the SPIKE override).
    let (mut adapter, state) = make_adapter_with_types([
        rust_spike::constants::DEVICE_LARGE_ANGULAR_MOTOR,
        rust_spike::constants::DEVICE_LARGE_ANGULAR_MOTOR,
        0, 0, 0, 0,
    ]);
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 10).unwrap();
    let cmd = state.lock().unwrap().commands[0].clone();
    adapter.disconnect();
    assert_eq!(parse_op(&cmd), protocol::OP_PARALLEL_RUN_FOR_TIME);
}

#[test]
fn test_port_event_push_updates_cache() {
    // Agent's poll-and-diff loop pushes a port_event when a device type
    // changes. Slot must accept it and update the shared cache so
    // validate_output_port reflects the new state without reconnecting.
    let (mut adapter, state) = make_adapter_with_types([0; 6]);
    assert!(adapter.validate_output_port("c").is_err());

    let payload = vec![
        protocol::REPLY_PORT_EVENT,
        2,
        rust_spike::constants::DEVICE_LIGHT as u8,
        (rust_spike::constants::DEVICE_LIGHT >> 8) as u8,
    ];
    let mut msg = Vec::with_capacity(3 + payload.len());
    msg.push(atlantis::ID_TUNNEL_MESSAGE);
    msg.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    msg.extend_from_slice(&payload);
    state.lock().unwrap().outgoing.extend(cobs::pack(&msg));

    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if adapter.port_types.lock().unwrap()[2]
            == rust_spike::constants::DEVICE_LIGHT
        {
            break;
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "port_types[2] never updated; got {:#x}",
                adapter.port_types.lock().unwrap()[2]
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    adapter.disconnect();
    assert_eq!(
        adapter.port_types.lock().unwrap()[2],
        rust_spike::constants::DEVICE_LIGHT
    );
}

#[test]
fn test_port_event_out_of_bounds_dropped() {
    // Malformed agent push with port=99 must be dropped silently.
    let (mut adapter, state) = make_adapter_with_types([0; 6]);
    let payload = vec![protocol::REPLY_PORT_EVENT, 99, 0x08, 0x00];
    let mut msg = Vec::with_capacity(3 + payload.len());
    msg.push(atlantis::ID_TUNNEL_MESSAGE);
    msg.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    msg.extend_from_slice(&payload);
    state.lock().unwrap().outgoing.extend(cobs::pack(&msg));

    std::thread::sleep(Duration::from_millis(100));
    let cache = *adapter.port_types.lock().unwrap();
    adapter.disconnect();
    assert_eq!(cache, [0u16; 6]);
}

#[test]
fn test_reply_to_logo_type_list() {
    assert_eq!(
        reply_to_logo(Reply::TypeList(vec![1, 75, 0])),
        LogoValue::List(vec![
            LogoValue::Number(1.0),
            LogoValue::Number(75.0),
            LogoValue::Number(0.0),
        ])
    );
}

#[test]
fn test_ping_event_roundtrip() {
    // Sanity-check that Event::Heartbeat parses cleanly — the slot uses this
    // path for watchdog updates.
    let bytes = vec![protocol::REPLY_HEARTBEAT];
    assert_eq!(protocol::parse_event(&bytes).unwrap(), Event::Heartbeat);
}
