use super::*;
use crate::scheduler;
use rust_ev3::protocol;
use std::sync::{Arc, Mutex};

/// Captures every sent frame and fabricates a generic "success" reply for
/// any request (test_busy, get_count, etc.). Reply payload is 4 zero bytes —
/// enough to satisfy `test_busy` (first byte=0 → not busy), `get_count`
/// (4-byte i32 = 0), and `read_sensor_si`/`pct` (value = 0).
struct MockTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl rust_ev3::transport::Transport for MockTransport {
    fn send(&mut self, frame: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(frame.to_vec());
        Ok(())
    }
    fn recv(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, String> {
        // Reply to the most recent send. EV3 request/reply is strictly
        // counter-matched, so echoing the last sent counter works.
        let sent = self.sent.lock().unwrap();
        let last = sent.last().ok_or("mock: no frame sent")?.clone();
        drop(sent);
        if last.len() < 4 {
            return Err("mock: bad frame".into());
        }
        let counter = u16::from_le_bytes([last[2], last[3]]);
        let payload = [0u8; 4];
        let len = (2 + 1 + payload.len()) as u16;
        let mut reply = Vec::with_capacity(2 + len as usize);
        reply.extend_from_slice(&len.to_le_bytes());
        reply.extend_from_slice(&counter.to_le_bytes());
        reply.push(0x02); // DirectReplyOk
        reply.extend_from_slice(&payload);
        Ok(reply)
    }
}

fn make_adapter_with_mock() -> (EV3Adapter, Arc<Mutex<Vec<Vec<u8>>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn rust_ev3::transport::Transport> = Box::new(MockTransport {
        sent: sent.clone(),
    });
    let ev3 = Ev3::new(transport);
    let (tx, rx) = mpsc::channel();
    let alive = Arc::new(AtomicBool::new(true));
    let slot = EV3Slot {
        ev3,
        rx,
        alive: alive.clone(),
        pending: Vec::new(),
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = EV3Adapter::new(None);
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    adapter.alive = alive;
    (adapter, sent)
}

/// Opcode is the first byte of the frame body, which sits after:
/// length(2) + counter(2) + type(1) + header(2) = 7 bytes.
fn frame_opcode(frame: &[u8]) -> Option<u8> {
    frame.get(7).copied()
}

fn frames_with_opcode(sent: &[Vec<u8>], op: u8) -> Vec<Vec<u8>> {
    sent.iter()
        .filter(|f| frame_opcode(f) == Some(op))
        .cloned()
        .collect()
}

#[test]
fn test_ev3_run_ports_for_time_fires_both_motors() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let time_power_frames = frames_with_opcode(&sent, protocol::OP_OUTPUT_TIME_POWER);
    // The batch fires one time_power per port — both without waiting between
    // them, so the motors start within ~1ms of each other on real hardware.
    assert_eq!(
        time_power_frames.len(),
        2,
        "expected 2 OUTPUT_TIME_POWER frames (one per port), got {}: {:#04x?}",
        time_power_frames.len(),
        sent.iter().filter_map(|f| frame_opcode(f)).collect::<Vec<_>>()
    );

    // After the batch fires, the slot polls OUTPUT_TEST for the combined
    // mask (0x01|0x02 = 0x03) to detect completion.
    let test_frames = frames_with_opcode(&sent, protocol::OP_OUTPUT_TEST);
    assert!(!test_frames.is_empty(), "expected at least one OUTPUT_TEST poll");
}

#[test]
fn test_ev3_rotate_ports_by_degrees_fires_both_motors() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 90).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let step_frames = frames_with_opcode(&sent, protocol::OP_OUTPUT_STEP_POWER);
    assert_eq!(
        step_frames.len(),
        2,
        "expected 2 OUTPUT_STEP_POWER frames (one per port), got {}",
        step_frames.len()
    );
}

#[test]
fn test_ev3_rotate_ports_to_position_fires_both_motors() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    // Mock get_count returns 0, so the mod-360 delta to target=90 is 90°.
    adapter.rotate_ports_to_position(&commands, 90).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let step_frames = frames_with_opcode(&sent, protocol::OP_OUTPUT_STEP_POWER);
    assert_eq!(
        step_frames.len(),
        2,
        "expected 2 OUTPUT_STEP_POWER frames (one per port), got {}",
        step_frames.len()
    );
}

#[test]
fn test_ev3_run_ports_for_time_single_port_not_degraded() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 50,
    }];
    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let time_power_frames = frames_with_opcode(&sent, protocol::OP_OUTPUT_TIME_POWER);
    assert_eq!(time_power_frames.len(), 1);
}

#[test]
fn test_ev3_rotate_to_home_errors() {
    // EV3 motors have no absolute-position encoder — rotate_to_home
    // should return an error, and the batch method inherits the default
    // which loops over rotate_to_home and therefore errors on the first
    // port.
    let (mut adapter, _sent) = make_adapter_with_mock();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 50,
    }];
    let r = adapter.rotate_ports_to_home(&commands);
    adapter.disconnect();
    assert!(r.is_err(), "EV3 should reject rotate_to_home");
}
