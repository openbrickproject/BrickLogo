use super::*;
use crate::adapter::{PortCommand, PortDirection};
use crate::scheduler;
use rust_spike::protocol::*;
use std::sync::{Arc, Mutex, mpsc};

/// Mock serial transport — records writes, injects OK responses.
struct MockTransport {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl SpikeTransport for MockTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        // Return OK\x04\x04 for every command (immediate success, no stdout)
        let response = b"OK\x04\x04";
        let n = buf.len().min(response.len());
        buf[..n].copy_from_slice(&response[..n]);
        Ok(n)
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.writes.lock().unwrap().push(data.to_vec());
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn make_adapter_with_mock() -> (SpikeAdapter, Arc<Mutex<Vec<Vec<u8>>>>) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn SpikeTransport> = Box::new(MockTransport {
        writes: writes.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let slot = SpikeSlot {
        port: transport,
        rx,
        response_buf: Vec::new(),
        alive: true,
        pending: None,
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = SpikeAdapter::new(None);
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    (adapter, writes)
}

fn wait_for_writes(writes: &Arc<Mutex<Vec<Vec<u8>>>>, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if writes.lock().unwrap().len() >= count {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn writes_contain(writes: &[Vec<u8>], needle: &str) -> bool {
    writes.iter().any(|w| {
        let s = String::from_utf8_lossy(w);
        s.contains(needle)
    })
}

#[test]
fn test_start_port_sends_motor_run() {
    let (mut adapter, writes) = make_adapter_with_mock();
    adapter.start_port("a", PortDirection::Even, 50).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes_contain(&writes, "motor.run(port.A, 500)"));
}

#[test]
fn test_stop_port_sends_motor_stop() {
    let (mut adapter, writes) = make_adapter_with_mock();
    adapter.stop_port("b").unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes_contain(&writes, "motor.stop(port.B)"));
}

#[test]
fn test_direction_mapping() {
    let (mut adapter, writes) = make_adapter_with_mock();
    adapter.start_port("a", PortDirection::Even, 80).unwrap();
    adapter.start_port("b", PortDirection::Odd, 60).unwrap();
    wait_for_writes(&writes, 2);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    // Even: positive velocity
    assert!(writes_contain(&writes, "motor.run(port.A, 800)"));
    // Odd: negative velocity
    assert!(writes_contain(&writes, "motor.run(port.B, -600)"));
}

#[test]
fn test_rotate_sends_runloop() {
    let (mut adapter, writes) = make_adapter_with_mock();
    adapter.rotate_port_by_degrees("c", PortDirection::Even, 50, 360).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes_contain(&writes, "runloop.run(motor.run_for_degrees(port.C, 360, 500))"));
}

#[test]
fn test_parallel_rotate_uses_gather() {
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Odd, power: 50 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 360).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes_contain(&writes, "runloop.run("));
    assert!(writes_contain(&writes, "motor.run_for_degrees(port.A, 360, 500)"));
    assert!(writes_contain(&writes, "motor.run_for_degrees(port.B, 360, -500)"));
}

#[test]
fn test_parallel_onfor_uses_gather() {
    let (mut adapter, writes) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 75 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 75 },
    ];
    adapter.run_ports_for_time(&commands, 10).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes_contain(&writes, "runloop.run("));
    assert!(writes_contain(&writes, "motor.run_for_time(port.A, 1000, 750)"));
    assert!(writes_contain(&writes, "motor.run_for_time(port.B, 1000, 750)"));
}

#[test]
fn test_reset_zero() {
    let (mut adapter, writes) = make_adapter_with_mock();
    adapter.reset_port_zero("d").unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes_contain(&writes, "motor.reset_relative_position(port.D, 0)"));
}

#[test]
fn test_rotate_to_abs() {
    let (mut adapter, writes) = make_adapter_with_mock();
    adapter.rotate_to_abs("e", PortDirection::Even, 50, 90).unwrap();
    wait_for_writes(&writes, 1);
    adapter.disconnect();

    let writes = writes.lock().unwrap();
    assert!(writes_contain(&writes, "motor.run_to_absolute_position(port.E, 90, 500, direction=0)"));
}

#[test]
fn test_validate_ports() {
    let (adapter, _) = make_adapter_with_mock();
    assert!(adapter.validate_output_port("a").is_ok());
    assert!(adapter.validate_output_port("f").is_ok());
    assert!(adapter.validate_output_port("g").is_err());
    assert!(adapter.validate_sensor_port("a", Some("rotation")).is_ok());
    assert!(adapter.validate_sensor_port("z", None).is_err());
}
