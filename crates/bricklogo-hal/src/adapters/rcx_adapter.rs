use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::Instant;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::driver::{self, DeviceSlot};
use rust_rcx::constants::*;
use rust_rcx::protocol;

const OUTPUT_PORTS: &[&str] = &["a", "b", "c"];
const INPUT_PORTS: &[&str] = &["1", "2", "3"];

fn to_direction(direction: PortDirection) -> u8 {
    match direction {
        PortDirection::Even => DIR_FORWARD,
        PortDirection::Odd => DIR_REVERSE,
    }
}

fn power_to_rcx(power: u8) -> u8 {
    // Map 0-100% to 0-7 (RCX native range) with rounding
    ((power as u16 * 7 + 50) / 100).min(7) as u8
}

// ── Driver slot for RCX ─────────────────────────

#[derive(Debug, Clone)]
enum RcxCommand {
    SetDirection { mask: u8, direction: u8 },
    SetPower { mask: u8, power: u8 },
    MotorOn { mask: u8 },
    MotorOff { mask: u8 },
    SetSensorType { sensor: u8, sensor_type: u8 },
    SetSensorMode { sensor: u8, mode: u8 },
    ReadSensor { sensor: u8, source: u8, reply_tx: mpsc::Sender<Result<i16, String>> },
}

/// Shared state between the adapter and the driver slot.

/// Trait for RCX tower transport (serial or USB).
pub trait RcxTransport: Send {
    fn send(&mut self, msg: &[u8]) -> Result<(), String>;
    fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String>;
    fn request_firmware(&mut self, msg: &[u8]) -> Result<Vec<u8>, String>;
    fn read_available(&mut self, buf: &mut [u8]) -> Result<usize, String>;
}

/// USB tower transport.
struct UsbTransport {
    handle: rusb::DeviceHandle<rusb::Context>,
    endpoint_in: u8,
    endpoint_out: u8,
}

impl UsbTransport {
    fn request_with_timeout(&mut self, msg: &[u8], timeout_ms: u64) -> Result<Vec<u8>, String> {
        let timeout = std::time::Duration::from_millis(timeout_ms);
        self.handle.write_interrupt(self.endpoint_out, msg, timeout)
            .map_err(|e| format!("USB write failed: {}", e))?;

        let deadline = Instant::now() + std::time::Duration::from_millis(timeout_ms);
        let mut response = Vec::new();
        let mut buf = [0u8; 64];
        while Instant::now() < deadline {
            let remaining = deadline.duration_since(Instant::now());
            match self.handle.read_interrupt(self.endpoint_in, &mut buf, remaining) {
                Ok(n) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    if let Some(payload) = protocol::parse_reply(&response) {
                        return Ok(payload);
                    }
                }
                Ok(_) => {}
                Err(rusb::Error::Timeout) => {}
                Err(e) => return Err(format!("USB read failed: {}", e)),
            }
        }
        Err("RCX reply timed out".to_string())
    }
}

impl RcxTransport for UsbTransport {
    fn send(&mut self, msg: &[u8]) -> Result<(), String> {
        let timeout = std::time::Duration::from_millis(COMMAND_TIMEOUT_MS);
        self.handle.write_interrupt(self.endpoint_out, msg, timeout)
            .map_err(|e| format!("USB write failed: {}", e))?;
        Ok(())
    }

    fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.request_with_timeout(msg, COMMAND_TIMEOUT_MS)
    }

    fn request_firmware(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.request_with_timeout(msg, FIRMWARE_TIMEOUT_MS)
    }

    fn read_available(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        let timeout = std::time::Duration::from_millis(10);
        match self.handle.read_interrupt(self.endpoint_in, buf, timeout) {
            Ok(n) => Ok(n),
            Err(rusb::Error::Timeout) => Ok(0),
            Err(e) => Err(format!("USB read failed: {}", e)),
        }
    }
}

/// Serial tower transport.
struct SerialTransport {
    port: Box<dyn serialport::SerialPort>,
}

impl SerialTransport {
    fn request_with_timeout(&mut self, msg: &[u8], timeout_ms: u64) -> Result<Vec<u8>, String> {
        self.port.write_all(msg).map_err(|e| format!("Write failed: {}", e))?;
        self.port.flush().map_err(|e| format!("Flush failed: {}", e))?;
        let deadline = Instant::now() + std::time::Duration::from_millis(timeout_ms);
        let mut buf = [0u8; 256];
        let mut response = Vec::new();
        let sent_len = msg.len();

        while Instant::now() < deadline {
            match self.port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    // Serial tower echoes — skip past echo
                    if response.len() > sent_len {
                        if let Some(payload) = protocol::parse_reply(&response[sent_len..]) {
                            return Ok(payload);
                        }
                    }
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(format!("Read failed: {}", e)),
            }
        }
        Err("RCX reply timed out".to_string())
    }
}

impl RcxTransport for SerialTransport {
    fn send(&mut self, msg: &[u8]) -> Result<(), String> {
        self.port.write_all(msg).map_err(|e| format!("Write failed: {}", e))?;
        self.port.flush().map_err(|e| format!("Flush failed: {}", e))?;
        Ok(())
    }

    fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.request_with_timeout(msg, COMMAND_TIMEOUT_MS)
    }

    fn request_firmware(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.request_with_timeout(msg, FIRMWARE_TIMEOUT_MS)
    }

    fn read_available(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(format!("Read failed: {}", e)),
        }
    }
}

struct RcxSlot {
    transport: Box<dyn RcxTransport>,
    rx: mpsc::Receiver<RcxCommand>,
    alive: bool,
}

impl RcxSlot {
    fn send_with_retry(&mut self, msg: &[u8]) {
        for _ in 0..COMMAND_RETRIES {
            match self.transport.request(msg) {
                Ok(_) => return,
                Err(_) => continue,
            }
        }
    }
}

impl DeviceSlot for RcxSlot {
    fn tick(&mut self) {
        // Drain command queue
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                RcxCommand::SetDirection { mask, direction } => {
                    let msg = protocol::cmd_set_direction(mask, direction);
                    self.send_with_retry(&msg);
                }
                RcxCommand::SetPower { mask, power } => {
                    let msg = protocol::cmd_set_power(mask, power);
                    self.send_with_retry(&msg);
                }
                RcxCommand::MotorOn { mask } => {
                    let msg = protocol::cmd_set_motor_state(mask, MOTOR_ON);
                    self.send_with_retry(&msg);
                }
                RcxCommand::MotorOff { mask } => {
                    let msg = protocol::cmd_set_motor_state(mask, MOTOR_OFF);
                    self.send_with_retry(&msg);
                }
                RcxCommand::SetSensorType { sensor, sensor_type } => {
                    let msg = protocol::cmd_set_sensor_type(sensor, sensor_type);
                    self.send_with_retry(&msg);
                }
                RcxCommand::SetSensorMode { sensor, mode } => {
                    let msg = protocol::cmd_set_sensor_mode(sensor, mode);
                    self.send_with_retry(&msg);
                }
                RcxCommand::ReadSensor { sensor, source, reply_tx } => {
                    let msg = protocol::cmd_get_value(source, sensor);
                    let result = self.transport.request(&msg)
                        .and_then(|payload| {
                            protocol::reply_value(&payload)
                                .ok_or_else(|| "Invalid sensor reply".to_string())
                        });
                    let _ = reply_tx.send(result);
                }
            }
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── Adapter ─────────────────────────────────────

pub struct RcxAdapter {
    tx: Option<mpsc::Sender<RcxCommand>>,
    slot_id: Option<usize>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    sensor_types: HashMap<usize, u8>,
    serial_path: Option<String>,
}

impl RcxAdapter {
    pub fn new(serial_path: Option<&str>) -> Self {
        RcxAdapter {
            tx: None,
            slot_id: None,
            display_name: "LEGO Mindstorms RCX".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            sensor_types: HashMap::new(),
            serial_path: serial_path.map(|s| s.to_string()),
        }
    }

    fn send_cmd(&self, cmd: RcxCommand) -> Result<(), String> {
        self.tx.as_ref().ok_or("Not connected")?
            .send(cmd).map_err(|_| "Send failed".to_string())
    }

    /// Get the serial path (None = USB tower).
    pub fn serial_path(&self) -> Option<&str> {
        self.serial_path.as_deref()
    }
}

/// Open a fresh RCX transport. Used for firmware upload outside the driver thread.
pub fn open_transport(serial_path: Option<&str>) -> Result<Box<dyn RcxTransport>, String> {
    if let Some(path) = serial_path {
        let serial = rust_rcx::serial::RcxSerial::open(path)?;
        let port = serial.try_clone_port()?;
        Ok(Box::new(SerialTransport { port }))
    } else {
        let usb = rust_rcx::usb::RcxUsb::open()?;
        let (handle, ep_in, ep_out) = usb.into_parts();
        Ok(Box::new(UsbTransport { handle, endpoint_in: ep_in, endpoint_out: ep_out }))
    }
}

impl HardwareAdapter for RcxAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &self.input_ports }
    fn connected(&self) -> bool { self.tx.is_some() }

    fn connect(&mut self) -> Result<(), String> {
        let transport: Box<dyn RcxTransport> = if let Some(ref path) = self.serial_path {
            // Serial IR tower
            let serial = rust_rcx::serial::RcxSerial::open(path)?;
            let port = serial.try_clone_port()?;
            Box::new(SerialTransport { port })
        } else {
            // USB IR tower
            let usb = rust_rcx::usb::RcxUsb::open()?;
            let (handle, ep_in, ep_out) = usb.into_parts();
            Box::new(UsbTransport { handle, endpoint_in: ep_in, endpoint_out: ep_out })
        };

        // Ping the RCX to verify it's there
        let ping_msg = protocol::cmd_alive();
        let mut test_transport = transport;
        match test_transport.request(&ping_msg) {
            Ok(_) => {}
            Err(_) => return Err("No RCX responded (is it turned on?)".to_string()),
        }

        let (tx, rx) = mpsc::channel();

        let slot = RcxSlot {
            transport: test_transport,
            rx,
            alive: true,
        };

        let slot_id = driver::register(Box::new(slot));
        self.tx = Some(tx);
        self.slot_id = Some(slot_id);
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(id) = self.slot_id.take() {
            driver::deregister(id);
        }
        self.tx = None;
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if OUTPUT_PORTS.contains(&port) { Ok(()) }
        else { Err(format!("Unknown output port \"{}\"", port)) }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        if sensor_index(port).is_none() {
            return Err(format!("Unknown sensor port \"{}\"", port));
        }
        if let Some(m) = mode {
            match m {
                "touch" | "light" | "temperature" | "rotation" | "raw" => Ok(()),
                _ => Err(format!("Unsupported sensor mode \"{}\" for RCX", m)),
            }
        } else {
            Ok(())
        }
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let mask = motor_mask(&port.to_uppercase())
            .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.send_cmd(RcxCommand::SetDirection { mask, direction: to_direction(direction) })?;
        self.send_cmd(RcxCommand::SetPower { mask, power: power_to_rcx(power) })?;
        self.send_cmd(RcxCommand::MotorOn { mask })
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let mask = motor_mask(&port.to_uppercase())
            .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.send_cmd(RcxCommand::SetPower { mask, power: 0 })?;
        self.send_cmd(RcxCommand::MotorOff { mask })
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("RCX does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("RCX does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("RCX does not support position reset".to_string())
    }

    fn rotate_to_home(&mut self, _port: &str, _direction: PortDirection, _power: u8) -> Result<(), String> {
        Err("RCX does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let idx = sensor_index(port)
            .ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;

        // Configure sensor type and mode.
        // Always send type to force the RCX to refresh its sensor hardware —
        // without a running program, the RCX won't poll sensors on its own.
        let is_rotation = mode == Some("rotation");
        if let Some(m) = mode {
            let (stype, smode) = match m {
                "touch" => (SENSOR_TYPE_TOUCH, SENSOR_MODE_RAW),
                "light" => (SENSOR_TYPE_LIGHT, SENSOR_MODE_RAW),
                "temperature" => (SENSOR_TYPE_TEMPERATURE, SENSOR_MODE_RAW),
                // Rotation needs angle mode with slope for continuous polling
                "rotation" => (SENSOR_TYPE_ROTATION, SENSOR_MODE_ANGLE | 1),
                "raw" => (SENSOR_TYPE_RAW, SENSOR_MODE_RAW),
                _ => return Err(format!("Unsupported sensor mode \"{}\"", m)),
            };

            let current = self.sensor_types.get(&(idx as usize));
            if current != Some(&stype) || !is_rotation {
                self.send_cmd(RcxCommand::SetSensorType { sensor: idx, sensor_type: stype })?;
                self.send_cmd(RcxCommand::SetSensorMode { sensor: idx, mode: smode })?;
                self.sensor_types.insert(idx as usize, stype);
                // Give the RCX time to read the sensor
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // Rotation uses the RCX's processed value (accumulated count).
        // Other sensors read raw to get fresh values.
        let (reply_tx, reply_rx) = mpsc::channel();
        let source = if is_rotation { SOURCE_SENSOR_VALUE } else { SOURCE_RAW_SENSOR };

        self.send_cmd(RcxCommand::ReadSensor { sensor: idx, source, reply_tx })?;

        let result = reply_rx.recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|_| "Sensor read timed out".to_string())?
            .map_err(|e| e)?;

        match mode {
            Some("touch") => {
                // Raw touch sensor: low value = pressed, high = released
                let pressed = result < 500;
                Ok(Some(LogoValue::Word(if pressed { "true" } else { "false" }.to_string())))
            }
            Some("light") => {
                // Convert raw (0-1023) to percentage (0-100), inverted (higher raw = less light)
                let percent = ((1023 - result.max(0) as u16) as f64 / 1023.0 * 100.0).round();
                Ok(Some(LogoValue::Number(percent)))
            }
            Some("temperature") => {
                // Convert raw to Celsius (corrected formula from Gaston project)
                let celsius = ((817.6 - result as f64) / 10.27 * 10.0).round() / 10.0;
                Ok(Some(LogoValue::Number(celsius)))
            }
            Some("rotation") => {
                // Raw rotation value
                Ok(Some(LogoValue::Number(result as f64)))
            }
            _ => Ok(Some(LogoValue::Number(result as f64))),
        }
    }

    // ── Batch overrides ─────────────────────────

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        // Combine motor masks and batch direction/power/on
        let mut combined_mask: u8 = 0;
        for cmd in commands {
            let mask = motor_mask(&cmd.port.to_uppercase())
                .ok_or_else(|| format!("Unknown port \"{}\"", cmd.port))?;
            combined_mask |= mask;
            // Direction per motor (may differ)
            self.send_cmd(RcxCommand::SetDirection { mask, direction: to_direction(cmd.direction) })?;
        }

        // Power — batch if all same
        let powers: Vec<u8> = commands.iter().map(|c| power_to_rcx(c.power)).collect();
        if powers.windows(2).all(|w| w[0] == w[1]) {
            self.send_cmd(RcxCommand::SetPower { mask: combined_mask, power: powers[0] })?;
        } else {
            for cmd in commands {
                let mask = motor_mask(&cmd.port.to_uppercase()).unwrap();
                self.send_cmd(RcxCommand::SetPower { mask, power: power_to_rcx(cmd.power) })?;
            }
        }

        // Turn all on at once
        self.send_cmd(RcxCommand::MotorOn { mask: combined_mask })
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        let mut combined_mask: u8 = 0;
        for port in ports {
            let mask = motor_mask(&port.to_uppercase())
                .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
            combined_mask |= mask;
        }
        self.send_cmd(RcxCommand::SetPower {
            mask: combined_mask,
            power: 0,
        })?;
        self.send_cmd(RcxCommand::MotorOff {
            mask: combined_mask,
        })
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        self.start_ports(commands)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        let ports: Vec<&str> = commands.iter().map(|c| c.port).collect();
        self.stop_ports(&ports)
    }

    // ── Firmware upload ─────────────────────────

    fn prepare_firmware_upload(&mut self) -> Result<Option<String>, String> {
        self.disconnect();
        Ok(self.serial_path.clone())
    }

    fn reconnect_after_firmware(&mut self) -> Result<(), String> {
        // RCX takes a few seconds to reboot after firmware unlock
        std::thread::sleep(std::time::Duration::from_secs(3));
        self.connect()
    }
}
