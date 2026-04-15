use super::*;
use crate::scheduler;
use std::sync::{Arc, Mutex};

/// Mock serial transport — records every frame written, ignores reads.
struct MockTransport {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ControlLabTransport for MockTransport {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, String> {
        Ok(0)
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.writes.lock().unwrap().push(data.to_vec());
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn make_adapter_with_mock() -> (ControlLabAdapter, Arc<Mutex<Vec<Vec<u8>>>>) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn ControlLabTransport> = Box::new(MockTransport {
        writes: writes.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(ControlLabShared::new()));
    let slot = ControlLabSlot {
        port: transport,
        rx,
        shared: shared.clone(),
        read_buffer: Vec::new(),
        last_write: Instant::now(),
        alive: true,
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = ControlLabAdapter::new("/dev/null");
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    adapter.shared = shared;
    (adapter, writes)
}

/// Filter out keep-alive frames (opcode 0x02). Motor commands use other opcodes.
fn motor_frames(writes: &[Vec<u8>]) -> Vec<Vec<u8>> {
    writes
        .iter()
        .filter(|f| !f.is_empty() && f[0] != 0x02) // 0x02 = keep-alive
        .cloned()
        .collect()
}

#[test]
fn test_controllab_run_ports_for_time_batches_same_power() {
    // Two ports at the same power collapse into one DirectionLeft frame
    // with a combined mask — that's the slot's power_groups batching.
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 4 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 4 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    let motor = motor_frames(&writes);
    // Expect: 1 DirectionLeft (combined mask), 1 PowerOff (combined mask).
    // `motor` may include a few extra keep-alive-esque writes in rare
    // races, but there should be exactly 2 frames that are direction/off.
    let direction_frames: Vec<_> = motor
        .iter()
        .filter(|f| f[0] == rust_controllab::constants::ControlLabCommand::DirectionLeft as u8)
        .collect();
    let off_frames: Vec<_> = motor
        .iter()
        .filter(|f| f[0] == rust_controllab::constants::ControlLabCommand::PowerOff as u8)
        .collect();

    assert_eq!(
        direction_frames.len(),
        1,
        "expected 1 direction frame for same-power batch, got {}",
        direction_frames.len()
    );
    assert_eq!(
        off_frames.len(),
        1,
        "expected 1 off frame (combined mask), got {}",
        off_frames.len()
    );

    // Direction frame's second byte is the combined output mask; both
    // port A (0x01) and port B (0x02) bits should be set.
    let mask = direction_frames[0][1];
    assert_eq!(mask & 0x03, 0x03, "combined mask should include A|B");

    // Off frame should also target both ports.
    let off_mask = off_frames[0][1];
    assert_eq!(off_mask & 0x03, 0x03, "off mask should include A|B");
}

#[test]
fn test_controllab_run_ports_for_time_single_port_not_degraded() {
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 4,
    }];
    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    let motor = motor_frames(&writes);
    let off_frames: Vec<_> = motor
        .iter()
        .filter(|f| f[0] == rust_controllab::constants::ControlLabCommand::PowerOff as u8)
        .collect();
    assert_eq!(off_frames.len(), 1);
    // Only port A bit set.
    assert_eq!(off_frames[0][1] & 0x03, 0x01);
}
