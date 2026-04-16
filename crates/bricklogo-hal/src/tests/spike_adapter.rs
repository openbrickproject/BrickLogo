use super::*;
use crate::adapter::{PortCommand, PortDirection};
use crate::scheduler;
use rust_spike::constants::*;
use rust_spike::protocol::*;
use std::sync::{Arc, Mutex, mpsc};

/// Mock serial transport — records writes, can inject reads.
struct MockTransport {
    writes: Arc<Mutex<Vec<String>>>,
    read_data: Arc<Mutex<Vec<u8>>>,
}

impl SpikeTransport for MockTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        let mut data = self.read_data.lock().unwrap();
        let n = buf.len().min(data.len());
        if n > 0 {
            buf[..n].copy_from_slice(&data[..n]);
            data.drain(..n);
        }
        Ok(n)
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

fn make_adapter_with_mock() -> (SpikeAdapter, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<u8>>>) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let read_data = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn SpikeTransport> = Box::new(MockTransport {
        writes: writes.clone(),
        read_data: read_data.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(SpikeShared::new()));
    // Pre-populate ports: large angular motor on A and B
    {
        let mut s = shared.lock().unwrap();
        s.ports[0].device_type = DEVICE_LARGE_ANGULAR_MOTOR;
        s.ports[1].device_type = DEVICE_LARGE_ANGULAR_MOTOR;
    }
    let slot = SpikeSlot {
        port: transport,
        rx,
        shared: shared.clone(),
        read_buffer: String::new(),
        alive: true,
        task_id_gen: TaskIdGen::new(),
        pending: Vec::new(),
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = SpikeAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.shared = shared;
    adapter.slot_id = Some(slot_id);
    (adapter, writes, read_data)
}

fn wait_for_writes(writes: &Arc<Mutex<Vec<String>>>, count: usize) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if writes.lock().unwrap().len() >= count {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn test_start_port_sends_motor_start() {
    let (mut adapter, writes, _) = make_adapter_with_mock();
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes.iter().any(|w| w.contains("scratch.motor_start") && w.contains("\"A\"")));
}

#[test]
fn test_stop_port_sends_motor_stop() {
    let (mut adapter, writes, _) = make_adapter_with_mock();
    adapter.stop_port("a").unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes.iter().any(|w| w.contains("scratch.motor_stop")));
}

#[test]
fn test_start_ports_two_uses_dual_motor_command() {
    let (mut adapter, writes, _) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Odd, power: 75 },
    ];
    adapter.start_ports(&commands).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes.iter().any(|w| w.contains("scratch.move_start_speeds")));
}

#[test]
fn test_stop_ports_two_uses_dual_motor_command() {
    let (mut adapter, writes, _) = make_adapter_with_mock();
    adapter.stop_ports(&["a", "b"]).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes.iter().any(|w| w.contains("scratch.move_stop")));
}

#[test]
fn test_motor_set_position_sends_reset() {
    let (mut adapter, writes, _) = make_adapter_with_mock();
    adapter.reset_port_zero("a").unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes.iter().any(|w| w.contains("scratch.motor_set_position")));
}

#[test]
fn test_direction_mapping() {
    let (mut adapter, writes, _) = make_adapter_with_mock();
    // Even → positive speed
    adapter.start_port("a", PortDirection::Even, 80).unwrap();
    // Odd → negative speed
    adapter.start_port("b", PortDirection::Odd, 60).unwrap();
    wait_for_writes(&writes, 2);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    let a_cmd = writes.iter().find(|w| w.contains("\"A\"")).unwrap();
    assert!(a_cmd.contains("\"speed\":80"));
    let b_cmd = writes.iter().find(|w| w.contains("\"B\"")).unwrap();
    assert!(b_cmd.contains("\"speed\":-60"));
}

#[test]
fn test_read_sensor_from_cached_telemetry() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    // Inject motor data: [speed=10, rel_pos=180, abs_pos=45, power=50]
    {
        let mut shared = adapter.shared.lock().unwrap();
        shared.ports[0].device_type = DEVICE_LARGE_ANGULAR_MOTOR;
        shared.ports[0].data = [10.0, 180.0, 45.0, 50.0];
    }
    let rot = adapter.read_sensor("a", Some("rotation")).unwrap().unwrap();
    assert_eq!(rot, LogoValue::Number(180.0));
    let speed = adapter.read_sensor("a", Some("speed")).unwrap().unwrap();
    assert_eq!(speed, LogoValue::Number(10.0));
    let abs = adapter.read_sensor("a", Some("absolute")).unwrap().unwrap();
    assert_eq!(abs, LogoValue::Number(45.0));

    adapter.disconnect();
}

#[test]
fn test_read_sensor_imu() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    {
        let mut shared = adapter.shared.lock().unwrap();
        shared.imu.yaw_pitch_roll = [10.0, 20.0, 30.0];
        shared.imu.gyro = [1.0, 2.0, 3.0];
        shared.imu.accel = [4.0, 5.0, 6.0];
    }
    let tilt = adapter.read_sensor("tilt", None).unwrap().unwrap();
    assert_eq!(tilt, LogoValue::List(vec![LogoValue::Number(20.0), LogoValue::Number(30.0)]));

    let gyro = adapter.read_sensor("gyro", None).unwrap().unwrap();
    assert_eq!(gyro, LogoValue::List(vec![
        LogoValue::Number(1.0), LogoValue::Number(2.0), LogoValue::Number(3.0),
    ]));

    adapter.disconnect();
}

#[test]
fn test_require_tacho_rejects_non_motor() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    {
        let mut shared = adapter.shared.lock().unwrap();
        shared.ports[2].device_type = DEVICE_COLOR_SENSOR; // port C = color sensor
    }
    let err = adapter.rotate_port_by_degrees("c", PortDirection::Even, 50, 90);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("not a tacho motor"));
    adapter.disconnect();
}

#[test]
fn test_require_absolute_rejects_tacho_only() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    {
        let mut shared = adapter.shared.lock().unwrap();
        shared.ports[0].device_type = DEVICE_MEDIUM_LINEAR_MOTOR; // tacho but not absolute
    }
    let err = adapter.rotate_to_home("a", PortDirection::Even, 50);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("absolute position"));
    adapter.disconnect();
}

#[test]
fn test_validate_ports() {
    let (adapter, _, _) = make_adapter_with_mock();
    assert!(adapter.validate_output_port("a").is_ok());
    assert!(adapter.validate_output_port("f").is_ok());
    assert!(adapter.validate_output_port("g").is_err());

    assert!(adapter.validate_sensor_port("a", Some("rotation")).is_ok());
    assert!(adapter.validate_sensor_port("tilt", None).is_ok());
    assert!(adapter.validate_sensor_port("gyro", None).is_ok());
    assert!(adapter.validate_sensor_port("z", None).is_err());
}
