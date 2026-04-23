use super::*;
use crate::adapter::{HardwareAdapter, PortDirection};
use crate::port_manager::PortManager;
use bricklogo_lang::value::LogoValue;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Minimal adapter whose `connected()` flips based on an external
/// `AtomicBool`, so tests can simulate a mid-session disconnect without
/// spinning up real hardware or BLE.
struct FlippableAdapter {
    name: String,
    ports: Vec<String>,
    alive: Arc<AtomicBool>,
}

impl FlippableAdapter {
    fn new(name: &str, alive: Arc<AtomicBool>) -> Self {
        FlippableAdapter {
            name: name.to_string(),
            ports: vec!["a".to_string()],
            alive,
        }
    }
}

impl HardwareAdapter for FlippableAdapter {
    fn display_name(&self) -> &str { &self.name }
    fn output_ports(&self) -> &[String] { &self.ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.alive.load(Ordering::SeqCst) }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) { self.alive.store(false, Ordering::SeqCst); }
    fn validate_output_port(&self, _: &str) -> Result<(), String> { Ok(()) }
    fn validate_sensor_port(&self, _: &str, _: Option<&str>) -> Result<(), String> { Ok(()) }
    fn max_power(&self) -> u8 { 100 }
    fn start_port(&mut self, _: &str, _: PortDirection, _: u8) -> Result<(), String> { Ok(()) }
    fn stop_port(&mut self, _: &str) -> Result<(), String> { Ok(()) }
    fn run_port_for_time(&mut self, _: &str, _: PortDirection, _: u8, _: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _: &str, _: PortDirection, _: u8, _: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _: &str, _: PortDirection, _: u8, _: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _: &str, _: PortDirection, _: u8, _: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _: &str, _: Option<&str>) -> Result<Option<LogoValue>, String> {
        Ok(None)
    }
}

fn pm_with(adapters: Vec<(&str, &str, Arc<AtomicBool>)>) -> Arc<Mutex<PortManager>> {
    let pm = Arc::new(Mutex::new(PortManager::new()));
    {
        let mut guard = pm.lock().unwrap();
        for (name, ty, alive) in adapters {
            guard.add_device(
                name,
                Box::new(FlippableAdapter::new(name, alive.clone())),
                ty,
            );
        }
    }
    pm
}

fn fake_system_fn() -> (Arc<dyn Fn(&str) + Send + Sync>, Arc<Mutex<Vec<String>>>) {
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let inner = captured.clone();
    let f: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |s: &str| {
        inner.lock().unwrap().push(s.to_string());
    });
    (f, captured)
}

// ── Happy path: everything connected, tick is a no-op ─────

#[test]
fn test_tick_noop_when_all_devices_connected() {
    let alive = Arc::new(AtomicBool::new(true));
    let pm = pm_with(vec![("robot", "science", alive)]);
    let (sys_fn, captured) = fake_system_fn();

    let flow = super::tick(&pm, &*sys_fn);
    assert!(matches!(flow, std::ops::ControlFlow::Continue(())));
    assert!(captured.lock().unwrap().is_empty(), "no dead device, no message");
    assert!(pm.lock().unwrap().get_connected_device_names().contains(&"robot".to_string()));
}

// ── Dead device gets removed and a message emitted ────────

#[test]
fn test_tick_removes_dead_device_and_emits_message() {
    let alive = Arc::new(AtomicBool::new(false)); // starts disconnected
    let pm = pm_with(vec![("robot", "science", alive)]);
    let (sys_fn, captured) = fake_system_fn();

    super::tick(&pm, &*sys_fn);

    let msgs = captured.lock().unwrap().clone();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].contains("robot"));
    assert!(msgs[0].contains("lost connection"));
    assert!(pm.lock().unwrap().get_connected_device_names().is_empty());
}

// ── Only the dead device is removed; live ones stay ───────

#[test]
fn test_tick_preserves_live_devices() {
    let alive_live = Arc::new(AtomicBool::new(true));
    let alive_dead = Arc::new(AtomicBool::new(false));
    let pm = pm_with(vec![
        ("keeper", "science", alive_live),
        ("gone", "science", alive_dead),
    ]);
    let (sys_fn, captured) = fake_system_fn();

    super::tick(&pm, &*sys_fn);

    let names = pm.lock().unwrap().get_connected_device_names();
    assert_eq!(names, vec!["keeper".to_string()]);
    assert_eq!(captured.lock().unwrap().len(), 1);
    assert!(captured.lock().unwrap()[0].contains("gone"));
}

// ── Poisoned lock → daemon exits ──────────────────────────

#[test]
fn test_tick_breaks_on_poisoned_lock() {
    let alive = Arc::new(AtomicBool::new(true));
    let pm = pm_with(vec![("robot", "science", alive)]);
    let (sys_fn, _) = fake_system_fn();

    // Poison the mutex.
    let pm_for_panic = pm.clone();
    let _ = std::thread::spawn(move || {
        let _guard = pm_for_panic.lock().unwrap();
        panic!("intentional poison");
    })
    .join();
    assert!(pm.is_poisoned());

    let flow = super::tick(&pm, &*sys_fn);
    assert!(matches!(flow, std::ops::ControlFlow::Break(())));
}

// ── Mid-session flip: dead-now, wasn't-before ─────────────

#[test]
fn test_tick_detects_device_that_drops_between_ticks() {
    let alive = Arc::new(AtomicBool::new(true));
    let pm = pm_with(vec![("robot", "science", alive.clone())]);
    let (sys_fn, captured) = fake_system_fn();

    // First tick: device still alive, no action.
    super::tick(&pm, &*sys_fn);
    assert!(captured.lock().unwrap().is_empty());

    // Simulate BLE dropping.
    alive.store(false, Ordering::SeqCst);

    // Second tick: reconciliation fires.
    super::tick(&pm, &*sys_fn);
    assert_eq!(captured.lock().unwrap().len(), 1);
    assert!(pm.lock().unwrap().get_connected_device_names().is_empty());
}
