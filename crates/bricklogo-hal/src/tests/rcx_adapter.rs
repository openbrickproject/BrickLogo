use super::*;
use crate::scheduler;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Records every frame sent; always ACKs with an empty payload so the slot
/// sees each command as successful.
struct MockTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl RcxTransport for MockTransport {
    fn send(&mut self, msg: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(msg.to_vec());
        Ok(())
    }
    fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.sent.lock().unwrap().push(msg.to_vec());
        Ok(Vec::new())
    }
    fn request_firmware(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.request(msg)
    }
    fn read_available(&mut self, _buf: &mut [u8]) -> Result<usize, String> {
        Ok(0)
    }
}

fn make_adapter_with_mock() -> (RcxAdapter, Arc<Mutex<Vec<Vec<u8>>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn RcxTransport> = Box::new(MockTransport {
        sent: sent.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let slot = RcxSlot {
        transport,
        rx,
        alive: true,
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = RcxAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    (adapter, sent)
}

/// Return all frames whose opcode matches `op`.
fn frames_with_op(sent: &[Vec<u8>], op: u8) -> Vec<Vec<u8>> {
    sent.iter()
        .filter(|f| f.len() > 3 && f[3] == op)
        .cloned()
        .collect()
}

/// Extract motor+state byte from a `cmd_set_motor_state` frame.
/// Frame layout: [HEADER(3), OP, !OP, code, !code, checksum, !checksum]
fn motor_state_code(frame: &[u8]) -> u8 {
    frame[5]
}

#[test]
fn test_rcx_run_ports_for_time_batches_motor_on_off() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 4 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 4 },
    ];

    adapter.run_ports_for_time(&commands, 1).unwrap(); // 100ms
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let motor_frames = frames_with_op(&sent, OP_SET_MOTOR_ON_OFF);

    // Batched: exactly one ON frame and one OFF frame.
    assert_eq!(
        motor_frames.len(),
        2,
        "expected 2 motor on/off frames (1 batched on, 1 batched off), got {}: {:?}",
        motor_frames.len(),
        motor_frames
    );

    // First frame: ON with combined mask A|B = 0x03.
    assert_eq!(
        motor_state_code(&motor_frames[0]),
        MOTOR_A | MOTOR_B | MOTOR_ON,
        "first motor frame should be ON with combined mask"
    );
    // Second frame: OFF with combined mask.
    assert_eq!(
        motor_state_code(&motor_frames[1]),
        MOTOR_A | MOTOR_B | MOTOR_OFF,
        "second motor frame should be OFF with combined mask"
    );
}

#[test]
fn test_rcx_run_ports_for_time_single_port_not_degraded() {
    // Regression guard: batch path must still work for a single port.
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 4,
    }];

    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let motor_frames = frames_with_op(&sent, OP_SET_MOTOR_ON_OFF);
    assert_eq!(motor_frames.len(), 2);
    assert_eq!(motor_state_code(&motor_frames[0]), MOTOR_A | MOTOR_ON);
    assert_eq!(motor_state_code(&motor_frames[1]), MOTOR_A | MOTOR_OFF);
}

#[test]
fn test_rcx_run_ports_for_time_timing() {
    // Verify the batch path actually sleeps for the commanded duration,
    // and doesn't sequentialize internally.
    let (mut adapter, _sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 4 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 4 },
    ];

    let start = std::time::Instant::now();
    adapter.run_ports_for_time(&commands, 2).unwrap(); // 200ms
    let elapsed = start.elapsed();
    adapter.disconnect();

    // Sequential sleeps would be ~400ms; batch path ~200ms. Allow generous
    // headroom for scheduler latency (~16ms per send_cmd_wait × several calls).
    assert!(
        elapsed >= Duration::from_millis(180),
        "too fast — didn't actually sleep: {:?}",
        elapsed
    );
    assert!(
        elapsed < Duration::from_millis(400),
        "sequentialized: {:?}",
        elapsed
    );
}
