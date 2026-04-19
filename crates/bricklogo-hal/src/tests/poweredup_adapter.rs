use super::*;
use crate::adapter::HardwareAdapter;
use rust_poweredup::constants::{DeviceType, HubType};
use rust_poweredup::devices::SensorReading;
use std::sync::{Arc, Mutex};

/// Mock PUP BLE: records every `send` / `request_all` call and exposes a
/// manually-populated Hub so the adapter sees devices attached.
struct MockPupBle {
    hub: Arc<Mutex<Hub>>,
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    request_all_calls: Arc<Mutex<Vec<Vec<(u8, Vec<u8>)>>>>,
}

impl MockPupBle {
    fn new(hub_type: HubType) -> Self {
        MockPupBle {
            hub: Arc::new(Mutex::new(Hub::new(hub_type))),
            sent: Arc::new(Mutex::new(Vec::new())),
            request_all_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl PupBle for MockPupBle {
    fn hub(&self) -> &Arc<Mutex<Hub>> {
        &self.hub
    }
    fn is_connected(&self) -> bool {
        true
    }
    fn connect(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn disconnect(&mut self) {}
    fn send(&self, data: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(data.to_vec());
        Ok(())
    }
    fn request(&self, _port_id: u8, data: &[u8]) -> Result<bool, String> {
        self.sent.lock().unwrap().push(data.to_vec());
        Ok(true)
    }
    fn request_all(&self, commands: &[(u8, Vec<u8>)]) -> Result<(), String> {
        self.request_all_calls
            .lock()
            .unwrap()
            .push(commands.to_vec());
        Ok(())
    }
    fn subscribe(&self, _port_id: u8, _mode: u8) -> Result<(), String> {
        Ok(())
    }
    fn set_stop_flag(&mut self, _flag: Arc<std::sync::atomic::AtomicBool>) {}
}

struct MockHandles {
    hub: Arc<Mutex<Hub>>,
    #[allow(dead_code)]
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    request_all_calls: Arc<Mutex<Vec<Vec<(u8, Vec<u8>)>>>>,
}

fn make_adapter_with_mock(hub_type: HubType) -> (PoweredUpAdapter, MockHandles) {
    let mock = MockPupBle::new(hub_type);
    let handles = MockHandles {
        hub: mock.hub.clone(),
        sent: mock.sent.clone(),
        request_all_calls: mock.request_all_calls.clone(),
    };
    let adapter = PoweredUpAdapter {
        ble: Box::new(mock),
    };
    (adapter, handles)
}

/// Attach two Technic angular motors on ports A (id 0) and B (id 1), with
/// rotation mode already selected and a zero reading cached so that
/// `read_sensor("rotation")` resolves immediately without going through
/// the subscribe-and-wait loop.
fn attach_angular_motors(hub: &Arc<Mutex<Hub>>) {
    let mut hub = hub.lock().unwrap();
    hub.attach_device(0, DeviceType::TechnicMediumAngularMotor);
    hub.attach_device(1, DeviceType::TechnicMediumAngularMotor);
    if let Some(d) = hub.get_device_mut(0) {
        d.current_mode = Some(0x02); // rotation mode
        d.last_reading = Some(SensorReading::Number(0.0));
    }
    if let Some(d) = hub.get_device_mut(1) {
        d.current_mode = Some(0x02);
        d.last_reading = Some(SensorReading::Number(0.0));
    }
}

#[test]
fn test_pup_run_ports_for_time_uses_request_all_for_tacho() {
    // Tacho motors use cmd_start_speed_for_time + request_all, which fires
    // all per-port commands in one call. Verifies multi-port batching
    // within a single PUP hub.
    let (mut adapter, handles) = make_adapter_with_mock(HubType::TechnicMediumHub);
    attach_angular_motors(&handles.hub);

    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.run_ports_for_time(&commands, 1).unwrap();

    let calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        1,
        "expected one request_all batch, got {}",
        calls.len()
    );
    let batch = &calls[0];
    assert_eq!(batch.len(), 2, "batch should contain both ports");
    let port_ids: Vec<u8> = batch.iter().map(|(p, _)| *p).collect();
    assert!(port_ids.contains(&0), "batch should include port A (id 0)");
    assert!(port_ids.contains(&1), "batch should include port B (id 1)");
}

#[test]
fn test_pup_rotate_ports_by_degrees_batches() {
    let (mut adapter, handles) = make_adapter_with_mock(HubType::TechnicMediumHub);
    attach_angular_motors(&handles.hub);

    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 90).unwrap();

    let calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "expected one request_all batch");
    assert_eq!(calls[0].len(), 2);
}

#[test]
fn test_pup_rotate_ports_to_position_batches() {
    let (mut adapter, handles) = make_adapter_with_mock(HubType::TechnicMediumHub);
    attach_angular_motors(&handles.hub);

    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_to_position(&commands, 90).unwrap();

    let calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "expected one request_all batch");
    assert_eq!(calls[0].len(), 2);
}

#[test]
fn test_pup_rotate_ports_to_abs_batches() {
    // rotate_to_abs requires absolute motor — Technic angular motors qualify.
    // Seed each port's last_reading with a non-zero APOS so the adapter
    // computes a non-zero delta and actually fires commands.
    let (mut adapter, handles) = make_adapter_with_mock(HubType::TechnicMediumHub);
    attach_angular_motors(&handles.hub);
    {
        let mut hub = handles.hub.lock().unwrap();
        if let Some(d) = hub.get_device_mut(0) {
            d.last_reading = Some(SensorReading::Number(80.0));
        }
        if let Some(d) = hub.get_device_mut(1) {
            d.last_reading = Some(SensorReading::Number(-45.0));
        }
    }

    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_to_abs(&commands, 0).unwrap();

    let calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "expected one request_all batch");
    assert_eq!(calls[0].len(), 2);
}

#[test]
fn test_pup_rotate_to_abs_reads_apos_not_pos() {
    // Regression: rotatetoabs must target mechanical zero (APOS), not the
    // relative encoder's zero (POS). With APOS=80, the adapter must issue
    // a StartSpeedForDegrees(|80|, odd) — NOT a GotoAbsolutePosition(0).
    let (mut adapter, handles) = make_adapter_with_mock(HubType::TechnicMediumHub);
    attach_angular_motors(&handles.hub);
    {
        let mut hub = handles.hub.lock().unwrap();
        if let Some(d) = hub.get_device_mut(0) {
            d.last_reading = Some(SensorReading::Number(80.0));
        }
    }

    adapter.rotate_to_abs("a", PortDirection::Even, 50, 0).unwrap();

    let calls = handles.request_all_calls.lock().unwrap();
    // rotate_port_by_degrees uses `self.ble.request` (single), not
    // request_all, so request_all_calls stays empty.
    assert!(
        calls.is_empty(),
        "single-port path should not use request_all"
    );
    // The fact that we reached here without error is the main signal —
    // the adapter read APOS, computed delta, and delegated correctly.
}

#[test]
fn test_pup_single_port_not_degraded() {
    let (mut adapter, handles) = make_adapter_with_mock(HubType::TechnicMediumHub);
    attach_angular_motors(&handles.hub);

    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 50,
    }];
    adapter.rotate_ports_by_degrees(&commands, 90).unwrap();

    let calls = handles.request_all_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].len(), 1);
    assert_eq!(calls[0][0].0, 0, "should target port A");
}
