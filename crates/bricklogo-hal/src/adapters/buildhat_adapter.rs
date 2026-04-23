use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use rust_buildhat::constants::*;
use rust_buildhat::protocol::*;
use rust_buildhat::constants::{is_absolute_motor, needs_led_init};
use rust_buildhat::firmware;

/// Serial transport abstraction — lets tests inject a mock without a real
/// Build HAT attached.
pub trait BuildHATTransport: Send {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String>;
    fn write_all(&mut self, data: &[u8]) -> Result<(), String>;
    fn flush(&mut self) -> Result<(), String>;
}

impl BuildHATTransport for Box<dyn serialport::SerialPort> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        match Read::read(self.as_mut(), buf) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(e.to_string()),
        }
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        Write::write_all(self.as_mut(), data).map_err(|e| e.to_string())
    }
    fn flush(&mut self) -> Result<(), String> {
        Write::flush(self.as_mut()).map_err(|e| e.to_string())
    }
}

const SENSOR_POLL_INTERVAL_MS: u32 = 50;
const ALL_PORTS: [&str; 4] = ["a", "b", "c", "d"];

// ── Commands queued for the driver slot ─────────

type ReplyTx = Option<mpsc::Sender<Result<(), String>>>;

#[allow(dead_code)]
enum BuildHATCommand {
    Raw(String),
    MotorSet { port: u8, speed: i32, reply_tx: ReplyTx },
    MotorSpeed { port: u8, speed: i32, reply_tx: ReplyTx },
    MotorCoast { port: u8, reply_tx: ReplyTx },
    MotorOff { port: u8, reply_tx: ReplyTx },
    MotorPulse { port: u8, speed: i32, seconds: f64 },
    MotorRamp { port: u8, from: f64, to: f64, duration: f64 },
    SelectMode { port: u8, mode: u8 },
    SelectCombi { port: u8, combi_index: u8, modes: Vec<(u8, u8)>, interval_ms: u32 },
    SetValue { port: u8, value: i32 },
    Preset { port: u8, mode: u8, value: f64 },
    Plimit { port: u8, limit: f64 },
}

// ── Shared state ────────────────────────────────

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub type_id: u16,
    #[allow(dead_code)]
    pub connected: bool,
}

impl Default for PortInfo {
    fn default() -> Self {
        PortInfo { type_id: 0, connected: false }
    }
}

pub struct BuildHATShared {
    pub ports: [PortInfo; PORT_COUNT],
    pub sensor_data: HashMap<String, Vec<f64>>,
    pub completions: [bool; PORT_COUNT],
    pub selected_modes: [Option<u8>; PORT_COUNT],
}

impl BuildHATShared {
    fn new() -> Self {
        BuildHATShared {
            ports: Default::default(),
            sensor_data: HashMap::new(),
            completions: [false; PORT_COUNT],
            selected_modes: [None; PORT_COUNT],
        }
    }
}

// ── Driver slot ─────────────────────────────────

struct PendingInit {
    port: u8,
    type_id: u16,
    ready_at: Instant,
}

struct BuildHATSlot {
    port: Box<dyn BuildHATTransport>,
    rx: mpsc::Receiver<BuildHATCommand>,
    shared: Arc<Mutex<BuildHATShared>>,
    read_buffer: String,
    alive: bool,
    pending_inits: Vec<PendingInit>,
}

impl BuildHATSlot {
    fn write_cmd(&mut self, cmd: &str) {
        let _ = self.port.write_all(cmd.as_bytes());
        let _ = self.port.flush();
    }

    fn write_cmd_checked(&mut self, cmd: &str) -> Result<(), String> {
        self.port.write_all(cmd.as_bytes())?;
        self.port.flush()
    }

    fn reply(tx: ReplyTx, result: Result<(), String>) {
        if let Some(tx) = tx {
            let _ = tx.send(result);
        }
    }
}

impl DeviceSlot for BuildHATSlot {
    fn tick(&mut self) {
        // ── Read serial data ──────────────────
        let mut buf = [0u8; 512];
        match self.port.read(&mut buf) {
            Ok(n) if n > 0 => {
                self.read_buffer.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            _ => {}
        }

        // Process complete lines
        while let Some(newline_pos) = self.read_buffer.find('\n') {
            let line = self.read_buffer[..newline_pos].trim().to_string();
            self.read_buffer = self.read_buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let mut shared = self.shared.lock().unwrap();

            // Check for sensor data
            if let Some(data) = parse_sensor_data(&line) {
                let key = format!("{}:{}", data.port, data.mode);
                shared.sensor_data.insert(key, data.values);
            }

            // Check for command completion
            if let Some((port, _kind)) = parse_completion(&line) {
                if (port as usize) < PORT_COUNT {
                    shared.completions[port as usize] = true;
                }
            }

            // Check for device attach/detach
            if let Some(dev) = parse_device_line(&line) {
                if (dev.port as usize) < PORT_COUNT {
                    let is_new = !shared.ports[dev.port as usize].connected
                        || shared.ports[dev.port as usize].type_id != dev.type_id;
                    shared.ports[dev.port as usize] = PortInfo {
                        type_id: dev.type_id,
                        connected: true,
                    };
                    // Queue deferred init for newly attached device (1 sec delay)
                    if is_new {
                        self.pending_inits.push(PendingInit {
                            port: dev.port,
                            type_id: dev.type_id,
                            ready_at: Instant::now() + Duration::from_secs(1),
                        });
                    }
                }
            }

            if line.starts_with('P') && (line.contains("no device") || line.contains("disconnected") || line.contains("timeout")) {
                if let Some(port_byte) = line.as_bytes().get(1) {
                    let port = port_byte.wrapping_sub(b'0') as usize;
                    if port < PORT_COUNT {
                        shared.ports[port] = PortInfo::default();
                        shared.selected_modes[port] = None;
                    }
                }
            }
        }

        // ── Process deferred device inits ─────
        let now = Instant::now();
        let all_inits: Vec<PendingInit> = self.pending_inits.drain(..).collect();
        let mut remaining = Vec::new();
        let mut ready = Vec::new();
        for init in all_inits {
            if now >= init.ready_at {
                ready.push(init);
            } else {
                remaining.push(init);
            }
        }
        self.pending_inits = remaining;
        for init in ready {
            if needs_led_init(init.type_id) {
                self.write_cmd(&cmd_set_value(init.port, -1));
            }
            if is_motor(init.type_id) {
                self.write_cmd(&cmd_plimit(init.port, 1.0));
            }
            if is_tacho_motor(init.type_id) {
                let modes = if is_absolute_motor(init.type_id) {
                    vec![(1, 0), (2, 0), (3, 0)]
                } else {
                    vec![(1, 0), (2, 0)]
                };
                self.write_cmd(&cmd_select_combi(init.port, 0, &modes, 10));
            }
        }

        // ── Drain command queue ───────────────
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                BuildHATCommand::Raw(s) => self.write_cmd(&s),
                BuildHATCommand::MotorSet { port, speed, reply_tx } => {
                    let r = self.write_cmd_checked(&cmd_motor_set(port, speed));
                    Self::reply(reply_tx, r);
                }
                BuildHATCommand::MotorSpeed { port, speed, reply_tx } => {
                    let r = self.write_cmd_checked(&cmd_motor_speed(port, speed));
                    Self::reply(reply_tx, r);
                }
                BuildHATCommand::MotorCoast { port, reply_tx } => {
                    let r = self.write_cmd_checked(&cmd_motor_coast(port));
                    Self::reply(reply_tx, r);
                }
                BuildHATCommand::MotorOff { port, reply_tx } => {
                    let r = self.write_cmd_checked(&cmd_motor_off(port));
                    Self::reply(reply_tx, r);
                }
                BuildHATCommand::MotorPulse { port, speed, seconds } => {
                    {
                        let mut shared = self.shared.lock().unwrap();
                        shared.completions[port as usize] = false;
                    }
                    self.write_cmd(&cmd_motor_pulse(port, speed, seconds));
                }
                BuildHATCommand::MotorRamp { port, from, to, duration } => {
                    {
                        let mut shared = self.shared.lock().unwrap();
                        shared.completions[port as usize] = false;
                    }
                    self.write_cmd(&cmd_motor_ramp(port, from, to, duration));
                }
                BuildHATCommand::SelectMode { port, mode } => {
                    self.write_cmd(&cmd_select_mode(port, mode, SENSOR_POLL_INTERVAL_MS));
                }
                BuildHATCommand::SelectCombi { port, combi_index, modes, interval_ms } => {
                    self.write_cmd(&cmd_select_combi(port, combi_index, &modes, interval_ms));
                }
                BuildHATCommand::SetValue { port, value } => {
                    self.write_cmd(&cmd_set_value(port, value));
                }
                BuildHATCommand::Preset { port, mode, value } => {
                    self.write_cmd(&cmd_preset(port, mode, value));
                }
                BuildHATCommand::Plimit { port, limit } => {
                    self.write_cmd(&cmd_plimit(port, limit));
                }
            }
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── Helper: speed from direction + power ────────

/// Build HAT native power range is 0..100 (sent as 1% PWM steps).
const MAX_POWER: u8 = 100;

fn to_signed_speed(direction: PortDirection, power: u8) -> i32 {
    let speed = power.min(MAX_POWER) as i32;
    match direction {
        PortDirection::Even => speed,
        PortDirection::Odd => -speed,
    }
}

// ── Adapter ─────────────────────────────────────

pub struct BuildHATAdapter {
    tx: Option<mpsc::Sender<BuildHATCommand>>,
    shared: Arc<Mutex<BuildHATShared>>,
    slot_id: Option<usize>,
    serial_path: String,
    display_name: String,
    port_names: Vec<String>,
}

impl BuildHATAdapter {
    pub fn new() -> Self {
        BuildHATAdapter {
            tx: None,
            shared: Arc::new(Mutex::new(BuildHATShared::new())),
            slot_id: None,
            serial_path: DEFAULT_SERIAL_PATH.to_string(),
            display_name: "Raspberry Pi Build HAT".to_string(),
            port_names: ALL_PORTS.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn require_device(&self, port: u8) -> Result<u16, String> {
        let shared = self.shared.lock().unwrap();
        let idx = port as usize;
        if idx >= PORT_COUNT || !shared.ports[idx].connected {
            return Err(format!("No device on port \"{}\"", port_letter(idx)));
        }
        Ok(shared.ports[idx].type_id)
    }

    fn send_cmd(&self, cmd: BuildHATCommand) -> Result<(), String> {
        self.tx.as_ref().ok_or("Not connected")?
            .send(cmd).map_err(|_| "Send failed".to_string())
    }

    fn send_cmd_wait(&self, cmd_fn: impl FnOnce(ReplyTx) -> BuildHATCommand) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.send_cmd(cmd_fn(Some(tx)))?;
        rx.recv_timeout(Duration::from_millis(500))
            .map_err(|_| "Command timed out".to_string())?
    }

    fn wait_for_completion(&self, port: u8, timeout_secs: u64) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if Instant::now() > deadline {
                return Err("Command timed out".to_string());
            }
            {
                let shared = self.shared.lock().unwrap();
                if shared.completions[port as usize] {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn port_index(&self, port: &str) -> Result<u8, String> {
        port_index(port)
            .map(|i| i as u8)
            .ok_or_else(|| format!("Unknown port \"{}\"", port))
    }

    /// Initialize connected devices after enumeration.
    /// - Color/Distance sensors: send `set -1` to enable LED
    /// - Tacho motors: set up combi mode for continuous position/speed data
    fn init_devices(&self) -> Result<(), String> {
        let ports_snapshot: Vec<(u8, u16)> = {
            let shared = self.shared.lock().unwrap();
            shared.ports.iter().enumerate()
                .filter(|(_, p)| p.connected)
                .map(|(i, p)| (i as u8, p.type_id))
                .collect()
        };

        for (port, type_id) in ports_snapshot {
            if needs_led_init(type_id) {
                // Color and Distance sensors need `set -1` to enable LED
                self.send_cmd(BuildHATCommand::SetValue { port, value: -1 })?;
            }

            if is_motor(type_id) {
                // Default plimit is 0.1 (10%) — set to full power
                self.send_cmd(BuildHATCommand::Plimit { port, limit: 1.0 })?;
            }

            if is_tacho_motor(type_id) {
                if is_absolute_motor(type_id) {
                    // Absolute motor: combi mode with speed(1), position(2), absolute(3) at 10ms
                    self.send_cmd(BuildHATCommand::SelectCombi {
                        port,
                        combi_index: 0,
                        modes: vec![(1, 0), (2, 0), (3, 0)],
                        interval_ms: 10,
                    })?;
                } else {
                    // Tacho motor (no absolute): combi mode with speed(1), position(2) at 10ms
                    self.send_cmd(BuildHATCommand::SelectCombi {
                        port,
                        combi_index: 0,
                        modes: vec![(1, 0), (2, 0)],
                        interval_ms: 10,
                    })?;
                }
            }
        }

        // Give devices time to initialize
        std::thread::sleep(Duration::from_millis(200));
        Ok(())
    }
}

impl HardwareAdapter for BuildHATAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.port_names }
    fn input_ports(&self) -> &[String] { &self.port_names }
    fn connected(&self) -> bool { self.tx.is_some() }

    fn connect(&mut self) -> Result<(), String> {
        if !cfg!(target_os = "linux") {
            return Err("Build HAT is only supported on Raspberry Pi (Linux)".to_string());
        }

        let mut port = serialport::new(&self.serial_path, DEFAULT_BAUD_RATE)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| format!("Failed to open {}: {}", self.serial_path, e))?;

        // Detect state and upload firmware if needed
        let mut state = firmware::detect_state(&mut *port)?;
        if let HatState::Bootloader = state {
            // Load bundled firmware. `resolve_bundled` tries CWD first, then
            // the bricklogo install dir (`~/.bricklogo/firmware/buildhat/`),
            // so running the binary from anywhere on PATH still works.
            let cwd = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let fw_path = bricklogo_lang::paths::resolve_bundled(
                "firmware/buildhat/buildhat-firmware-1902784.bin",
                &cwd,
                "",
            );
            let sig_path = bricklogo_lang::paths::resolve_bundled(
                "firmware/buildhat/buildhat-signature-1902784.bin",
                &cwd,
                "",
            );
            let fw_data = std::fs::read(&fw_path)
                .map_err(|e| format!("Cannot read Build HAT firmware at {}: {}", fw_path.display(), e))?;
            let sig_data = std::fs::read(&sig_path)
                .map_err(|e| format!("Cannot read Build HAT signature at {}: {}", sig_path.display(), e))?;
            let progress: firmware::ProgressFn = Box::new(|_| {});
            firmware::upload_firmware(&mut *port, &fw_data, &sig_data, &progress)?;

            // Verify firmware is now running
            state = firmware::detect_state(&mut *port)
                .map_err(|_| "Build HAT firmware uploaded but did not start".to_string())?;
            if let HatState::Bootloader = state {
                return Err("Build HAT still in bootloader after firmware upload".to_string());
            }
        }

        // Initialize
        Write::write_all(&mut port, cmd_echo_off().as_bytes()).map_err(|e| e.to_string())?;
        Write::flush(&mut port).map_err(|e| e.to_string())?;
        Write::write_all(&mut port, cmd_select_all_ports().as_bytes()).map_err(|e| e.to_string())?;
        Write::flush(&mut port).map_err(|e| e.to_string())?;
        Write::write_all(&mut port, cmd_list().as_bytes()).map_err(|e| e.to_string())?;
        Write::flush(&mut port).map_err(|e| e.to_string())?;

        // Wait for device enumeration to complete
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut buf = [0u8; 512];
        let mut response = String::new();
        let mut init_done = false;

        while Instant::now() < deadline && !init_done {
            match Read::read(&mut port, &mut buf) {
                Ok(n) if n > 0 => {
                    response.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if response.contains("Done initialising") {
                        init_done = true;
                    }
                }
                _ => {}
            }
        }

        // Parse initially discovered devices into shared state
        let shared = Arc::new(Mutex::new(BuildHATShared::new()));
        for line in response.lines() {
            if let Some(dev) = parse_device_line(line) {
                let idx = dev.port as usize;
                if idx < PORT_COUNT {
                    shared.lock().unwrap().ports[idx] = PortInfo {
                        type_id: dev.type_id,
                        connected: true,
                    };
                }
            }
        }

        // Create driver slot — it will handle ongoing attach/detach events
        let (tx, rx) = mpsc::channel();
        let slot = BuildHATSlot {
            port: Box::new(port),
            rx,
            shared: shared.clone(),
            read_buffer: String::new(),
            alive: true,
            pending_inits: Vec::new(),
        };

        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.shared = shared;
        self.slot_id = Some(slot_id);

        // Initialize connected devices
        self.init_devices()?;

        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
    }

    fn max_power(&self) -> u8 { MAX_POWER }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        let idx = self.port_index(port)?;
        let type_id = self.require_device(idx)?;
        if !is_motor(type_id) {
            return Err(format!("Device on port \"{}\" is not a motor", port));
        }
        Ok(())
    }

    fn validate_sensor_port(&self, port: &str, _mode: Option<&str>) -> Result<(), String> {
        let idx = self.port_index(port)?;
        self.require_device(idx)?;
        Ok(())
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let idx = self.port_index(port)?;
        self.require_device(idx)?;
        let speed = to_signed_speed(direction, power);
        self.send_cmd_wait(|reply_tx| BuildHATCommand::MotorSet { port: idx, speed, reply_tx })
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let idx = self.port_index(port)?;
        self.send_cmd_wait(|reply_tx| BuildHATCommand::MotorCoast { port: idx, reply_tx })
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        let idx = self.port_index(port)?;
        let type_id = self.require_device(idx)?;
        if is_tacho_motor(type_id) {
            // Tacho motor: use PID pulse (firmware handles timing)
            let speed = to_signed_speed(direction, power);
            let seconds = tenths as f64 / 10.0;
            {
                let mut shared = self.shared.lock().unwrap();
                shared.completions[idx as usize] = false;
            }
            self.send_cmd(BuildHATCommand::MotorPulse { port: idx, speed, seconds })?;
            self.wait_for_completion(idx, tenths as u64 / 10 + 5)
        } else {
            // Basic motor: PWM + sleep + coast
            self.start_port(port, direction, power)?;
            std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
            self.stop_port(port)
        }
    }

    fn rotate_port_by_degrees(&mut self, port: &str, direction: PortDirection, power: u8, degrees: i32) -> Result<(), String> {
        let idx = self.port_index(port)?;
        let type_id = self.require_device(idx)?;
        if !is_tacho_motor(type_id) {
            return Err("This motor does not support rotation by degrees".to_string());
        }
        let speed = to_signed_speed(direction, power).abs().max(1);
        // Get current position from combi data (index 1 = position/mode 2)
        let current = {
            let shared = self.shared.lock().unwrap();
            shared.sensor_data.get(&format!("{}:0", idx))
                .and_then(|v| v.get(1).copied())
                .unwrap_or(0.0)
        };
        let from_rot = current / 360.0;
        let delta = if direction == PortDirection::Even { degrees } else { -degrees };
        let to_rot = from_rot + (delta as f64 / 360.0);
        let duration = (degrees.abs() as f64 / 360.0) / (speed as f64 / 100.0);
        {
            let mut shared = self.shared.lock().unwrap();
            shared.completions[idx as usize] = false;
        }
        self.send_cmd(BuildHATCommand::MotorRamp { port: idx, from: from_rot, to: to_rot, duration })?;
        self.wait_for_completion(idx, duration as u64 + 5)
    }

    fn rotate_port_to_position(&mut self, port: &str, direction: PortDirection, power: u8, position: i32) -> Result<(), String> {
        let idx = self.port_index(port)?;
        let type_id = self.require_device(idx)?;
        if !is_tacho_motor(type_id) {
            return Err("This motor does not support rotation to position".to_string());
        }
        // Read current encoder position, compute mod-360 delta respecting
        // direction, then delegate to rotate_port_by_degrees.
        let current = {
            let shared = self.shared.lock().unwrap();
            shared.sensor_data.get(&format!("{}:0", idx))
                .and_then(|v| v.get(1).copied())
                .unwrap_or(0.0)
        } as i32;
        let delta = crate::adapter::rotateto_delta(current, position, direction);
        if delta == 0 {
            return Ok(());
        }
        let delta_dir = if delta > 0 { PortDirection::Even } else { PortDirection::Odd };
        self.rotate_port_by_degrees(port, delta_dir, power, delta.abs())
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        let idx = self.port_index(port)?;
        let type_id = self.require_device(idx)?;
        if !is_tacho_motor(type_id) {
            return Err("This device does not support position reset".to_string());
        }
        // Reset position (mode 2) to 0 using preset command
        self.send_cmd(BuildHATCommand::Preset { port: idx, mode: 2, value: 0.0 })
    }

    fn rotate_to_abs(&mut self, port: &str, direction: PortDirection, power: u8, position: i32) -> Result<(), String> {
        let idx = self.port_index(port)?;
        let type_id = self.require_device(idx)?;
        if !is_absolute_motor(type_id) {
            return Err(format!(
                "Motor on port \"{}\" has no absolute-position encoder",
                port
            ));
        }
        let apos = match self.read_sensor(port, Some("absolute"))? {
            Some(LogoValue::Number(n)) => n as i32,
            _ => return Err("Could not read absolute position".to_string()),
        };
        let delta = crate::adapter::rotate_abs_delta(apos, position, direction);
        if delta == 0 {
            return Ok(());
        }
        let delta_dir = if delta > 0 {
            PortDirection::Even
        } else {
            PortDirection::Odd
        };
        self.rotate_port_by_degrees(port, delta_dir, power, delta.abs())
    }

    fn rotate_ports_to_abs(&mut self, commands: &[PortCommand], position: i32) -> Result<(), String> {
        // Plan every port first (read APOS, compute delta, build MotorRamp
        // args), then fire all commands without blocking between them, and
        // finally poll each port's completion flag. This gives within-hub
        // parallelism: the motors ramp toward home simultaneously instead
        // of one-at-a-time through the default per-port loop.
        let mut plans: Vec<(u8, f64, f64, f64)> = Vec::new();
        for cmd in commands {
            let idx = self.port_index(cmd.port)?;
            let type_id = self.require_device(idx)?;
            if !is_absolute_motor(type_id) {
                return Err(format!(
                    "Motor on port \"{}\" has no absolute-position encoder",
                    cmd.port
                ));
            }
            let apos = match self.read_sensor(cmd.port, Some("absolute"))? {
                Some(LogoValue::Number(n)) => n as i32,
                _ => return Err("Could not read absolute position".to_string()),
            };
            let delta = crate::adapter::rotate_abs_delta(apos, position, cmd.direction);
            if delta == 0 {
                continue;
            }
            let degrees = delta.unsigned_abs() as i32;
            let speed = to_signed_speed(cmd.direction, cmd.power).abs().max(1);
            let current = {
                let shared = self.shared.lock().unwrap();
                shared.sensor_data.get(&format!("{}:0", idx))
                    .and_then(|v| v.get(1).copied())
                    .unwrap_or(0.0)
            };
            let from_rot = current / 360.0;
            let to_rot = from_rot + (delta as f64 / 360.0);
            let duration = (degrees as f64 / 360.0) / (speed as f64 / 100.0);
            plans.push((idx, from_rot, to_rot, duration));
        }

        if plans.is_empty() {
            return Ok(());
        }

        // Clear completion flags, then queue all ramps without waiting.
        {
            let mut shared = self.shared.lock().unwrap();
            for &(idx, _, _, _) in &plans {
                shared.completions[idx as usize] = false;
            }
        }
        for &(idx, from, to, duration) in &plans {
            self.send_cmd(BuildHATCommand::MotorRamp {
                port: idx,
                from,
                to,
                duration,
            })?;
        }

        // Wait for every port to signal completion. Timeout is the longest
        // planned duration plus a small margin.
        let max_duration = plans.iter().map(|p| p.3).fold(0.0f64, f64::max);
        let timeout_secs = (max_duration as u64) + 5;
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if Instant::now() > deadline {
                return Err("Command timed out".to_string());
            }
            let all_done = {
                let shared = self.shared.lock().unwrap();
                plans.iter().all(|&(idx, _, _, _)| shared.completions[idx as usize])
            };
            if all_done {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let idx = self.port_index(port)?;
        let device_type = {
            let shared = self.shared.lock().unwrap();
            shared.ports[idx as usize].type_id
        };

        // Map mode name to Build HAT mode number (same as Powered UP — same LPF2 devices)
        let mode_num = match (mode, device_type) {
            // Technic Color Sensor (61)
            (Some("color"), DEVICE_COLOR_SENSOR) => 0,
            (Some("light"), DEVICE_COLOR_SENSOR) => 1,
            (Some("ambient"), DEVICE_COLOR_SENSOR) => 2,
            (Some("rgb"), DEVICE_COLOR_SENSOR) => 5,
            (Some("hsv"), DEVICE_COLOR_SENSOR) => 6,
            (Some("hsvambient"), DEVICE_COLOR_SENSOR) => 7,
            // Color Distance Sensor (37)
            (Some("color"), DEVICE_COLOR_DISTANCE_SENSOR) => 0,
            (Some("distance"), DEVICE_COLOR_DISTANCE_SENSOR) => 1,
            (Some("distanceCount"), DEVICE_COLOR_DISTANCE_SENSOR) => 2,
            (Some("light"), DEVICE_COLOR_DISTANCE_SENSOR) => 3,
            (Some("ambient"), DEVICE_COLOR_DISTANCE_SENSOR) => 4,
            (Some("rgb"), DEVICE_COLOR_DISTANCE_SENSOR) => 6,
            (Some("colordistance"), DEVICE_COLOR_DISTANCE_SENSOR) => 8,
            // Technic Distance Sensor (62)
            (Some("distance"), DEVICE_DISTANCE_SENSOR) => 0,
            (Some("fastDistance"), DEVICE_DISTANCE_SENSOR) => 1,
            // Technic Force Sensor (63)
            (Some("force"), DEVICE_FORCE_SENSOR) => 0,
            (Some("touched"), DEVICE_FORCE_SENSOR) => 1,
            (Some("tapped"), DEVICE_FORCE_SENSOR) => 2,
            // Tilt Sensor (34)
            (Some("tilt"), DEVICE_TILT_SENSOR) => 0,
            (Some("direction"), DEVICE_TILT_SENSOR) => 1,
            (Some("impactCount"), DEVICE_TILT_SENSOR) => 2,
            (Some("accel"), DEVICE_TILT_SENSOR) => 3,
            // Motion Sensor (35)
            (Some("distance"), DEVICE_MOTION_SENSOR) => 0,
            // Tacho/Absolute motors — mode numbers match Powered UP
            (Some("rotation"), _) if is_tacho_motor(device_type) => 2,
            (Some("speed"), _) if is_tacho_motor(device_type) => 1,
            (Some("absolute"), _) if is_absolute_motor(device_type) => 3,
            (None, _) => 0, // Default mode
            _ => return Err(format!("Unsupported sensor mode \"{}\" for this device", mode.unwrap_or("none"))),
        };

        // For tacho motors, data comes from combi mode (already set up in init).
        // Extract the correct value from the combi data based on which mode was requested.
        if is_tacho_motor(device_type) {
            let combi_key = format!("{}:0", idx); // combi index 0
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if Instant::now() > deadline {
                    return Err("Sensor read timed out".to_string());
                }
                {
                    let shared = self.shared.lock().unwrap();
                    if let Some(values) = shared.sensor_data.get(&combi_key) {
                        // Combi layout: [speed(1), position(2)] or [speed(1), position(2), absolute(3)]
                        let value = match mode_num {
                            1 => values.first().copied(),     // speed is index 0 in combi
                            2 => values.get(1).copied(),      // position is index 1
                            3 => values.get(2).copied(),      // absolute is index 2
                            _ => None,
                        };
                        if let Some(v) = value {
                            return Ok(Some(LogoValue::Number(v)));
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(16));
            }
        }

        // Only send SelectMode if the mode changed — avoid flooding the Build HAT
        let mode_changed = {
            let shared = self.shared.lock().unwrap();
            shared.selected_modes[idx as usize] != Some(mode_num)
        };

        let key = format!("{}:{}", idx, mode_num);

        if mode_changed {
            // Clear stale data from previous mode and select the new one
            {
                let mut shared = self.shared.lock().unwrap();
                shared.sensor_data.remove(&key);
                shared.selected_modes[idx as usize] = Some(mode_num);
            }
            self.send_cmd(BuildHATCommand::SelectMode { port: idx, mode: mode_num })?;
        }

        // Wait for data (fresh if mode just changed, latest from stream otherwise)
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if Instant::now() > deadline {
                return Err("Sensor read timed out".to_string());
            }
            {
                let shared = self.shared.lock().unwrap();
                if let Some(values) = shared.sensor_data.get(&key) {
                    if values.len() == 1 {
                        return Ok(Some(LogoValue::Number(values[0])));
                    } else if values.len() > 1 {
                        return Ok(Some(LogoValue::List(
                            values.iter().map(|v| LogoValue::Number(*v)).collect()
                        )));
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

}

#[cfg(test)]
#[path = "../tests/buildhat_adapter.rs"]
mod tests;
