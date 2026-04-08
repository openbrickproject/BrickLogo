use super::*;
use bricklogo_hal::adapter::PortDirection;
use bricklogo_lang::primitives::register_core_primitives;

struct MockAdapter {
    outputs: Vec<String>,
    connected: bool,
}

impl MockAdapter {
    fn new(outputs: &[&str]) -> Self {
        Self {
            outputs: outputs.iter().map(|s| s.to_string()).collect(),
            connected: true,
        }
    }
}

impl HardwareAdapter for MockAdapter {
    fn display_name(&self) -> &str {
        "Mock"
    }
    fn output_ports(&self) -> &[String] {
        &self.outputs
    }
    fn input_ports(&self) -> &[String] {
        &[]
    }
    fn connected(&self) -> bool {
        self.connected
    }
    fn connect(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn disconnect(&mut self) {
        self.connected = false;
    }
    fn validate_output_port(&self, _port: &str) -> Result<(), String> {
        Ok(())
    }
    fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> {
        Ok(())
    }
    fn start_port(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
    ) -> Result<(), String> {
        Ok(())
    }
    fn stop_port(&mut self, _port: &str) -> Result<(), String> {
        Ok(())
    }
    fn run_port_for_time(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _tenths: u32,
    ) -> Result<(), String> {
        Ok(())
    }
    fn rotate_port_by_degrees(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _degrees: i32,
    ) -> Result<(), String> {
        Ok(())
    }
    fn rotate_port_to_position(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _position: i32,
    ) -> Result<(), String> {
        Ok(())
    }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Ok(())
    }
    fn rotate_to_home(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
    ) -> Result<(), String> {
        Ok(())
    }
    fn read_sensor(
        &mut self,
        _port: &str,
        _mode: Option<&str>,
    ) -> Result<Option<LogoValue>, String> {
        Ok(Some(LogoValue::Number(1.0)))
    }
}

fn setup_eval() -> (Evaluator, Arc<Mutex<PortManager>>) {
    let mut eval = Evaluator::new(Arc::new(|_| {}));
    register_core_primitives(&mut eval);
    let pm = Arc::new(Mutex::new(PortManager::new()));
    register_hardware_primitives(&mut eval, pm.clone(), Arc::new(|_| {}));
    (eval, pm)
}

#[test]
fn test_bridge_primitives_update_port_manager_state() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
        manager.add_device("bot2", Box::new(MockAdapter::new(&["b"])));
    }

    eval.evaluate("use \"bot2").unwrap();
    eval.evaluate("talkto [b bot1.a]").unwrap();
    eval.evaluate("listento \"bot1.a").unwrap();

    let manager = pm.lock().unwrap();
    assert_eq!(manager.get_active_device_name(), Some("bot2"));
    assert_eq!(
        manager.get_selected_output_display_ports(),
        vec!["b".to_string(), "bot1.a".to_string()]
    );
    assert_eq!(
        manager.get_selected_input_display_ports(),
        vec!["bot1.a".to_string()]
    );
}

#[test]
fn test_bridge_disconnect_removes_active_device() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
        manager.add_device("bot2", Box::new(MockAdapter::new(&["b"])));
    }

    eval.evaluate("use \"bot2").unwrap();
    eval.evaluate("disconnect").unwrap();

    let manager = pm.lock().unwrap();
    assert_eq!(manager.get_active_device_name(), Some("bot1"));
    assert_eq!(
        manager.get_connected_device_names(),
        vec!["bot1".to_string()]
    );
}

#[test]
fn test_bridge_connect_rejects_unknown_type() {
    let (mut eval, _) = setup_eval();
    let err = eval.evaluate("connectto \"nope \"bot").unwrap_err();
    assert_eq!(
        err.to_string(),
        "Type must be \"science\", \"pup\", \"wedo\", \"controllab\", or \"rcx\""
    );
}
