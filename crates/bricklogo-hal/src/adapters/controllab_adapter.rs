use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use bricklogo_lang::value::LogoValue;
use rust_controllab::constants::*;
use rust_controllab::controllab::{self, ControlLabSensorPayload, process_sensor_data};
use rust_controllab::protocol::{encode_keep_alive, encode_output_power, get_output_port_mask};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Serial transport abstraction — lets tests inject a mock without a real
/// Interface B plugged in.
pub trait ControlLabTransport: Send {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String>;
    fn write_all(&mut self, data: &[u8]) -> Result<(), String>;
    fn flush(&mut self) -> Result<(), String>;
}

impl ControlLabTransport for Box<dyn serialport::SerialPort> {
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

const OUTPUT_PORTS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h"];
const INPUT_PORTS: &[&str] = &["1", "2", "3", "4", "5", "6", "7", "8"];

const SENSOR_MODE_MAP: &[(&str, SensorType)] = &[
    ("touch", SensorType::Touch),
    ("temperature", SensorType::Temperature),
    ("light", SensorType::Light),
    ("rotation", SensorType::Rotation),
];

/// Control Lab native power range is 0..8 (1-8 wire encoded as 0-7,
/// with 0 sent as a separate "off" opcode). See rust-controllab.
const MAX_POWER: u8 = 8;

fn to_signed_power(direction: PortDirection, power: u8) -> i8 {
    let native = power.min(MAX_POWER) as i8;
    match direction {
        PortDirection::Even => native,
        PortDirection::Odd => -native,
    }
}

// ── Driver slot for Control Lab ─────────────────

type ReplyTx = Option<mpsc::Sender<Result<(), String>>>;

enum ControlLabCommand {
    Power { mask: u8, power: i8, reply_tx: ReplyTx },
}

/// Shared state between the adapter and the driver slot.
pub struct ControlLabShared {
    pub sensor_types: [SensorType; INPUT_PORT_COUNT],
    pub rotation_values: [i32; INPUT_PORT_COUNT],
    pub last_payloads: HashMap<String, ControlLabSensorPayload>,
}

impl ControlLabShared {
    fn new() -> Self {
        ControlLabShared {
            sensor_types: [SensorType::Unknown; INPUT_PORT_COUNT],
            rotation_values: [0; INPUT_PORT_COUNT],
            last_payloads: HashMap::new(),
        }
    }
}

struct ControlLabSlot {
    port: Box<dyn ControlLabTransport>,
    rx: mpsc::Receiver<ControlLabCommand>,
    shared: Arc<Mutex<ControlLabShared>>,
    read_buffer: Vec<u8>,
    last_write: Instant,
    alive: bool,
}

impl DeviceSlot for ControlLabSlot {
    fn tick(&mut self) {
        // ── Read sensor data ──────────────────
        let mut buf = [0u8; 256];
        match self.port.read(&mut buf) {
            Ok(n) if n > 0 => self.read_buffer.extend_from_slice(&buf[..n]),
            _ => {}
        }

        {
            let shared = &mut *self.shared.lock().unwrap();
            process_sensor_data(
                &mut self.read_buffer,
                &shared.sensor_types,
                &mut shared.rotation_values,
                &mut shared.last_payloads,
            );
        }

        // ── Drain command queue and batch ─────
        let mut power_groups: HashMap<i8, u8> = HashMap::new();
        let mut reply_senders: Vec<mpsc::Sender<Result<(), String>>> = Vec::new();
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                ControlLabCommand::Power { mask, power, reply_tx } => {
                    *power_groups.entry(power).or_insert(0) |= mask;
                    if let Some(tx) = reply_tx {
                        reply_senders.push(tx);
                    }
                }
            }
        }

        if !power_groups.is_empty() {
            let mut result: Result<(), String> = Ok(());
            for (power, mask) in &power_groups {
                let encoded = encode_output_power(*mask, *power);
                if let Err(e) = self.port.write_all(&encoded) {
                    result = Err(e);
                    break;
                }
            }
            if result.is_ok() {
                if let Err(e) = self.port.flush() {
                    result = Err(e);
                }
            }
            if result.is_ok() {
                self.last_write = Instant::now();
            }
            for tx in reply_senders {
                let _ = tx.send(result.clone());
            }
        }

        // ── Keep-alive ───────────────────────
        if self.last_write.elapsed() >= std::time::Duration::from_millis(KEEP_ALIVE_INTERVAL_MS) {
            let ka = encode_keep_alive();
            let _ = self.port.write_all(&ka);
            let _ = self.port.flush();
            self.last_write = Instant::now();
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── Adapter ─────────────────────────────────────

pub struct ControlLabAdapter {
    tx: Option<mpsc::Sender<ControlLabCommand>>,
    shared: Arc<Mutex<ControlLabShared>>,
    slot_id: Option<usize>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    sensor_types: HashMap<usize, SensorType>,
    serial_path: String,
}

impl ControlLabAdapter {
    pub fn new(serial_path: &str) -> Self {
        ControlLabAdapter {
            tx: None,
            shared: Arc::new(Mutex::new(ControlLabShared::new())),
            slot_id: None,
            display_name: "LEGO Control Lab".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            sensor_types: HashMap::new(),
            serial_path: serial_path.to_string(),
        }
    }
}

impl HardwareAdapter for ControlLabAdapter {
    fn display_name(&self) -> &str {
        &self.display_name
    }
    fn output_ports(&self) -> &[String] {
        &self.output_ports
    }
    fn input_ports(&self) -> &[String] {
        &self.input_ports
    }
    fn connected(&self) -> bool {
        self.tx.is_some()
    }

    fn connect(&mut self) -> Result<(), String> {
        let port = controllab::connect(&self.serial_path, DEFAULT_BAUD_RATE)?;

        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Mutex::new(ControlLabShared::new()));

        let slot = ControlLabSlot {
            port: Box::new(port),
            rx,
            shared: shared.clone(),
            read_buffer: Vec::new(),
            last_write: Instant::now(),
            alive: true,
        };

        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.shared = shared;
        self.slot_id = Some(slot_id);
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
        if OUTPUT_PORTS.contains(&port) {
            Ok(())
        } else {
            Err(format!("Unknown output port \"{}\"", port))
        }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        let input_port: usize = port
            .parse()
            .map_err(|_| format!("Unknown sensor port \"{}\"", port))?;
        if input_port < 1 || input_port > 8 {
            return Err(format!("Unknown sensor port \"{}\"", port));
        }
        if let Some(m) = mode {
            if m != "raw" && !SENSOR_MODE_MAP.iter().any(|(name, _)| *name == m) {
                return Err(format!("Unsupported sensor mode \"{}\" for Control Lab", m));
            }
        }
        Ok(())
    }

    fn start_port(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
    ) -> Result<(), String> {
        let mask = get_output_port_mask(&port.to_uppercase())
            .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let signed = to_signed_power(direction, power);
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(ControlLabCommand::Power {
                mask,
                power: signed,
                reply_tx: Some(tx),
            })
            .map_err(|_| "Send failed".to_string())?;
        rx.recv_timeout(Duration::from_millis(500))
            .map_err(|_| "Command timed out".to_string())?
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let mask = get_output_port_mask(&port.to_uppercase())
            .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(ControlLabCommand::Power { mask, power: 0, reply_tx: Some(tx) })
            .map_err(|_| "Send failed".to_string())?;
        rx.recv_timeout(Duration::from_millis(500))
            .map_err(|_| "Command timed out".to_string())?
    }

    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _degrees: i32,
    ) -> Result<(), String> {
        Err("Control Lab does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _position: i32,
    ) -> Result<(), String> {
        Err("Control Lab does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("Control Lab does not support position reset".to_string())
    }

    fn rotate_to_abs(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _position: i32,
    ) -> Result<(), String> {
        Err("Control Lab does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let input_port: usize = port
            .parse()
            .map_err(|_| format!("Unknown sensor port \"{}\"", port))?;
        if input_port < 1 || input_port > 8 {
            return Err(format!("Unknown sensor port \"{}\"", port));
        }

        // Set sensor type if mode specified and changed
        if let Some(m) = mode {
            if m != "raw" {
                if let Some((_, sensor_type)) = SENSOR_MODE_MAP.iter().find(|(name, _)| *name == m)
                {
                    if self.sensor_types.get(&input_port) != Some(sensor_type) {
                        let mut shared = self.shared.lock().unwrap();
                        shared.sensor_types[input_port - 1] = *sensor_type;
                        shared.rotation_values[input_port - 1] = 0;
                        self.sensor_types.insert(input_port, *sensor_type);
                    }
                }
            }
        }

        let shared = self.shared.lock().unwrap();
        let sensor_type = shared.sensor_types[input_port - 1];
        let kind = match sensor_type {
            SensorType::Touch => "touch",
            SensorType::Temperature => "temperature",
            SensorType::Light => "light",
            SensorType::Rotation => "rotation",
            SensorType::Unknown => {
                if mode == Some("touch") {
                    return Ok(Some(LogoValue::Word("false".to_string())));
                }
                return Ok(Some(LogoValue::Number(0.0)));
            }
        };
        let key = format!("{}:{}", kind, input_port);

        let payload = shared.last_payloads.get(&key);
        if payload.is_none() {
            if mode == Some("touch") {
                return Ok(Some(LogoValue::Word("false".to_string())));
            }
            return Ok(Some(LogoValue::Number(0.0)));
        }

        let payload = payload.unwrap();
        if mode == Some("raw") {
            return match payload {
                ControlLabSensorPayload::Touch(p) => {
                    Ok(Some(LogoValue::Number(p.raw_value as f64)))
                }
                ControlLabSensorPayload::Temperature(p) => {
                    Ok(Some(LogoValue::Number(p.raw_value as f64)))
                }
                ControlLabSensorPayload::Light(p) => {
                    Ok(Some(LogoValue::Number(p.raw_value as f64)))
                }
                ControlLabSensorPayload::Rotation(p) => {
                    Ok(Some(LogoValue::Number(p.raw_value as f64)))
                }
            };
        }

        match payload {
            ControlLabSensorPayload::Touch(p) => Ok(Some(LogoValue::Word(
                if p.pressed { "true" } else { "false" }.to_string(),
            ))),
            ControlLabSensorPayload::Temperature(p) => Ok(Some(LogoValue::Number(p.celsius))),
            ControlLabSensorPayload::Light(p) => Ok(Some(LogoValue::Number(p.intensity as f64))),
            ControlLabSensorPayload::Rotation(p) => Ok(Some(LogoValue::Number(p.rotations as f64))),
        }
    }

    // ── Batch overrides ─────────────────────────

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        let tx_ch = self.tx.as_ref().ok_or("Not connected")?;
        let mut groups: HashMap<i8, u8> = HashMap::new();
        for cmd in commands {
            let power = to_signed_power(cmd.direction, cmd.power);
            let mask = get_output_port_mask(&cmd.port.to_uppercase())
                .ok_or_else(|| format!("Unknown port \"{}\"", cmd.port))?;
            *groups.entry(power).or_insert(0) |= mask;
        }
        let (reply_tx, reply_rx) = mpsc::channel();
        let count = groups.len();
        for (power, mask) in groups {
            tx_ch.send(ControlLabCommand::Power { mask, power, reply_tx: if count == 1 { Some(reply_tx.clone()) } else { Some(reply_tx.clone()) } })
                .map_err(|_| "Send failed".to_string())?;
        }
        // Wait for at least one reply (all go in the same tick batch)
        reply_rx.recv_timeout(Duration::from_millis(500))
            .map_err(|_| "Command timed out".to_string())?
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        let tx_ch = self.tx.as_ref().ok_or("Not connected")?;
        let mut combined_mask: u8 = 0;
        for port in ports {
            let mask = get_output_port_mask(&port.to_uppercase())
                .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
            combined_mask |= mask;
        }
        let (reply_tx, reply_rx) = mpsc::channel();
        tx_ch.send(ControlLabCommand::Power {
            mask: combined_mask,
            power: 0,
            reply_tx: Some(reply_tx),
        })
        .map_err(|_| "Send failed".to_string())?;
        reply_rx.recv_timeout(Duration::from_millis(500))
            .map_err(|_| "Command timed out".to_string())?
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        self.start_ports(commands)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        let ports: Vec<&str> = commands.iter().map(|c| c.port).collect();
        self.stop_ports(&ports)
    }
}

#[cfg(test)]
#[path = "../tests/controllab_adapter.rs"]
mod tests;
