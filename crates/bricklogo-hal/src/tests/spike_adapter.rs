use super::*;
use crate::adapter::{PortCommand, PortDirection};
use crate::scheduler;
use rust_spike::atlantis;
use rust_spike::cobs;
use rust_spike::protocol;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Mock Atlantis transport. Records incoming frames and auto-replies with
/// plausible agent responses: motor commands get `{"id":N,"ok":true}`,
/// reads get `{"id":N,"value":0}`.
struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

struct MockState {
    /// Raw bytes written by the adapter (exactly as sent to the wire).
    writes: Vec<Vec<u8>>,
    /// Decoded JSON command payloads carried by TunnelMessage frames.
    commands: Vec<serde_json::Value>,
    /// Raw bytes queued for the adapter to read.
    outgoing: VecDeque<u8>,
    /// If set, the mock auto-generates agent replies for tunnel commands.
    auto_reply: bool,
    /// Optional override returning a specific value for a `read` op.
    read_override: Option<serde_json::Value>,
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

        // Extract complete frames and handle each.
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
            // Host→hub TunnelMessages use the plain wire layout
            // `[0x32, size_u16, payload]` — matches what the LEGO firmware
            // delivers to the MicroPython `module_tunnel` callback.
            if body.first() == Some(&atlantis::ID_TUNNEL_MESSAGE) && body.len() >= 3 {
                let size = u16::from_le_bytes([body[1], body[2]]) as usize;
                let end = (3 + size).min(body.len());
                handle_tunnel_payload(&mut state, &body[3..end]);
            }
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn handle_tunnel_payload(state: &mut MockState, payload: &[u8]) {
    for line in payload.split(|&b: &u8| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let cmd: serde_json::Value = match serde_json::from_slice(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        state.commands.push(cmd.clone());
        if !state.auto_reply {
            continue;
        }
        let id = cmd.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
        let op = cmd.get("op").and_then(|v| v.as_str()).unwrap_or("");
        let reply = if op == "read" {
            state
                .read_override
                .clone()
                .map(|v| serde_json::json!({"id": id, "value": v}))
                .unwrap_or(serde_json::json!({"id": id, "value": 0}))
        } else {
            serde_json::json!({"id": id, "ok": true})
        };
        enqueue_tunnel_reply(state, &reply);
    }
}

/// Emit a hub→host TunnelMessage in the plain `[0x32, size_u16, payload]`
/// layout the firmware actually uses.
fn enqueue_tunnel_reply(state: &mut MockState, reply: &serde_json::Value) {
    let mut line = serde_json::to_vec(reply).unwrap();
    line.push(b'\n');
    let mut msg = Vec::with_capacity(3 + line.len());
    msg.push(atlantis::ID_TUNNEL_MESSAGE);
    msg.extend_from_slice(&(line.len() as u16).to_le_bytes());
    msg.extend_from_slice(&line);
    let framed = cobs::pack(&msg);
    state.outgoing.extend(framed);
}

fn make_adapter() -> (SpikeAdapter, Arc<Mutex<MockState>>) {
    make_adapter_with_override(None)
}

fn make_adapter_with_override(
    read_override: Option<serde_json::Value>,
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

fn wait_for_commands(state: &Arc<Mutex<MockState>>, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if state.lock().unwrap().commands.len() >= count {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn test_start_port_sends_motor_run() {
    let (mut adapter, state) = make_adapter();
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["op"], "motor_run");
    assert_eq!(cmd["port"], "a");
    assert_eq!(cmd["velocity"], 500);
}

#[test]
fn test_stop_port_sends_motor_stop() {
    let (mut adapter, state) = make_adapter();
    adapter.stop_port("b").unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["op"], "motor_stop");
    assert_eq!(cmd["port"], "b");
}

#[test]
fn test_direction_mapping() {
    let (mut adapter, state) = make_adapter();
    adapter.start_port("a", PortDirection::Even, 80).unwrap();
    adapter.start_port("b", PortDirection::Odd, 60).unwrap();
    adapter.disconnect();
    let cmds = &state.lock().unwrap().commands;
    assert_eq!(cmds[0]["velocity"], 800);
    assert_eq!(cmds[1]["velocity"], -600);
}

#[test]
fn test_rotate_sends_run_for_degrees() {
    let (mut adapter, state) = make_adapter();
    adapter
        .rotate_port_by_degrees("c", PortDirection::Even, 50, 360)
        .unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["op"], "motor_run_for_degrees");
    assert_eq!(cmd["port"], "c");
    assert_eq!(cmd["degrees"], 360);
    assert_eq!(cmd["velocity"], 500);
}

#[test]
fn test_parallel_rotate_uses_parallel_op() {
    let (mut adapter, state) = make_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Odd, power: 50 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 360).unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["op"], "parallel_run_for_degrees");
    let entries = cmd["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["port"], "a");
    assert_eq!(entries[0]["velocity"], 500);
    assert_eq!(entries[1]["port"], "b");
    assert_eq!(entries[1]["velocity"], -500);
}

#[test]
fn test_parallel_onfor_uses_parallel_op() {
    let (mut adapter, state) = make_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 75 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 75 },
    ];
    adapter.run_ports_for_time(&commands, 10).unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["op"], "parallel_run_for_time");
    assert_eq!(cmd["ms"], 1000);
    let entries = cmd["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["velocity"], 750);
    assert_eq!(entries[1]["velocity"], 750);
}

#[test]
fn test_reset_zero() {
    let (mut adapter, state) = make_adapter();
    adapter.reset_port_zero("d").unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["op"], "motor_reset");
    assert_eq!(cmd["port"], "d");
    assert_eq!(cmd["offset"], 0);
}

#[test]
fn test_rotate_to_abs() {
    let (mut adapter, state) = make_adapter();
    adapter.rotate_to_abs("e", PortDirection::Even, 50, 90).unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["op"], "motor_run_to_abs");
    assert_eq!(cmd["port"], "e");
    assert_eq!(cmd["position"], 90);
    assert_eq!(cmd["velocity"], 500);
    assert_eq!(cmd["direction"], 0);
}

#[test]
fn test_rotate_to_abs_counterclockwise() {
    let (mut adapter, state) = make_adapter();
    adapter.rotate_to_abs("f", PortDirection::Odd, 50, 90).unwrap();
    adapter.disconnect();
    let cmd = state.lock().unwrap().commands[0].clone();
    assert_eq!(cmd["direction"], 1);
}

#[test]
fn test_read_sensor_returns_value() {
    let (mut adapter, _state) = make_adapter_with_override(Some(serde_json::json!(42)));
    let result = adapter.read_sensor("a", Some("rotation")).unwrap();
    adapter.disconnect();
    assert_eq!(result, Some(LogoValue::Number(42.0)));
}

#[test]
fn test_read_sensor_list_value() {
    let (mut adapter, _state) =
        make_adapter_with_override(Some(serde_json::json!([10, 20, 30])));
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
    wait_for_commands(&state, 1);
    adapter.disconnect();
    let writes = &state.lock().unwrap().writes;
    // Every write must end with the Atlantis delimiter.
    for w in writes {
        assert_eq!(*w.last().unwrap(), cobs::END_FRAME);
    }
}

#[test]
fn test_command_ids_are_unique() {
    let (mut adapter, state) = make_adapter();
    adapter.start_port("a", PortDirection::Even, 10).unwrap();
    adapter.start_port("b", PortDirection::Even, 20).unwrap();
    adapter.start_port("c", PortDirection::Even, 30).unwrap();
    let cmds_snapshot = state.lock().unwrap().commands.clone();
    adapter.disconnect();
    let mut ids: Vec<u64> = cmds_snapshot
        .iter()
        .map(|c| c["id"].as_u64().unwrap())
        .collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), total);
    assert_eq!(ids.len(), 3);
}

#[test]
fn test_json_to_logo_bool() {
    assert_eq!(
        json_to_logo(&serde_json::json!(true)),
        LogoValue::Word("true".to_string())
    );
    assert_eq!(
        json_to_logo(&serde_json::json!(false)),
        LogoValue::Word("false".to_string())
    );
}

// Silence unused-function warnings when only some helpers are exercised above.
#[allow(dead_code)]
fn _touch_helpers() {
    let _ = wait_for_commands;
    let _ = protocol::ping;
}
