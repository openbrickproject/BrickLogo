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
    let slot = SpikeSlot {
        transport,
        rx,
        framer: FrameReader::new(),
        pending: HashMap::new(),
        alive: true,
        alive_flag: alive_flag.clone(),
        last_heartbeat: Instant::now(),
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = SpikeAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    adapter.alive = alive_flag;
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
fn test_validate_ports() {
    let (adapter, _) = make_adapter();
    assert!(adapter.validate_output_port("a").is_ok());
    assert!(adapter.validate_output_port("f").is_ok());
    assert!(adapter.validate_output_port("g").is_err());
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
fn test_ping_event_roundtrip() {
    // Sanity-check that Event::Heartbeat parses cleanly — the slot uses this
    // path for watchdog updates.
    let bytes = vec![protocol::REPLY_HEARTBEAT];
    assert_eq!(protocol::parse_event(&bytes).unwrap(), Event::Heartbeat);
}
