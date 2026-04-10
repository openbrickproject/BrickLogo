use super::*;
use bricklogo_hal::adapter::{HardwareAdapter, PortDirection};

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
        Ok(None)
    }
}

fn run_until_idle(app: &mut App) {
    for _ in 0..50 {
        app.tick();
        if !app.busy {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.tick();
}

#[test]
fn test_help_clear_and_bye_are_repl_commands() {
    let mut app = App::new(None).unwrap();

    app.input = "help".to_string();
    app.submit_input();
    assert!(app.help_mode);

    app.output_lines.push(OutputLine {
        text: "old".to_string(),
        line_type: OutputLineType::Output,
    });
    app.input = "clear".to_string();
    app.submit_input();
    assert!(app.output_lines.is_empty());

    app.input = "bye".to_string();
    app.submit_input();
    assert!(app.should_quit);
}

#[test]
fn test_multiline_definition_mode_executes_definition() {
    let mut app = App::new(None).unwrap();

    app.input = "to greet".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());

    app.input = "print \"hi".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());

    app.input = "end".to_string();
    app.submit_input();
    run_until_idle(&mut app);

    app.input = "greet".to_string();
    app.submit_input();
    run_until_idle(&mut app);

    assert!(app.output_lines.iter().any(|line| line.text == "hi"));
}

#[test]
fn test_cancel_definition_clears_buffer() {
    let mut app = App::new(None).unwrap();
    app.input = "to greet".to_string();
    app.submit_input();
    app.cancel_definition();
    assert!(app.multi_line.is_none());
    assert!(
        app.output_lines
            .iter()
            .any(|line| line.text == "Cancelled")
    );
}

#[test]
fn test_multiline_bracket_mode() {
    let mut app = App::new(None).unwrap();

    app.input = "forever [".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());

    app.input = "print \"hello".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());

    app.input = "]".to_string();
    app.submit_input();
    // Should have submitted — multi_line cleared, now busy
    assert!(app.multi_line.is_none());
}

#[test]
fn test_multiline_to_with_brackets() {
    let mut app = App::new(None).unwrap();

    app.input = "to greet [".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());

    app.input = "print \"hi".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());

    // Closing bracket but still need end
    app.input = "] end".to_string();
    app.submit_input();
    run_until_idle(&mut app);

    assert!(app.multi_line.is_none());
}

#[test]
fn test_syntax_error_no_multiline() {
    let mut app = App::new(None).unwrap();
    app.input = "repeat 4 ]".to_string();
    app.submit_input();
    assert!(app.multi_line.is_none());
    assert!(app.output_lines.iter().any(|line| matches!(line.line_type, OutputLineType::Error)));
}

#[test]
fn test_error_on_continuation_discards_line() {
    let mut app = App::new(None).unwrap();

    app.input = "forever [".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());

    // Type a bad line
    app.input = ")".to_string();
    app.submit_input();
    // Error shown but multi_line preserved (bad line discarded)
    assert!(app.multi_line.is_some());
    assert_eq!(app.multi_line.as_ref().unwrap().lines.len(), 1); // only "forever ["
    assert!(app.output_lines.iter().any(|line| matches!(line.line_type, OutputLineType::Error)));
}

#[test]
fn test_history_navigation_round_trips() {
    let mut app = App::new(None).unwrap();
    for cmd in ["print \"one", "print \"two"] {
        app.input = cmd.to_string();
        app.submit_input();
        run_until_idle(&mut app);
    }

    app.history_up();
    assert_eq!(app.input, "print \"two");
    app.history_up();
    assert_eq!(app.input, "print \"one");
    app.history_down();
    assert_eq!(app.input, "print \"two");
    app.history_down();
    assert_eq!(app.input, "");
}

#[test]
fn test_tick_syncs_device_and_selection_context() {
    let mut app = App::new(None).unwrap();
    {
        let mut pm = app.port_manager.lock().unwrap();
        pm.add_device("bot1", Box::new(MockAdapter::new(&["a"])));
        pm.add_device("bot2", Box::new(MockAdapter::new(&["b"])));
    }
    // Set selections on the evaluator
    if let Some(ref mut eval) = app.evaluator {
        eval.set_selected_outputs(vec!["a".to_string(), "bot2.b".to_string()]);
        eval.set_selected_inputs(vec!["bot2.b".to_string()]);
    }

    assert!(app.tick());
    assert_eq!(
        app.connected_devices,
        vec!["bot1".to_string(), "bot2".to_string()]
    );
    assert_eq!(app.active_device.as_deref(), Some("bot1"));
    assert_eq!(
        app.selected_outputs,
        vec!["a".to_string(), "bot2.b".to_string()]
    );
    assert_eq!(app.selected_inputs, vec!["bot2.b".to_string()]);
}
