use super::*;
use rust_coral::constants::CoralDeviceKind;
use rust_coral::coral::Coral;
use std::sync::{Arc, Mutex};

/// Mock Coral BLE: records all sent frames and request_all batches.
struct MockCoralBle {
    coral: Coral,
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    request_calls: Arc<Mutex<Vec<Vec<u8>>>>,
    request_all_calls: Arc<Mutex<Vec<Vec<(u8, u8, Vec<u8>)>>>>,
}

impl MockCoralBle {
    fn new(kind: CoralDeviceKind) -> Self {
        let mut coral = Coral::new();
        coral.on_connected(kind);
        MockCoralBle {
            coral,
            sent: Arc::new(Mutex::new(Vec::new())),
            request_calls: Arc::new(Mutex::new(Vec::new())),
            request_all_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CoralBleHandle for MockCoralBle {
    fn coral(&self) -> &Coral { &self.coral }
    fn is_connected(&self) -> bool { true }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) {}
    fn send(&self, data: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(data.to_vec());
        Ok(())
    }
    fn request(&mut self, data: &[u8]) -> Result<(), String> {
        self.request_calls.lock().unwrap().push(data.to_vec());
        Ok(())
    }
    fn request_all(&mut self, commands: &[(u8, u8, Vec<u8>)]) -> Result<(), String> {
        self.request_all_calls
            .lock()
            .unwrap()
            .push(commands.to_vec());
        Ok(())
    }
    fn poll(&mut self) -> Result<(), String> { Ok(()) }
    fn set_stop_flag(&mut self, _flag: Arc<std::sync::atomic::AtomicBool>) {}
}

struct MockHandles {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    request_calls: Arc<Mutex<Vec<Vec<u8>>>>,
    request_all_calls: Arc<Mutex<Vec<Vec<(u8, u8, Vec<u8>)>>>>,
}

fn make_double_motor_adapter() -> (CoralAdapter, MockHandles) {
    let mock = MockCoralBle::new(CoralDeviceKind::DoubleMotor);
    let handles = MockHandles {
        sent: mock.sent.clone(),
        request_calls: mock.request_calls.clone(),
        request_all_calls: mock.request_all_calls.clone(),
    };
    let mut adapter = CoralAdapter {
        ble: Box::new(mock),
        output_ports: vec!["a".to_string(), "b".to_string()],
        port_modes: HashMap::new(),
        display_name: "Coral Double Motor".to_string(),
        is_double_motor: true,
    };
    adapter
        .port_modes
        .insert("a".into(), vec!["rotation".into(), "speed".into()]);
    adapter
        .port_modes
        .insert("b".into(), vec!["rotation".into(), "speed".into()]);
    (adapter, handles)
}

#[test]
fn test_coral_run_ports_for_time_physically_same_dir_uses_combined_mask() {
    // On the Double Motor, port "a" is physically mirrored: Logo-Even on a
    // maps to counterclockwise, Logo-Even on b maps to clockwise. So to
    // make BOTH motors physically run clockwise, the test asks for a=Odd
    // (counter-mirror) and b=Even. That's the "same physical direction"
    // case where the adapter collapses to a single combined-mask request.
    let (mut adapter, handles) = make_double_motor_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Odd, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap();

    let request_calls = handles.request_calls.lock().unwrap();
    let request_all_calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(
        request_calls.len(),
        1,
        "expected 1 combined request, got {}",
        request_calls.len()
    );
    assert!(
        request_all_calls.is_empty(),
        "should not use request_all when physical directions match"
    );
}

#[test]
fn test_coral_run_ports_for_time_different_directions_uses_request_all() {
    // Logo-Even on both ports: physically these rotate in opposite
    // directions on the Double Motor (port "a" is mirrored). The adapter
    // falls back to request_all because the underlying motor directions
    // differ.
    let (mut adapter, handles) = make_double_motor_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap();

    let request_all_calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(
        request_all_calls.len(),
        1,
        "expected 1 request_all batch, got {}",
        request_all_calls.len()
    );
    assert_eq!(request_all_calls[0].len(), 2, "batch should contain both ports");
}

#[test]
fn test_coral_rotate_ports_by_degrees_batches() {
    let (mut adapter, handles) = make_double_motor_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 90).unwrap();

    let request_all_calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(request_all_calls.len(), 1, "expected 1 request_all batch");
    assert_eq!(request_all_calls[0].len(), 2);
}

#[test]
fn test_coral_rotate_ports_to_home_batches() {
    let (mut adapter, handles) = make_double_motor_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_to_home(&commands).unwrap();

    let request_all_calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(request_all_calls.len(), 1, "expected 1 request_all batch");
    assert_eq!(request_all_calls[0].len(), 2);
}

#[test]
fn test_coral_single_port_not_degraded() {
    let (mut adapter, handles) = make_double_motor_adapter();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 50,
    }];
    adapter.rotate_ports_by_degrees(&commands, 90).unwrap();

    let request_all_calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(request_all_calls.len(), 1);
    assert_eq!(request_all_calls[0].len(), 1);
}

#[test]
fn test_coral_per_port_speed_set_before_batch() {
    // Each port gets its own cmd_set_motor_speed sent via plain `send`
    // before the rotation batch fires.
    let (mut adapter, handles) = make_double_motor_adapter();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 75 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 90).unwrap();

    let sent = handles.sent.lock().unwrap();
    // 2 speed sets, one per port.
    assert_eq!(
        sent.len(),
        2,
        "expected 2 speed-set frames before batch, got {}",
        sent.len()
    );
}
