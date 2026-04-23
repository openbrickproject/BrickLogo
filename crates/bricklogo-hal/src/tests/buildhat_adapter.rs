use super::*;
use crate::scheduler;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

/// Build an adapter whose port 0 and port 1 hold absolute motors with
/// pre-populated combi data (speed, position, apos). Caller picks each
/// port's APOS for the home-delta test.
fn make_adapter_with_absolute_motors(
    apos_a: f64,
    apos_b: f64,
) -> (BuildHATAdapter, Arc<Mutex<Vec<String>>>, Arc<Mutex<BuildHATShared>>) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn BuildHATTransport> = Box::new(MockTransport {
        writes: writes.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(BuildHATShared::new()));
    {
        let mut s = shared.lock().unwrap();
        // DEVICE_MEDIUM_ANGULAR_MOTOR is both tacho and absolute.
        s.ports[0] = PortInfo { type_id: DEVICE_MEDIUM_ANGULAR_MOTOR, connected: true };
        s.ports[1] = PortInfo { type_id: DEVICE_MEDIUM_ANGULAR_MOTOR, connected: true };
        // combi layout: [speed, position, absolute]
        s.sensor_data.insert("0:0".into(), vec![0.0, 0.0, apos_a]);
        s.sensor_data.insert("1:0".into(), vec![0.0, 0.0, apos_b]);
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
    adapter.shared = shared.clone();
    (adapter, writes, shared)
}

#[test]
fn test_buildhat_rotate_ports_to_abs_both_at_home_is_noop() {
    // Both motors already at APOS=0 → no ramps issued, returns immediately.
    let (mut adapter, writes, _) = make_adapter_with_absolute_motors(0.0, 0.0);
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_to_abs(&commands, 0).unwrap();
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert_eq!(
        count_matching(&writes, "ramp"),
        0,
        "expected no ramp commands, got {:?}",
        writes
    );
}

#[test]
fn test_buildhat_rotate_ports_to_abs_fires_both_ramps_before_waiting() {
    // Both motors at APOS=60 (needing rotation). The batch must issue BOTH
    // `ramp` commands before blocking on completions — i.e. while the main
    // thread is in the wait loop, both ramps should already be visible in
    // the mock transport's writes. Helper thread flips completion flags
    // once it sees both ramps, so the function returns.
    let (mut adapter, writes, shared) = make_adapter_with_absolute_motors(60.0, 60.0);
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];

    let writes_for_flipper = writes.clone();
    let shared_for_flipper = shared.clone();
    let ramps_at_flip = std::thread::spawn(move || {
        // Wait for both ramps to appear in writes.
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let ramp_count = writes_for_flipper
                .lock()
                .unwrap()
                .iter()
                .filter(|w| w.contains("ramp"))
                .count();
            if ramp_count >= 2 {
                break;
            }
            if std::time::Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let ramp_count = writes_for_flipper
            .lock()
            .unwrap()
            .iter()
            .filter(|w| w.contains("ramp"))
            .count();
        // Flip both completions to unblock the wait loop.
        let mut s = shared_for_flipper.lock().unwrap();
        s.completions[0] = true;
        s.completions[1] = true;
        ramp_count
    });

    adapter.rotate_ports_to_abs(&commands, 0).unwrap();
    let ramps_observed = ramps_at_flip.join().unwrap();
    adapter.disconnect();

    assert_eq!(
        ramps_observed, 2,
        "both ramps should be queued before waiting on completions (observed {})",
        ramps_observed
    );
}

#[test]
fn test_buildhat_rotate_ports_by_degrees_fires_both_ramps_before_waiting() {
    // Regression guard: `rotate_ports_by_degrees` must queue every ramp up
    // front and then block on the completion flags, not loop over
    // `rotate_port_by_degrees` (which would serialize the hub). If the
    // override is removed and the default trait impl is used, only one
    // ramp will be observed when the flipper thread samples.
    let (mut adapter, writes, shared) = make_adapter_with_absolute_motors(0.0, 0.0);
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];

    let writes_for_flipper = writes.clone();
    let shared_for_flipper = shared.clone();
    let ramps_at_flip = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let ramp_count = writes_for_flipper
                .lock()
                .unwrap()
                .iter()
                .filter(|w| w.contains("ramp"))
                .count();
            if ramp_count >= 2 || std::time::Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let ramp_count = writes_for_flipper
            .lock()
            .unwrap()
            .iter()
            .filter(|w| w.contains("ramp"))
            .count();
        let mut s = shared_for_flipper.lock().unwrap();
        s.completions[0] = true;
        s.completions[1] = true;
        ramp_count
    });

    adapter.rotate_ports_by_degrees(&commands, 360).unwrap();
    let ramps_observed = ramps_at_flip.join().unwrap();
    adapter.disconnect();

    assert_eq!(
        ramps_observed, 2,
        "both ramps should be queued before waiting on completions (observed {})",
        ramps_observed
    );
}

#[test]
fn test_buildhat_rotate_ports_to_position_fires_both_ramps_before_waiting() {
    // Regression guard, same as above but for the relative-position variant.
    let (mut adapter, writes, shared) = make_adapter_with_absolute_motors(0.0, 0.0);
    // Seed non-zero relative positions so both motors need a rotation delta.
    {
        let mut s = shared.lock().unwrap();
        s.sensor_data.insert("0:0".into(), vec![0.0, 90.0, 0.0]);
        s.sensor_data.insert("1:0".into(), vec![0.0, 90.0, 0.0]);
    }
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];

    let writes_for_flipper = writes.clone();
    let shared_for_flipper = shared.clone();
    let ramps_at_flip = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let ramp_count = writes_for_flipper
                .lock()
                .unwrap()
                .iter()
                .filter(|w| w.contains("ramp"))
                .count();
            if ramp_count >= 2 || std::time::Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let ramp_count = writes_for_flipper
            .lock()
            .unwrap()
            .iter()
            .filter(|w| w.contains("ramp"))
            .count();
        let mut s = shared_for_flipper.lock().unwrap();
        s.completions[0] = true;
        s.completions[1] = true;
        ramp_count
    });

    adapter.rotate_ports_to_position(&commands, 0).unwrap();
    let ramps_observed = ramps_at_flip.join().unwrap();
    adapter.disconnect();

    assert_eq!(
        ramps_observed, 2,
        "both ramps should be queued before waiting on completions (observed {})",
        ramps_observed
    );
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
