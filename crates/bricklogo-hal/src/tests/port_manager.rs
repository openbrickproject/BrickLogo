use super::*;

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
    fn rotate_to_home(&mut self, _port: &str, _dir: PortDirection, _power: u8) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> { Ok(None) }
}

#[test]
fn test_first_device_becomes_active() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
    assert_eq!(pm.get_active_device_name(), Some("bot"));
}

#[test]
fn test_second_device_not_active() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
    assert_eq!(pm.get_active_device_name(), Some("bot1"));
    assert_eq!(pm.get_connected_device_names(), vec!["bot1".to_string(), "bot2".to_string()]);
}

#[test]
fn test_use_switches_active() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
    pm.set_active_device("bot2").unwrap();
    assert_eq!(pm.get_active_device_name(), Some("bot2"));
}

#[test]
fn test_remove_device_fallback() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
    pm.remove_device("bot1");
    assert_eq!(pm.get_active_device_name(), Some("bot2"));
    assert_eq!(pm.get_connected_device_names(), vec!["bot2".to_string()]);
}

#[test]
fn test_ensure_port_states() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
    pm.ensure_port_states(&["a".to_string()]).unwrap();
}

#[test]
fn test_ensure_port_states_qualified() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
    pm.ensure_port_states(&["bot.a".to_string()]).unwrap();
}

#[test]
fn test_on_off() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
    let ports = vec!["a".to_string(), "b".to_string()];
    pm.ensure_port_states(&ports).unwrap();
    pm.on(&ports).unwrap();
    pm.off(&ports).unwrap();
}

#[test]
fn test_set_power() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a"])));
    let ports = vec!["a".to_string()];
    pm.ensure_port_states(&ports).unwrap();
    pm.set_power(&ports, 8);
    pm.on(&ports).unwrap();
}

#[test]
fn test_all_off() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a", "b"])));
    let ports = vec!["a".to_string(), "b".to_string()];
    pm.ensure_port_states(&ports).unwrap();
    pm.on(&ports).unwrap();
    pm.all_off();
}

#[test]
fn test_read_sensor_no_port() {
    let mut pm = PortManager::new();
    pm.add_device("bot", Box::new(MockAdapter::new(&["a"])));
    assert!(pm.read_sensor(&[], None).is_err());
}

#[test]
fn test_remove_all() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
    pm.add_device("bot2", Box::new(MockAdapter::new(&["a"])));
    pm.remove_all();
    assert!(pm.get_active_device_name().is_none());
    assert!(pm.get_connected_device_names().is_empty());
}

#[test]
fn test_connection_order_preserved_after_use_and_remove() {
    let mut pm = PortManager::new();
    pm.add_device("alpha", Box::new(MockAdapter::new(&["a"])));
    pm.add_device("beta", Box::new(MockAdapter::new(&["a"])));
    pm.add_device("gamma", Box::new(MockAdapter::new(&["a"])));
    pm.set_active_device("gamma").unwrap();

    assert_eq!(
        pm.get_connected_device_names(),
        vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
    );

    pm.remove_device("gamma");
    assert_eq!(pm.get_active_device_name(), Some("alpha"));
    assert_eq!(pm.get_connected_device_names(), vec!["alpha".to_string(), "beta".to_string()]);
}

#[test]
fn test_format_port_names() {
    let mut pm = PortManager::new();
    pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
    pm.add_device("bot2", Box::new(MockAdapter::new(&["b"])));

    // Active device is bot1, so "a" stays short, "bot2.b" stays qualified
    let outputs = vec!["a".to_string(), "bot2.b".to_string()];
    let display = pm.format_port_names(&outputs);
    assert_eq!(display, vec!["a".to_string(), "bot2.b".to_string()]);

    let inputs = vec!["bot2.b".to_string()];
    let display = pm.format_port_names(&inputs);
    assert_eq!(display, vec!["bot2.b".to_string()]);
}
