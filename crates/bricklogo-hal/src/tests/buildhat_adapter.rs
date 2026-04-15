use super::*;
use crate::scheduler;
use std::sync::{Arc, Mutex};

/// Mock serial transport for BuildHAT — records every text command written,
/// returns no data on read.
struct MockTransport {
    writes: Arc<Mutex<Vec<String>>>,
}

impl BuildHATTransport for MockTransport {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, String> {
        Ok(0)
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.writes
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(data).to_string());
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Build an adapter wired to a mock transport with a passive motor already
/// "connected" on both port A and port B. Basic motors (DEVICE_PASSIVE_MOTOR)
/// take the PWM+sleep+coast path in run_ports_for_time, which is what the
/// default HardwareAdapter trait impl exercises.
fn make_adapter_with_mock() -> (BuildHATAdapter, Arc<Mutex<Vec<String>>>) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn BuildHATTransport> = Box::new(MockTransport {
        writes: writes.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(BuildHATShared::new()));
    // Pretend a basic motor is attached on ports 0 (A) and 1 (B).
    {
        let mut s = shared.lock().unwrap();
        s.ports[0] = PortInfo { type_id: DEVICE_PASSIVE_MOTOR, connected: true };
        s.ports[1] = PortInfo { type_id: DEVICE_PASSIVE_MOTOR, connected: true };
    }
    let slot = BuildHATSlot {
        port: transport,
        rx,
        shared: shared.clone(),
        read_buffer: String::new(),
        alive: true,
        pending_inits: Vec::new(),
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = BuildHATAdapter::new();
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    adapter.shared = shared;
    (adapter, writes)
}

fn count_matching(writes: &[String], needle: &str) -> usize {
    writes.iter().filter(|w| w.contains(needle)).count()
}

#[test]
fn test_buildhat_run_ports_for_time_starts_both_motors() {
    // BuildHAT inherits the default run_ports_for_time (start_port per port
    // + sleep + stop_port per port). This verifies that BOTH motors get
    // started — i.e. the multi-port dispatch reaches the adapter correctly.
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap(); // 100ms
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    // `set` sends PWM on each port; there should be one per port at the
    // start phase. Stop phase sends `coast`.
    let set_count = count_matching(&writes, "set ");
    let coast_count = count_matching(&writes, "coast");
    assert_eq!(
        set_count, 2,
        "expected 2 `set` writes (one per port), got {}: {:?}",
        set_count, writes
    );
    assert_eq!(
        coast_count, 2,
        "expected 2 `coast` writes (one per port), got {}: {:?}",
        coast_count, writes
    );
}

#[test]
fn test_buildhat_run_ports_for_time_single_port_not_degraded() {
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 50,
    }];
    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert_eq!(count_matching(&writes, "set "), 1);
    assert_eq!(count_matching(&writes, "coast"), 1);
}

#[test]
fn test_buildhat_start_ports_sends_per_port_set() {
    // Direct test of start_ports: should emit one `set` per port.
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Odd, power: 75 },
    ];
    adapter.start_ports(&commands).unwrap();
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert_eq!(count_matching(&writes, "set "), 2);
}
