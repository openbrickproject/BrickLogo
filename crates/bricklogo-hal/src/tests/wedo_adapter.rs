use super::*;
use crate::scheduler;
use std::sync::{Arc, Mutex};

/// Mock HID transport: records every written frame; returns no data on read
/// (simulates "no sensor samples available this tick").
struct MockTransport {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl WeDoTransport for MockTransport {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, String> {
        Ok(0)
    }
    fn write(&mut self, data: &[u8]) -> Result<usize, String> {
        self.writes.lock().unwrap().push(data.to_vec());
        Ok(data.len())
    }
}

fn make_adapter_with_mock() -> (WeDoAdapter, Arc<Mutex<Vec<Vec<u8>>>>) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn WeDoTransport> = Box::new(MockTransport {
        writes: writes.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(WeDoShared::new()));
    let slot = WeDoSlot {
        device: transport,
        rx,
        shared: shared.clone(),
        output_bits: 0,
        motor_values: [0, 0],
        alive: true,
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = WeDoAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    adapter.shared = shared;
    (adapter, writes)
}

#[test]
fn test_wedo_run_ports_for_time_single_hid_write_per_phase() {
    // start_ports sets both motor values in one shot; the slot emits a
    // single HID frame carrying both motor bytes. Same for stop_ports.
    // So a 2-port run_ports_for_time issues exactly 2 HID writes.
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert_eq!(
        writes.len(),
        2,
        "expected 2 HID writes (1 on, 1 off), got {}",
        writes.len()
    );

    // First write: motor_a and motor_b both non-zero.
    let on = &writes[0];
    assert_eq!(on.len(), 9, "WeDo motor frame is 9 bytes");
    assert_ne!(on[2], 0, "motor_a should be running in first write");
    assert_ne!(on[3], 0, "motor_b should be running in first write");

    // Second write: both off.
    let off = &writes[1];
    assert_eq!(off[2], 0, "motor_a should be stopped in second write");
    assert_eq!(off[3], 0, "motor_b should be stopped in second write");
}

#[test]
fn test_wedo_run_ports_for_time_single_port_not_degraded() {
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 50,
    }];
    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 2);
    assert_ne!(writes[0][2], 0, "motor_a should be running");
    assert_eq!(writes[0][3], 0, "motor_b should remain stopped");
}
