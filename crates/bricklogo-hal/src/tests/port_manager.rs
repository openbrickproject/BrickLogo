use super::*;
use std::time::{Duration, Instant};

struct MockAdapter {
    ports: Vec<String>,
    connected: bool,
    start_calls: Vec<(String, PortDirection, u8)>,
    stop_calls: Vec<String>,
}

impl MockAdapter {
    fn new(ports: &[&str]) -> Self {
        MockAdapter {
            ports: ports.iter().map(|s| s.to_string()).collect(),
            connected: true,
            start_calls: Vec::new(),
            stop_calls: Vec::new(),
        }
    }
}

impl HardwareAdapter for MockAdapter {
    fn display_name(&self) -> &str { "Mock" }
    fn output_ports(&self) -> &[String] { &self.ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.connected }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) { self.connected = false; }
    fn validate_output_port(&self, _port: &str) -> Result<(), String> { Ok(()) }
    fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> { Ok(()) }
    fn max_power(&self) -> u8 { 100 }
    fn start_port(&mut self, port: &str, dir: PortDirection, power: u8) -> Result<(), String> {
        self.start_calls.push((port.to_string(), dir, power));
        Ok(())
    }
    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        self.stop_calls.push(port.to_string());
        Ok(())
    }
    fn run_port_for_time(&mut self, _port: &str, _dir: PortDirection, _power: u8, _tenths: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _port: &str, _dir: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _port: &str, _dir: PortDirection, _power: u8, _pos: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _port: &str, _dir: PortDirection, _power: u8, _position: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> { Ok(None) }
}

/// Mock adapter that reports a fixed pulse count per input port, so multi-port
/// `read_counter` can be exercised. Mirrors Interface A, the only real adapter
/// that implements counters.
struct CounterAdapter {
    ports: Vec<String>,
    counts: std::collections::HashMap<String, u32>,
    connected: bool,
}

impl CounterAdapter {
    fn new(counts: &[(&str, u32)]) -> Self {
        CounterAdapter {
            ports: counts.iter().map(|(p, _)| p.to_string()).collect(),
            counts: counts.iter().map(|(p, c)| (p.to_string(), *c)).collect(),
            connected: true,
        }
    }

    fn disconnected(mut self) -> Self {
        self.connected = false;
        self
    }
}

impl HardwareAdapter for CounterAdapter {
    fn display_name(&self) -> &str { "Counter" }
    fn output_ports(&self) -> &[String] { &[] }
    fn input_ports(&self) -> &[String] { &self.ports }
    fn connected(&self) -> bool { self.connected }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) { self.connected = false; }
    fn validate_output_port(&self, _port: &str) -> Result<(), String> { Ok(()) }
    fn validate_sensor_port(&self, port: &str, _mode: Option<&str>) -> Result<(), String> {
        if self.ports.iter().any(|p| p == port) {
            Ok(())
        } else {
            Err(format!("\"{port}\" is not a valid sensor port"))
        }
    }
    fn max_power(&self) -> u8 { 255 }
    fn start_port(&mut self, _port: &str, _dir: PortDirection, _power: u8) -> Result<(), String> { Ok(()) }
    fn stop_port(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn run_port_for_time(&mut self, _port: &str, _dir: PortDirection, _power: u8, _tenths: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _port: &str, _dir: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _port: &str, _dir: PortDirection, _power: u8, _pos: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _port: &str, _dir: PortDirection, _power: u8, _position: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> { Ok(None) }
    fn read_counter(&mut self, port: &str) -> Result<u32, String> {
        self.counts.get(port).copied()
            .ok_or_else(|| format!("no counter for port {port}"))
    }
}

/// Mock adapter whose batch methods sleep for the requested duration (or a
/// fixed 200ms for the degree/position methods). Used to verify that
/// PortManager fans out work across devices in parallel, not sequentially.
struct SleepyAdapter {
    ports: Vec<String>,
}

impl SleepyAdapter {
    fn new(ports: &[&str]) -> Self {
        SleepyAdapter {
            ports: ports.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl HardwareAdapter for SleepyAdapter {
    fn display_name(&self) -> &str { "Sleepy" }
    fn output_ports(&self) -> &[String] { &self.ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { true }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) {}
    fn validate_output_port(&self, _port: &str) -> Result<(), String> { Ok(()) }
    fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> { Ok(()) }
    fn max_power(&self) -> u8 { 100 }
    fn start_port(&mut self, _port: &str, _dir: PortDirection, _power: u8) -> Result<(), String> { Ok(()) }
    fn stop_port(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn run_port_for_time(&mut self, _port: &str, _dir: PortDirection, _power: u8, _tenths: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _port: &str, _dir: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _port: &str, _dir: PortDirection, _power: u8, _pos: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _port: &str, _dir: PortDirection, _power: u8, _position: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> { Ok(None) }

    fn start_ports(&mut self, _commands: &[PortCommand]) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
    fn stop_ports(&mut self, _ports: &[&str]) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
    fn run_ports_for_time(&mut self, _commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
        Ok(())
    }
    fn rotate_ports_by_degrees(&mut self, _commands: &[PortCommand], _degrees: i32) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
    fn rotate_ports_to_position(&mut self, _commands: &[PortCommand], _position: i32) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
    fn rotate_ports_to_abs(&mut self, _commands: &[PortCommand], _position: i32) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
}

/// Like SleepyAdapter but overrides parallel_safe() to false — simulates an
/// IR-based adapter (Power Functions) where concurrent transmissions interfere.
struct SlowIRAdapter {
    ports: Vec<String>,
}

impl SlowIRAdapter {
    fn new(ports: &[&str]) -> Self {
        SlowIRAdapter { ports: ports.iter().map(|s| s.to_string()).collect() }
    }
}

impl HardwareAdapter for SlowIRAdapter {
    fn display_name(&self) -> &str { "SlowIR" }
    fn output_ports(&self) -> &[String] { &self.ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { true }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) {}
    fn validate_output_port(&self, _port: &str) -> Result<(), String> { Ok(()) }
    fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> { Ok(()) }
    fn max_power(&self) -> u8 { 7 }
    fn start_port(&mut self, _port: &str, _dir: PortDirection, _power: u8) -> Result<(), String> { Ok(()) }
    fn stop_port(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn run_port_for_time(&mut self, _port: &str, _dir: PortDirection, _power: u8, _tenths: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _port: &str, _dir: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _port: &str, _dir: PortDirection, _power: u8, _pos: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _port: &str, _dir: PortDirection, _power: u8, _position: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> { Ok(None) }
    fn parallel_safe(&self) -> bool { false }
    fn start_ports(&mut self, _commands: &[PortCommand]) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
    fn stop_ports(&mut self, _ports: &[&str]) -> Result<(), String> {
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
}

#[test]
fn test_first_device_becomes_active() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])), "mock");
    assert_eq!(pm.get_active_device_name(), Some("bot"));
}

#[test]
fn test_second_device_not_active() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])), "mock");
    assert_eq!(pm.get_active_device_name(), Some("bot1"));
    assert_eq!(pm.get_connected_device_names(), vec!["bot1".to_string(), "bot2".to_string()]);
}

#[test]
fn test_use_switches_active() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.set_active_device("bot2").unwrap();
    assert_eq!(pm.get_active_device_name(), Some("bot2"));
}

#[test]
fn test_remove_device_fallback() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.remove_device("bot1");
    assert_eq!(pm.get_active_device_name(), Some("bot2"));
    assert_eq!(pm.get_connected_device_names(), vec!["bot2".to_string()]);
}

#[test]
fn test_ensure_port_states() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])), "mock");
    pm.ensure_port_states(&["a".to_string()]).unwrap();
}

#[test]
fn test_ensure_port_states_qualified() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])), "mock");
    pm.ensure_port_states(&["bot.a".to_string()]).unwrap();
}

#[test]
fn test_on_off() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])), "mock");
    let ports = vec!["a".to_string(), "b".to_string()];
    pm.ensure_port_states(&ports).unwrap();
    pm.on(&ports).unwrap();
    pm.off(&ports).unwrap();
}

#[test]
fn test_set_power() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a"])), "mock");
    let ports = vec!["a".to_string()];
    pm.ensure_port_states(&ports).unwrap();
    pm.set_power(&ports, 8);
    pm.on(&ports).unwrap();
}

#[test]
fn test_all_off() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])), "mock");
    let ports = vec!["a".to_string(), "b".to_string()];
    pm.ensure_port_states(&ports).unwrap();
    pm.on(&ports).unwrap();
    pm.all_off();
}

#[test]
fn test_read_sensor_no_port() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a"])), "mock");
    assert!(pm.read_sensor(&[], None).is_err());
}

#[test]
fn test_read_counter_no_port() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(CounterAdapter::new(&[("6", 0)])), "counter");
    assert!(pm.read_counter(&[]).is_err());
}

#[test]
fn test_read_counter_single_port_returns_number() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(CounterAdapter::new(&[("6", 42), ("7", 7)])), "counter");
    let val = pm.read_counter(&["6".to_string()]).unwrap();
    assert_eq!(val, Some(LogoValue::Number(42.0)));
}

#[test]
fn test_read_counter_multi_port_returns_list_in_order() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(CounterAdapter::new(&[("6", 42), ("7", 7)])), "counter");
    let val = pm.read_counter(&["6".to_string(), "7".to_string()]).unwrap();
    assert_eq!(
        val,
        Some(LogoValue::List(vec![
            LogoValue::Number(42.0),
            LogoValue::Number(7.0),
        ]))
    );
}

#[test]
fn test_read_counter_multi_port_preserves_selection_order() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(CounterAdapter::new(&[("6", 42), ("7", 7)])), "counter");
    // Reversed selection should reverse the list, proving it tracks selection
    // order rather than port declaration order.
    let val = pm.read_counter(&["7".to_string(), "6".to_string()]).unwrap();
    assert_eq!(
        val,
        Some(LogoValue::List(vec![
            LogoValue::Number(7.0),
            LogoValue::Number(42.0),
        ]))
    );
}

#[test]
fn test_read_counter_invalid_port_errors() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(CounterAdapter::new(&[("6", 42)])), "counter");
    // Port "9" isn't a valid sensor port — validation must reject the whole read,
    // like read_sensor, rather than reading the good ports and dropping the bad.
    assert!(pm.read_counter(&["6".to_string(), "9".to_string()]).is_err());
}

#[test]
fn test_read_counter_disconnected_device_errors() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(CounterAdapter::new(&[("6", 42)]).disconnected()), "counter");
    assert!(pm.read_counter(&["6".to_string()]).is_err());
}

#[test]
fn test_remove_all() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.remove_all();
    assert!(pm.get_active_device_name().is_none());
    assert!(pm.get_connected_device_names().is_empty());
}

#[test]
fn test_connection_order_preserved_after_use_and_remove() {
    let mut pm = PortManager::new();
    pm.add_device("alpha", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("beta", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("gamma", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.set_active_device("gamma").unwrap();

    assert_eq!(
        pm.get_connected_device_names(),
        vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
    );

    // Removing the active device falls back to the most-recently-used
    // remaining one. With only adds + one `use gamma`, the MRU stack is
    // [alpha, beta, gamma]; removing gamma leaves beta at the top.
    pm.remove_device("gamma");
    assert_eq!(pm.get_active_device_name(), Some("beta"));
    assert_eq!(pm.get_connected_device_names(), vec!["alpha".to_string(), "beta".to_string()]);
}

#[test]
fn test_mru_fallback_after_multiple_use_calls() {
    let mut pm = PortManager::new();
    pm.add_device("alpha", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("beta", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("gamma", Box::new(MockAdapter::new(&["a"])), "mock");
    // use beta, then gamma, then alpha — MRU = [beta, gamma, alpha]
    pm.set_active_device("beta").unwrap();
    pm.set_active_device("gamma").unwrap();
    pm.set_active_device("alpha").unwrap();

    // Removing alpha falls back to gamma (the previous most-recent).
    pm.remove_device("alpha");
    assert_eq!(pm.get_active_device_name(), Some("gamma"));

    // Removing gamma falls back to beta.
    pm.remove_device("gamma");
    assert_eq!(pm.get_active_device_name(), Some("beta"));

    // Removing beta — no devices left.
    pm.remove_device("beta");
    assert_eq!(pm.get_active_device_name(), None);
}

#[test]
fn test_format_port_names() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "mock");
    pm.add_device("bot2", Box::new(MockAdapter::new(&["b"])), "mock");

    // Active device is bot1, so "a" stays short, "bot2.b" stays qualified
    let outputs = vec!["a".to_string(), "bot2.b".to_string()];
    let display = pm.format_port_names(&outputs);
    assert_eq!(display, vec!["a".to_string(), "bot2.b".to_string()]);

    let inputs = vec!["bot2.b".to_string()];
    let display = pm.format_port_names(&inputs);
    assert_eq!(display, vec!["bot2.b".to_string()]);
}

// ── Cross-device parallelism ──────────────────────
//
// Each SleepyAdapter::run_ports_for_time (and rotate/rotate_to/rotate_to_abs)
// sleeps for a fixed duration. With three devices, sequential dispatch would
// take 3× the duration; parallel dispatch should take ~1×. We assert under
// 2× the single-device duration — a generous margin for CI jitter that
// still fails loudly on the original sequential bug.

#[test]
fn test_on_for_runs_across_devices_in_parallel() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("b", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("c", Box::new(SleepyAdapter::new(&["a"])), "mock");
    let ports = vec!["a.a".into(), "b.a".into(), "c.a".into()];
    pm.ensure_port_states(&ports).unwrap();

    let start = Instant::now();
    pm.on_for(&ports, 2).unwrap(); // 200ms per device
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(400),
        "on_for ran sequentially: took {:?}, expected <400ms",
        elapsed
    );
}

#[test]
fn test_rotate_runs_across_devices_in_parallel() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("b", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("c", Box::new(SleepyAdapter::new(&["a"])), "mock");
    let ports = vec!["a.a".into(), "b.a".into(), "c.a".into()];
    pm.ensure_port_states(&ports).unwrap();

    let start = Instant::now();
    pm.rotate(&ports, 90).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(400),
        "rotate ran sequentially: took {:?}, expected <400ms",
        elapsed
    );
}

#[test]
fn test_rotate_to_runs_across_devices_in_parallel() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("b", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("c", Box::new(SleepyAdapter::new(&["a"])), "mock");
    let ports = vec!["a.a".into(), "b.a".into(), "c.a".into()];
    pm.ensure_port_states(&ports).unwrap();

    let start = Instant::now();
    pm.rotate_to(&ports, 0).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(400),
        "rotate_to ran sequentially: took {:?}, expected <400ms",
        elapsed
    );
}

#[test]
fn test_rotate_to_abs_runs_across_devices_in_parallel() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("b", Box::new(SleepyAdapter::new(&["a"])), "mock");
    pm.add_device("c", Box::new(SleepyAdapter::new(&["a"])), "mock");
    let ports = vec!["a.a".into(), "b.a".into(), "c.a".into()];
    pm.ensure_port_states(&ports).unwrap();

    let start = Instant::now();
    pm.rotate_to_abs(&ports, 0).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(400),
        "rotate_to_abs ran sequentially: took {:?}, expected <400ms",
        elapsed
    );
}

#[test]
fn test_on_runs_across_parallel_safe_devices_in_parallel() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SleepyAdapter::new(&["p"])), "mock");
    pm.add_device("b", Box::new(SleepyAdapter::new(&["p"])), "mock");
    pm.add_device("c", Box::new(SleepyAdapter::new(&["p"])), "mock");
    let ports = vec!["a.p".into(), "b.p".into(), "c.p".into()];
    pm.ensure_port_states(&ports).unwrap();

    let start = Instant::now();
    pm.on(&ports).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(400),
        "on ran sequentially across parallel-safe devices: took {:?}, expected <400ms",
        elapsed
    );
}

#[test]
fn test_off_runs_across_parallel_safe_devices_in_parallel() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SleepyAdapter::new(&["p"])), "mock");
    pm.add_device("b", Box::new(SleepyAdapter::new(&["p"])), "mock");
    pm.add_device("c", Box::new(SleepyAdapter::new(&["p"])), "mock");
    let ports = vec!["a.p".into(), "b.p".into(), "c.p".into()];
    pm.ensure_port_states(&ports).unwrap();
    pm.on(&ports).unwrap();

    let start = Instant::now();
    pm.off(&ports).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(400),
        "off ran sequentially across parallel-safe devices: took {:?}, expected <400ms",
        elapsed
    );
}

#[test]
fn test_on_runs_sequentially_for_non_parallel_safe_devices() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SlowIRAdapter::new(&["p"])), "ir");
    pm.add_device("b", Box::new(SlowIRAdapter::new(&["p"])), "ir");
    pm.add_device("c", Box::new(SlowIRAdapter::new(&["p"])), "ir");
    let ports = vec!["a.p".into(), "b.p".into(), "c.p".into()];
    pm.ensure_port_states(&ports).unwrap();

    let start = Instant::now();
    pm.on(&ports).unwrap();
    let elapsed = start.elapsed();

    // Sequential: 3 × 200ms = 600ms; assert it took at least 2 devices worth
    // of time so we know they were NOT fanned out in parallel.
    assert!(
        elapsed >= Duration::from_millis(380),
        "on ran in parallel for non-parallel-safe devices: took {:?}, expected >=380ms",
        elapsed
    );
}

#[test]
fn test_off_runs_sequentially_for_non_parallel_safe_devices() {
    let mut pm = PortManager::new();
    pm.add_device("a", Box::new(SlowIRAdapter::new(&["p"])), "ir");
    pm.add_device("b", Box::new(SlowIRAdapter::new(&["p"])), "ir");
    pm.add_device("c", Box::new(SlowIRAdapter::new(&["p"])), "ir");
    let ports = vec!["a.p".into(), "b.p".into(), "c.p".into()];
    pm.ensure_port_states(&ports).unwrap();
    pm.on(&ports).unwrap();

    let start = Instant::now();
    pm.off(&ports).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(380),
        "off ran in parallel for non-parallel-safe devices: took {:?}, expected >=380ms",
        elapsed
    );
}

// ── Batched state sync (setpower / rd) ──────────────────────────────────────

/// Records each start_ports invocation as one entry, so tests can tell
/// batched dispatch from per-port loops.
struct BatchRecordingAdapter {
    ports: Vec<String>,
    batches: std::sync::Arc<Mutex<Vec<Vec<(String, u8)>>>>,
}

impl HardwareAdapter for BatchRecordingAdapter {
    fn display_name(&self) -> &str { "BatchRec" }
    fn output_ports(&self) -> &[String] { &self.ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { true }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) {}
    fn validate_output_port(&self, _port: &str) -> Result<(), String> { Ok(()) }
    fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> { Ok(()) }
    fn max_power(&self) -> u8 { 7 }
    fn start_port(&mut self, port: &str, _dir: PortDirection, power: u8) -> Result<(), String> {
        self.batches.lock().unwrap().push(vec![(port.to_string(), power)]);
        Ok(())
    }
    fn stop_port(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn run_port_for_time(&mut self, _port: &str, _dir: PortDirection, _power: u8, _tenths: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _port: &str, _dir: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _port: &str, _dir: PortDirection, _power: u8, _pos: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _port: &str, _dir: PortDirection, _power: u8, _position: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> { Ok(None) }
    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        self.batches.lock().unwrap().push(
            commands.iter().map(|c| (c.port.to_string(), c.power)).collect(),
        );
        Ok(())
    }
}

#[test]
fn test_set_power_batches_running_ports_into_one_start_ports_call() {
    let batches = std::sync::Arc::new(Mutex::new(Vec::new()));
    let adapter = BatchRecordingAdapter {
        ports: vec!["a".to_string(), "b".to_string()],
        batches: batches.clone(),
    };
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(adapter), "mock");
    let ports = vec!["a".to_string(), "b".to_string()];

    pm.on(&ports).unwrap();
    pm.set_power(&ports, 5).unwrap();

    let b = batches.lock().unwrap();
    assert_eq!(b.len(), 2, "on + setpower should each be one batch: {:?}", b);
    // setpower delivered both ports in a single grouped call — the property
    // the PF adapter needs to pair them into one Combo message.
    assert_eq!(b[1], vec![("a".to_string(), 5), ("b".to_string(), 5)]);
    drop(b);

    // Unchanged state must not retransmit (sync dedup preserved).
    pm.set_power(&ports, 5).unwrap();
    assert_eq!(batches.lock().unwrap().len(), 2);

    // rd on running ports is also one batch.
    pm.reverse_direction(&ports);
    assert_eq!(batches.lock().unwrap().len(), 3);
}

#[test]
fn test_set_power_after_one_port_off_transmits_only_the_running_port() {
    // talkto both → on → off one → setpower: the stopped port must stay
    // silent on the air (its new power just latches into state), and the
    // running port arrives alone — so the PF adapter sees a lone output,
    // not a pair, and no combo touches the stopped motor.
    let batches = std::sync::Arc::new(Mutex::new(Vec::new()));
    let adapter = BatchRecordingAdapter {
        ports: vec!["a".to_string(), "b".to_string()],
        batches: batches.clone(),
    };
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(adapter), "mock");
    let both = vec!["a".to_string(), "b".to_string()];

    pm.on(&both).unwrap();                       // batch 1: a+b
    pm.off(&["b".to_string()]).unwrap();         // stop (not recorded by mock)
    pm.set_power(&both, 3).unwrap();             // batch 2: must be a alone

    let b = batches.lock().unwrap();
    assert_eq!(b.len(), 2, "batches: {:?}", b);
    assert_eq!(b[1], vec![("a".to_string(), 3)]);
    drop(b);

    // The stopped port's power latched: turning both back on starts the
    // pair together at the new level.
    pm.on(&both).unwrap();
    let b = batches.lock().unwrap();
    assert_eq!(b[2], vec![("a".to_string(), 3), ("b".to_string(), 3)]);
}
