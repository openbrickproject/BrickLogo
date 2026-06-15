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
    fn max_power(&self) -> u8 { 100 }
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
    fn rotate_to_abs(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _position: i32,
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

/// Sensor mock with named input ports, each returning a fixed reading, so
/// `sensor?` can be exercised across single- and multi-port selections.
struct SensorMock {
    inputs: Vec<String>,
    readings: std::collections::HashMap<String, LogoValue>,
}

impl SensorMock {
    fn new(readings: &[(&str, LogoValue)]) -> Self {
        Self {
            inputs: readings.iter().map(|(p, _)| p.to_string()).collect(),
            readings: readings.iter().map(|(p, v)| (p.to_string(), v.clone())).collect(),
        }
    }
}

impl HardwareAdapter for SensorMock {
    fn display_name(&self) -> &str { "SensorMock" }
    fn output_ports(&self) -> &[String] { &[] }
    fn input_ports(&self) -> &[String] { &self.inputs }
    fn connected(&self) -> bool { true }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) {}
    fn validate_output_port(&self, _port: &str) -> Result<(), String> { Ok(()) }
    fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> { Ok(()) }
    fn max_power(&self) -> u8 { 100 }
    fn start_port(&mut self, _port: &str, _direction: PortDirection, _power: u8) -> Result<(), String> { Ok(()) }
    fn stop_port(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn run_port_for_time(&mut self, _port: &str, _direction: PortDirection, _power: u8, _tenths: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        Ok(self.readings.get(port).cloned())
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
        manager.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "pup");
        manager.add_device("bot2", Box::new(MockAdapter::new(&["b"])), "pup");
    }

    eval.evaluate("use \"bot2").unwrap();
    eval.evaluate("talkto [b bot1.a]").unwrap();
    eval.evaluate("listento \"bot1.a").unwrap();

    let manager = pm.lock().unwrap();
    assert_eq!(manager.get_active_device_name(), Some("bot2"));
    // Port selections are now on the evaluator, not the port manager
    assert_eq!(eval.selected_outputs(), &["b".to_string(), "bot1.a".to_string()]);
    assert_eq!(eval.selected_inputs(), &["bot1.a".to_string()]);
}

#[test]
fn test_bridge_disconnect_removes_active_device() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "pup");
        manager.add_device("bot2", Box::new(MockAdapter::new(&["b"])), "pup");
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
        "Type must be \"science\", \"pup\", \"wedo\", \"toypad\", \"controllab\", \"interfacea\", \"powerfunctions\", \"rcx\", \"buildhat\", \"ev3\", \"nxt\", or \"spike\" (interfacea and powerfunctions both use the \"brickinterface\" config entry)"
    );
}

// ── connected ───────────────────────────────

#[test]
fn test_connected_empty() {
    let (mut eval, _) = setup_eval();
    let result = eval.evaluate("connected").unwrap();
    assert_eq!(result, Some(LogoValue::List(vec![])));
}

#[test]
fn test_connected_with_devices() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("alpha", Box::new(MockAdapter::new(&["a"])), "pup");
        manager.add_device("beta", Box::new(MockAdapter::new(&["b"])), "science");
    }
    let result = eval.evaluate("connected").unwrap();
    assert_eq!(
        result,
        Some(LogoValue::List(vec![
            LogoValue::Word("alpha".to_string()),
            LogoValue::Word("beta".to_string()),
        ]))
    );
}

#[test]
fn test_connected_after_disconnect() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot1", Box::new(MockAdapter::new(&["a"])), "pup");
        manager.add_device("bot2", Box::new(MockAdapter::new(&["b"])), "pup");
    }
    eval.evaluate("disconnect").unwrap(); // disconnects bot1 (active)
    let result = eval.evaluate("connected").unwrap();
    assert_eq!(
        result,
        Some(LogoValue::List(vec![LogoValue::Word("bot2".to_string())]))
    );
}

// ── connected? ──────────────────────────────

#[test]
fn test_connected_query_true() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot", Box::new(MockAdapter::new(&["a"])), "pup");
    }
    let result = eval.evaluate("connected? \"bot").unwrap();
    assert_eq!(result, Some(LogoValue::Word("true".to_string())));
}

#[test]
fn test_connected_query_false_nonexistent() {
    let (mut eval, _) = setup_eval();
    let result = eval.evaluate("connected? \"nope").unwrap();
    assert_eq!(result, Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_connected_query_false_after_disconnect() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot", Box::new(MockAdapter::new(&["a"])), "pup");
    }
    eval.evaluate("disconnect").unwrap();
    let result = eval.evaluate("connected? \"bot").unwrap();
    assert_eq!(result, Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_connected_query_case_insensitive() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("mybot", Box::new(MockAdapter::new(&["a"])), "pup");
    }
    let result = eval.evaluate("connected? \"MYBOT").unwrap();
    assert_eq!(result, Some(LogoValue::Word("true".to_string())));
}

// ── device ──────────────────────────────────

#[test]
fn test_device_returns_type() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot", Box::new(MockAdapter::new(&["a"])), "pup");
    }
    let result = eval.evaluate("device \"bot").unwrap();
    assert_eq!(result, Some(LogoValue::Word("pup".to_string())));
}

#[test]
fn test_device_different_types() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("hub", Box::new(MockAdapter::new(&["a"])), "pup");
        manager.add_device("lab", Box::new(MockAdapter::new(&["a"])), "controllab");
        manager.add_device("hat", Box::new(MockAdapter::new(&["a"])), "buildhat");
    }
    assert_eq!(eval.evaluate("device \"hub").unwrap(), Some(LogoValue::Word("pup".to_string())));
    assert_eq!(eval.evaluate("device \"lab").unwrap(), Some(LogoValue::Word("controllab".to_string())));
    assert_eq!(eval.evaluate("device \"hat").unwrap(), Some(LogoValue::Word("buildhat".to_string())));
}

#[test]
fn test_device_nonexistent_errors() {
    let (mut eval, _) = setup_eval();
    let result = eval.evaluate("device \"nope");
    assert!(result.is_err());
}

#[test]
fn test_device_case_insensitive() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("mybot", Box::new(MockAdapter::new(&["a"])), "science");
    }
    assert_eq!(eval.evaluate("device \"MYBOT").unwrap(), Some(LogoValue::Word("science".to_string())));
}

// ── outputs ─────────────────────────────────

#[test]
fn test_outputs_returns_port_list() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot", Box::new(MockAdapter::new(&["a", "b", "c"])), "pup");
    }
    let result = eval.evaluate("outputs \"bot").unwrap();
    assert_eq!(
        result,
        Some(LogoValue::List(vec![
            LogoValue::Word("a".to_string()),
            LogoValue::Word("b".to_string()),
            LogoValue::Word("c".to_string()),
        ]))
    );
}

#[test]
fn test_outputs_single_port() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot", Box::new(MockAdapter::new(&["a"])), "science");
    }
    let result = eval.evaluate("outputs \"bot").unwrap();
    assert_eq!(
        result,
        Some(LogoValue::List(vec![LogoValue::Word("a".to_string())]))
    );
}

#[test]
fn test_outputs_nonexistent_errors() {
    let (mut eval, _) = setup_eval();
    assert!(eval.evaluate("outputs \"nope").is_err());
}

// ── inputs ──────────────────────────────────

#[test]
fn test_inputs_returns_empty_for_mock() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot", Box::new(MockAdapter::new(&["a"])), "pup");
    }
    // MockAdapter returns empty input_ports
    let result = eval.evaluate("inputs \"bot").unwrap();
    assert_eq!(result, Some(LogoValue::List(vec![])));
}

#[test]
fn test_inputs_nonexistent_errors() {
    let (mut eval, _) = setup_eval();
    assert!(eval.evaluate("inputs \"nope").is_err());
}

// ── combined usage ──────────────────────────

#[test]
fn test_foreach_connected_devices() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("alpha", Box::new(MockAdapter::new(&["a"])), "pup");
        manager.add_device("beta", Box::new(MockAdapter::new(&["b"])), "science");
    }
    // Use connected + foreach to iterate device names
    eval.evaluate("make \"types []").unwrap();
    eval.evaluate("foreach \"d connected [make \"types lput device :d :types]").unwrap();
    let result = eval.evaluate(":types").unwrap();
    assert_eq!(
        result,
        Some(LogoValue::List(vec![
            LogoValue::Word("pup".to_string()),
            LogoValue::Word("science".to_string()),
        ]))
    );
}

#[test]
fn test_if_connected_pattern() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("bot", Box::new(MockAdapter::new(&["a"])), "pup");
    }
    // Common pattern: conditionally use a device
    eval.evaluate("make \"result \"no").unwrap();
    eval.evaluate("if connected? \"bot [make \"result \"yes]").unwrap();
    assert_eq!(eval.evaluate(":result").unwrap(), Some(LogoValue::Word("yes".to_string())));
    eval.evaluate("if connected? \"nope [make \"result \"bad]").unwrap();
    assert_eq!(eval.evaluate(":result").unwrap(), Some(LogoValue::Word("yes".to_string())));
}

// ── BrickLogoConfig loading ───────────────────────────────────────────────────

// Write `content` to a unique temp file and run BrickLogoConfig::from_file on
// it, collecting any warnings. `None` exercises the missing-file path.
fn load_config_from(content: Option<&str>) -> (BrickLogoConfig, Vec<String>) {
    let path = std::env::temp_dir().join(format!(
        "bricklogo-config-test-{}-{:?}.json",
        std::process::id(),
        std::thread::current().id(),
    ));
    if let Some(c) = content {
        std::fs::write(&path, c).unwrap();
    } else {
        let _ = std::fs::remove_file(&path);
    }
    let warnings = Mutex::new(Vec::new());
    let config = BrickLogoConfig::from_file(&path, &|msg| {
        warnings.lock().unwrap().push(msg.to_string());
    });
    let _ = std::fs::remove_file(&path);
    (config, warnings.into_inner().unwrap())
}

#[test]
fn test_config_valid_file_parses_silently() {
    let (config, warnings) =
        load_config_from(Some(r#"{ "controllab": ["/dev/tty.usbserial-X"], "rcx": [] }"#));
    assert_eq!(config.controllab, vec!["/dev/tty.usbserial-X"]);
    assert!(config.rcx.is_empty());
    assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
}

#[test]
fn test_config_missing_file_defaults_silently() {
    let (config, warnings) = load_config_from(None);
    assert!(config.controllab.is_empty());
    assert!(warnings.is_empty());
}

#[test]
fn test_config_invalid_json_warns_and_defaults() {
    // Trailing comma — the classic hand-edit slip. Must warn (with serde's
    // line/column detail) instead of silently dropping the whole config.
    let (config, warnings) =
        load_config_from(Some("{\n  \"controllab\": [\"/dev/tty.usbserial-X\"],\n}\n"));
    assert!(config.controllab.is_empty(), "invalid config must fall back to defaults");
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("invalid JSON") && warnings[0].contains("line"),
        "warning should say what and where: {}",
        warnings[0]
    );
}

#[test]
fn test_config_unreadable_file_warns_and_defaults() {
    // A directory at the config path makes read_to_string fail while
    // `exists()` is true.
    let path = std::env::temp_dir().join(format!(
        "bricklogo-config-test-dir-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir(&path);
    std::fs::create_dir(&path).unwrap();
    let warnings = Mutex::new(Vec::new());
    let config = BrickLogoConfig::from_file(&path, &|msg| {
        warnings.lock().unwrap().push(msg.to_string());
    });
    let _ = std::fs::remove_dir(&path);
    assert!(config.controllab.is_empty());
    let warnings = warnings.into_inner().unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("could not read"), "got: {}", warnings[0]);
}

// ── sensor? ─────────────────────────────────

#[test]
fn test_sensor_query_single_port_truthy() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("s", Box::new(SensorMock::new(&[("a", LogoValue::Number(1.0))])), "pup");
    }
    eval.evaluate("listento [a]").unwrap();
    let result = eval.evaluate("sensor?").unwrap();
    assert_eq!(result, Some(LogoValue::Word("true".to_string())));
}

#[test]
fn test_sensor_query_single_port_falsey() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device("s", Box::new(SensorMock::new(&[("a", LogoValue::Number(0.0))])), "pup");
    }
    eval.evaluate("listento [a]").unwrap();
    let result = eval.evaluate("sensor?").unwrap();
    assert_eq!(result, Some(LogoValue::Word("false".to_string())));
}

#[test]
fn test_sensor_query_multi_port_returns_list_of_booleans() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device(
            "s",
            Box::new(SensorMock::new(&[
                ("a", LogoValue::Number(1.0)),
                ("b", LogoValue::Number(0.0)),
            ])),
            "pup",
        );
    }
    eval.evaluate("listento [a b]").unwrap();
    let result = eval.evaluate("sensor?").unwrap();
    // One boolean per port, aligned to selection order — mirroring `sensor`.
    assert_eq!(
        result,
        Some(LogoValue::List(vec![
            LogoValue::Word("true".to_string()),
            LogoValue::Word("false".to_string()),
        ]))
    );
}

#[test]
fn test_sensor_query_multi_port_preserves_selection_order() {
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device(
            "s",
            Box::new(SensorMock::new(&[
                ("a", LogoValue::Number(1.0)),
                ("b", LogoValue::Number(0.0)),
            ])),
            "pup",
        );
    }
    // Reversed selection reverses the result list.
    eval.evaluate("listento [b a]").unwrap();
    let result = eval.evaluate("sensor?").unwrap();
    assert_eq!(
        result,
        Some(LogoValue::List(vec![
            LogoValue::Word("false".to_string()),
            LogoValue::Word("true".to_string()),
        ]))
    );
}

#[test]
fn test_sensor_query_single_port_list_reading_is_one_boolean() {
    // A single multi-component reading (e.g. RGB) must collapse to ONE boolean,
    // not be mapped element-wise — that's why sensor? branches on port count.
    let (mut eval, pm) = setup_eval();
    {
        let mut manager = pm.lock().unwrap();
        manager.add_device(
            "s",
            Box::new(SensorMock::new(&[(
                "a",
                LogoValue::List(vec![
                    LogoValue::Number(255.0),
                    LogoValue::Number(0.0),
                    LogoValue::Number(0.0),
                ]),
            )])),
            "pup",
        );
    }
    eval.evaluate("listento [a]").unwrap();
    let result = eval.evaluate("sensor?").unwrap();
    assert_eq!(result, Some(LogoValue::Word("true".to_string())));
}

#[test]
fn test_sensor_query_propagates_read_error() {
    // A read error (here: a port on a device that doesn't exist) must surface,
    // not be swallowed into a bare `false`.
    let (mut eval, pm) = setup_eval();
    pm.lock().unwrap().add_device(
        "s",
        Box::new(SensorMock::new(&[("a", LogoValue::Number(1.0))])),
        "pup",
    );
    eval.evaluate("listento \"ghost.x").unwrap();
    assert!(eval.evaluate("sensor?").is_err());
}

// ── setcolor / setrgb ───────────────────────

/// Adapter that records color-output calls so the bridge primitives can be
/// tested end to end.
struct ColorMock {
    ports: Vec<String>,
    rgb: Arc<Mutex<Vec<(String, (u8, u8, u8))>>>,
    color: Arc<Mutex<Vec<(String, u8)>>>,
}

type RgbLog = Arc<Mutex<Vec<(String, (u8, u8, u8))>>>;
type ColorLog = Arc<Mutex<Vec<(String, u8)>>>;

impl ColorMock {
    fn new(ports: &[&str]) -> (Self, RgbLog, ColorLog) {
        let rgb: RgbLog = Arc::new(Mutex::new(Vec::new()));
        let color: ColorLog = Arc::new(Mutex::new(Vec::new()));
        let mock = ColorMock {
            ports: ports.iter().map(|s| s.to_string()).collect(),
            rgb: rgb.clone(),
            color: color.clone(),
        };
        (mock, rgb, color)
    }
}

impl HardwareAdapter for ColorMock {
    fn display_name(&self) -> &str { "ColorMock" }
    fn output_ports(&self) -> &[String] { &self.ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { true }
    fn connect(&mut self) -> Result<(), String> { Ok(()) }
    fn disconnect(&mut self) {}
    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if self.ports.iter().any(|p| p == port) { Ok(()) } else { Err(format!("bad port {port}")) }
    }
    fn validate_sensor_port(&self, _: &str, _: Option<&str>) -> Result<(), String> { Ok(()) }
    fn max_power(&self) -> u8 { 100 }
    fn start_port(&mut self, _: &str, _: PortDirection, _: u8) -> Result<(), String> { Ok(()) }
    fn stop_port(&mut self, _: &str) -> Result<(), String> { Ok(()) }
    fn run_port_for_time(&mut self, _: &str, _: PortDirection, _: u8, _: u32) -> Result<(), String> { Ok(()) }
    fn rotate_port_by_degrees(&mut self, _: &str, _: PortDirection, _: u8, _: i32) -> Result<(), String> { Ok(()) }
    fn rotate_port_to_position(&mut self, _: &str, _: PortDirection, _: u8, _: i32) -> Result<(), String> { Ok(()) }
    fn reset_port_zero(&mut self, _: &str) -> Result<(), String> { Ok(()) }
    fn rotate_to_abs(&mut self, _: &str, _: PortDirection, _: u8, _: i32) -> Result<(), String> { Ok(()) }
    fn read_sensor(&mut self, _: &str, _: Option<&str>) -> Result<Option<LogoValue>, String> { Ok(None) }
    fn set_color(&mut self, port: &str, id: u8) -> Result<(), String> {
        self.color.lock().unwrap().push((port.to_string(), id));
        Ok(())
    }
    fn set_rgb(&mut self, port: &str, rgb: (u8, u8, u8)) -> Result<(), String> {
        self.rgb.lock().unwrap().push((port.to_string(), rgb));
        Ok(())
    }
}

#[test]
fn test_setrgb_calls_adapter() {
    let (mut eval, pm) = setup_eval();
    let (mock, rgb, _color) = ColorMock::new(&["a"]);
    pm.lock().unwrap().add_device("bot", Box::new(mock), "pup");
    eval.evaluate("talkto \"a").unwrap();
    eval.evaluate("setrgb [255 0 128]").unwrap();
    assert_eq!(*rgb.lock().unwrap(), vec![("a".to_string(), (255, 0, 128))]);
}

#[test]
fn test_setcolor_accepts_name_and_id() {
    let (mut eval, pm) = setup_eval();
    let (mock, _rgb, color) = ColorMock::new(&["a"]);
    pm.lock().unwrap().add_device("bot", Box::new(mock), "pup");
    eval.evaluate("talkto \"a").unwrap();
    eval.evaluate("setcolor \"red").unwrap(); // name → id 9
    eval.evaluate("setcolor 3").unwrap();      // raw id
    assert_eq!(
        *color.lock().unwrap(),
        vec![("a".to_string(), 9), ("a".to_string(), 3)]
    );
}

#[test]
fn test_setcolor_unknown_name_errors() {
    let (mut eval, pm) = setup_eval();
    let (mock, _, _) = ColorMock::new(&["a"]);
    pm.lock().unwrap().add_device("bot", Box::new(mock), "pup");
    eval.evaluate("talkto \"a").unwrap();
    let err = eval.evaluate("setcolor \"chartreuse").unwrap_err();
    assert!(err.to_string().contains("unknown color"), "got: {}", err);
}

#[test]
fn test_setrgb_rejects_bad_args() {
    let (mut eval, pm) = setup_eval();
    let (mock, _, _) = ColorMock::new(&["a"]);
    pm.lock().unwrap().add_device("bot", Box::new(mock), "pup");
    eval.evaluate("talkto \"a").unwrap();
    assert!(eval.evaluate("setrgb [255 0]").is_err()); // wrong length
    assert!(eval.evaluate("setrgb [300 0 0]").is_err()); // out of range
    assert!(eval.evaluate("setrgb 5").is_err()); // not a list
}

#[test]
fn test_setrgb_not_supported_propagates() {
    let (mut eval, pm) = setup_eval();
    pm.lock().unwrap().add_device("bot", Box::new(MockAdapter::new(&["a"])), "pup");
    eval.evaluate("talkto \"a").unwrap();
    let err = eval.evaluate("setrgb [1 2 3]").unwrap_err();
    assert!(err.to_string().contains("does not support"), "got: {}", err);
}
