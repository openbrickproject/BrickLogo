use crate::bridge::register_hardware_primitives;
use bricklogo_hal::port_manager::PortManager;
use bricklogo_lang::error::LogoError;
use bricklogo_lang::evaluator::Evaluator;
use bricklogo_lang::primitives::register_core_primitives;
use bricklogo_lang::value::LogoValue;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum OutputLineType {
    Input,
    Output,
    Error,
    System,
}

#[derive(Debug, Clone)]
pub struct OutputLine {
    pub text: String,
    pub line_type: OutputLineType,
}

type EvalResult = (Evaluator, Result<Option<LogoValue>, LogoError>);

pub struct App {
    pub output_lines: Vec<OutputLine>,
    pub input: String,
    pub input_history: Vec<String>,
    pub history_index: i32,
    pub cursor_position: usize,
    pub busy: bool,
    pub connected_devices: Vec<String>,
    pub active_device: Option<String>,
    pub selected_outputs: Vec<String>,
    pub selected_inputs: Vec<String>,
    pub should_quit: bool,
    pub help_mode: bool,
    pub help_scroll: usize,
    pub def_buffer: Option<Vec<String>>,
    evaluator: Option<Evaluator>,
    eval_result_rx: Option<mpsc::Receiver<EvalResult>>,
    firmware_result_rx: Option<mpsc::Receiver<(Result<(), String>, Result<(), String>)>>,
    stop_flag: Arc<AtomicBool>,
    port_manager: Arc<Mutex<PortManager>>,
    output_buffer: Arc<Mutex<Vec<OutputLine>>>,
}

impl App {
    pub fn new() -> Self {
        let output_lines: Vec<OutputLine> = Vec::new();
        let output_lines_ref = Arc::new(Mutex::new(Vec::<OutputLine>::new()));
        let output_clone = output_lines_ref.clone();

        let mut evaluator = Evaluator::new(Arc::new(move |text: &str| {
            output_clone.lock().unwrap().push(OutputLine {
                text: text.to_string(),
                line_type: OutputLineType::Output,
            });
        }));
        register_core_primitives(&mut evaluator);

        let stop_flag = evaluator.stop_flag();
        let port_manager = Arc::new(Mutex::new(PortManager::new()));
        let system_output = output_lines_ref.clone();
        let system_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |text: &str| {
            system_output.lock().unwrap().push(OutputLine {
                text: text.to_string(),
                line_type: OutputLineType::System,
            });
        });
        evaluator.set_system_fn(system_fn.clone());
        register_hardware_primitives(&mut evaluator, port_manager.clone(), system_fn);

        App {
            output_lines,
            input: String::new(),
            input_history: Vec::new(),
            history_index: -1,
            cursor_position: 0,
            busy: false,
            connected_devices: Vec::new(),
            active_device: None,
            selected_outputs: Vec::new(),
            selected_inputs: Vec::new(),
            should_quit: false,
            help_mode: false,
            help_scroll: 0,
            def_buffer: None,
            evaluator: Some(evaluator),
            eval_result_rx: None,
            firmware_result_rx: None,
            stop_flag,
            port_manager,
            output_buffer: output_lines_ref,
        }
    }

    /// Drain any output produced by the evaluator's print/show/type callbacks.
    /// Returns true if any new output was drained.
    pub fn drain_output_buffer(&mut self) -> bool {
        let mut buf = self.output_buffer.lock().unwrap();
        if buf.is_empty() {
            return false;
        }
        self.output_lines.append(&mut buf);
        true
    }

    /// Check if a background evaluation has completed. Call from the main loop.
    /// Returns true if state changed (new output or evaluation finished).
    pub fn tick(&mut self) -> bool {
        let mut changed = self.drain_output_buffer();

        // Sync connected device names from port manager
        let (devices, active_device, selected_outputs, selected_inputs) = {
            let pm = self.port_manager.lock().unwrap();
            (
                pm.get_connected_device_names(),
                pm.get_active_device_name_owned(),
                pm.get_selected_output_display_ports(),
                pm.get_selected_input_display_ports(),
            )
        };

        if devices != self.connected_devices {
            self.connected_devices = devices;
            changed = true;
        }
        if active_device != self.active_device {
            self.active_device = active_device;
            changed = true;
        }
        if selected_outputs != self.selected_outputs {
            self.selected_outputs = selected_outputs;
            changed = true;
        }
        if selected_inputs != self.selected_inputs {
            self.selected_inputs = selected_inputs;
            changed = true;
        }

        if let Some(ref rx) = self.eval_result_rx {
            match rx.try_recv() {
                Ok((evaluator, result)) => {
                    self.evaluator = Some(evaluator);
                    self.eval_result_rx = None;
                    self.drain_output_buffer();
                    match result {
                        Ok(Some(val)) => {
                            self.add_output(&val.as_string(), OutputLineType::Output);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            self.add_output(&format!("{}", e), OutputLineType::Error);
                        }
                    }
                    self.busy = false;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Thread panicked — recover
                    self.eval_result_rx = None;
                    self.add_output("Evaluation failed unexpectedly", OutputLineType::Error);
                    self.busy = false;
                    changed = true;
                }
            }
        }

        if let Some(ref rx) = self.firmware_result_rx {
            match rx.try_recv() {
                Ok((upload_result, reconnect_result)) => {
                    self.firmware_result_rx = None;
                    self.drain_output_buffer();
                    match upload_result {
                        Ok(()) => self.add_output("Firmware upload complete", OutputLineType::System),
                        Err(e) => self.add_output(&format!("Firmware upload failed: {}", e), OutputLineType::Error),
                    }
                    match reconnect_result {
                        Ok(()) => self.add_output("RCX reconnected", OutputLineType::System),
                        Err(e) => self.add_output(&format!("Reconnect failed: {}", e), OutputLineType::Error),
                    }
                    self.busy = false;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.firmware_result_rx = None;
                    self.add_output("Firmware upload thread failed", OutputLineType::Error);
                    self.busy = false;
                    changed = true;
                }
            }
        }

        changed
    }

    pub fn add_output(&mut self, text: &str, line_type: OutputLineType) {
        self.output_lines.push(OutputLine {
            text: text.to_string(),
            line_type,
        });
    }

    pub fn submit_input(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Add to history
        if self.input_history.first().map(|s| s.as_str()) != Some(&input) {
            self.input_history.insert(0, input.clone());
        }
        self.history_index = -1;
        self.input.clear();
        self.cursor_position = 0;

        // Handle multi-line definition mode
        if self.def_buffer.is_some() {
            if input.to_lowercase() == "end" {
                let mut full_source = self.def_buffer.as_ref().unwrap().clone();
                full_source.push("end".to_string());
                let source = full_source.join("\n");
                self.add_output("> end", OutputLineType::Input);
                self.def_buffer = None;
                self.execute(&source);
            } else {
                let indent = self.calc_def_indent(&input);
                let display = format!("> {}{}", "  ".repeat(indent), input);
                self.add_output(&display, OutputLineType::Input);
                self.def_buffer.as_mut().unwrap().push(input);
            }
            return;
        }

        // Check for procedure definition start
        if input.to_lowercase().starts_with("to ") && !input.to_lowercase().contains(" end") {
            let words: Vec<&str> = input.split_whitespace().collect();
            if words.last().map(|w| w.to_lowercase()) != Some("end".to_string()) {
                self.add_output(&format!("? {}", input), OutputLineType::Input);
                self.def_buffer = Some(vec![input]);
                return;
            }
        }

        self.add_output(&format!("? {}", input), OutputLineType::Input);

        // Handle firmware command (before exact match)
        if input.to_lowercase().starts_with("firmware ") {
            self.handle_firmware_command(&input);
            return;
        }

        // Handle special commands
        match input.to_lowercase().as_str() {
            "clear" => {
                self.output_lines.clear();
                return;
            }
            "bye" | "exit" => {
                self.port_manager.lock().unwrap().remove_all();
                self.should_quit = true;
                return;
            }
            "help" => {
                self.help_mode = true;
                self.help_scroll = 0;
                return;
            }
            _ => {}
        }

        self.execute(&input);
    }

    fn handle_firmware_command(&mut self, input: &str) {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() != 3 {
            self.add_output("Usage: firmware \"device \"file.lgo", OutputLineType::Error);
            return;
        }
        let device_name = parts[1].trim_start_matches('"').to_lowercase();
        let firmware_path = parts[2].trim_start_matches('"').to_string();

        // Resolve path relative to disk path if evaluator is available
        let resolved_path = if let Some(ref eval) = self.evaluator {
            let disk = eval.disk_path();
            let candidate = disk.join(&firmware_path);
            if candidate.exists() {
                candidate.to_string_lossy().to_string()
            } else {
                firmware_path
            }
        } else {
            firmware_path
        };

        // Prepare: disconnect device, get transport config
        let prepare_result = self.port_manager.lock().unwrap()
            .prepare_firmware_upload(&device_name);
        let serial_path = match prepare_result {
            Ok(path) => path,
            Err(e) => {
                self.add_output(&format!("Error: {}", e), OutputLineType::Error);
                return;
            }
        };

        self.add_output("Starting firmware upload...", OutputLineType::System);
        self.busy = true;

        let pm = self.port_manager.clone();
        let output_buffer = self.output_buffer.clone();
        let device_name_owned = device_name.clone();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let upload_result = (|| -> Result<(), String> {
                // Read and parse S-Record file
                let content = std::fs::read_to_string(&resolved_path)
                    .map_err(|e| format!("Cannot read firmware file: {}", e))?;
                let image = rust_rcx::srec::parse_srec(&content)?;

                // Open a fresh transport
                let mut transport = bricklogo_hal::adapters::rcx_adapter::open_transport(
                    serial_path.as_deref()
                )?;

                // Upload with progress
                let ob = output_buffer.clone();
                let progress: rust_rcx::firmware::ProgressFn = Box::new(move |current, total, phase| {
                    ob.lock().unwrap().push(OutputLine {
                        text: format!("{} ({}/{})", phase, current, total),
                        line_type: OutputLineType::System,
                    });
                });

                rust_rcx::firmware::upload_firmware(
                    &image,
                    &mut |msg| transport.request_firmware(msg),
                    &progress,
                )
            })();

            // Reconnect
            let reconnect_result = pm.lock().unwrap()
                .reconnect_after_firmware(&device_name_owned);

            let _ = tx.send((upload_result, reconnect_result));
        });

        self.firmware_result_rx = Some(rx);
    }

    fn execute(&mut self, input: &str) {
        let mut evaluator = match self.evaluator.take() {
            Some(e) => e,
            None => return, // already busy
        };

        self.busy = true;
        let input = input.to_string();
        let (tx, rx) = mpsc::channel();
        self.eval_result_rx = Some(rx);

        std::thread::spawn(move || {
            let result = evaluator.evaluate(&input);
            let _ = tx.send((evaluator, result));
        });
    }

    pub fn request_stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    pub fn cancel_definition(&mut self) {
        if self.def_buffer.is_some() {
            self.def_buffer = None;
            self.add_output("Definition cancelled", OutputLineType::System);
        }
    }

    pub fn history_up(&mut self) {
        if !self.input_history.is_empty() {
            let new_index = (self.history_index + 1).min(self.input_history.len() as i32 - 1);
            self.history_index = new_index;
            self.input = self.input_history[new_index as usize].clone();
            self.cursor_position = self.input.len();
        }
    }

    pub fn history_down(&mut self) {
        if self.history_index <= 0 {
            self.history_index = -1;
            self.input.clear();
            self.cursor_position = 0;
        } else {
            self.history_index -= 1;
            self.input = self.input_history[self.history_index as usize].clone();
            self.cursor_position = self.input.len();
        }
    }

    fn calc_def_indent(&self, current_line: &str) -> usize {
        if current_line.trim().to_lowercase() == "end" {
            return 0;
        }
        if let Some(buffer) = &self.def_buffer {
            let mut depth: i32 = 1; // inside to...end
            for line in buffer {
                for ch in line.chars() {
                    if ch == '[' {
                        depth += 1;
                    }
                    if ch == ']' {
                        depth -= 1;
                    }
                }
            }
            if current_line.trim().starts_with(']') {
                depth -= 1;
            }
            depth.max(0) as usize
        } else {
            0
        }
    }

    pub fn get_prompt(&self) -> &str {
        if self.busy {
            return "... ";
        }
        if self.def_buffer.is_some() {
            return "> ";
        }
        "? "
    }

    pub fn help_lines(&self) -> Vec<String> {
        let mut lines = vec![
            String::new(),
            "  Connection".to_string(),
            "    connectto \"type \"name                Connect to a device".to_string(),
            "      Device types: \"science \"pup \"wedo \"controllab \"rcx".to_string(),
            "    use \"name                            Set active device".to_string(),
            "    disconnect                           Disconnect active device".to_string(),
            "    disconnect \"name                     Disconnect specific device".to_string(),
            "    disconnect \"all                      Disconnect all devices".to_string(),
            String::new(),
            "    firmware \"device \"file.lgo           Upload RCX firmware".to_string(),
            String::new(),
            "  Motor Control".to_string(),
            "    talkto \"port  /  talkto [a b]        Select output port(s)".to_string(),
            "    talkto \"name.port                    Select port on specific device".to_string(),
            "    on                                   Start selected ports".to_string(),
            "    off                                  Stop selected ports".to_string(),
            "    onfor <tenths>                       Run selected ports for time".to_string(),
            "    setpower <0-8>                       Set power level".to_string(),
            "    setleft / seteven                    Set direction to forward".to_string(),
            "    setright / setodd                    Set direction to reverse".to_string(),
            "    rd                                   Reverse direction".to_string(),
            "    rotate <degrees>                     Rotate by degrees".to_string(),
            "    rotateto <position>                  Rotate to position".to_string(),
            "    resetzero                            Reset encoder zero".to_string(),
            "    rotatetohome                         Rotate to hardware zero".to_string(),
            "    flash <on> <off>                     Flash on/off cycle".to_string(),
            "    alloff                               Stop all ports".to_string(),
            String::new(),
            "  Sensors".to_string(),
            "    listento \"port / listento [a b]      Select sensor port(s)".to_string(),
            "    sensor?                              Is sensor active?".to_string(),
            "    sensor \"mode                         Read sensor value".to_string(),
            "    color / light / force / angle        Typed sensor readers".to_string(),
            String::new(),
            "  Language".to_string(),
            "    make \"name <value>                   Set a variable".to_string(),
            "    :name                                Get a variable".to_string(),
            "    print <value>  /  show <value>       Output a value".to_string(),
            "    repeat <n> [...]                     Repeat commands".to_string(),
            "    forever [...]                        Loop forever".to_string(),
            "    if <cond> [...]                      Conditional".to_string(),
            "    ifelse <cond> [...] [...]            Conditional with else".to_string(),
            "    waituntil [...]                      Wait for condition".to_string(),
            "    wait <tenths>                        Pause".to_string(),
            "    to <name> <:params> ... end          Define a procedure".to_string(),
            "    output <value>  /  stop              Return from procedure".to_string(),
            "    erase \"name                          Remove a procedure".to_string(),
            "    carefully [...] [...]                Error handling".to_string(),
            String::new(),
        ];

        if let Some(ref evaluator) = self.evaluator {
            let procs = evaluator.get_all_procedures();
            if !procs.is_empty() {
                lines.push("  Procedures".to_string());
                for proc in procs {
                    let params = if proc.params.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " {}",
                            proc.params
                                .iter()
                                .map(|p| format!(":{}", p))
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    };
                    lines.push(format!("    {}{}", proc.name, params));
                }
                lines.push(String::new());
            }
        }

        lines.push("  Pages and Files".to_string());
        lines.push("    setdisk \"path                        Set file directory".to_string());
        lines.push("    disk                                 Show current directory".to_string());
        lines.push("    namepage \"name / np \"name            Set page name for save".to_string());
        lines.push("    save                                 Save procedures to page".to_string());
        lines.push("    load \"name / getpage \"name           Load a page".to_string());
        lines.push(String::new());
        lines.push("  REPL".to_string());
        lines.push("    clear                                Clear history".to_string());
        lines.push("    help                                 Show this help".to_string());
        lines.push("    bye / exit                           Quit BrickLogo".to_string());
        lines.push(String::new());

        lines
    }
}

#[cfg(test)]
#[path = "tests/app.rs"]
mod tests;
