use std::collections::HashMap;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortDirection};
use rust_controllab::controllab::ControlLab;
use rust_controllab::constants::SensorType;
use rust_controllab::ControlLabSensorPayload;

const OUTPUT_PORTS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h"];
const INPUT_PORTS: &[&str] = &["1", "2", "3", "4", "5", "6", "7", "8"];

const SENSOR_MODE_MAP: &[(&str, SensorType)] = &[
    ("touch", SensorType::Touch),
    ("temperature", SensorType::Temperature),
    ("light", SensorType::Light),
    ("rotation", SensorType::Rotation),
];

fn to_signed_power(direction: PortDirection, power: u8) -> i8 {
    // Map 0-100% back to 0-8 for Control Lab's native range
    let native = ((power as u16 * 8) / 100).min(8) as i8;
    match direction {
        PortDirection::Even => native,
        PortDirection::Odd => -native,
    }
}

pub struct ControlLabAdapter {
    hub: ControlLab,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    sensor_types: HashMap<usize, SensorType>,
}

impl ControlLabAdapter {
    pub fn new(serial_path: &str) -> Self {
        ControlLabAdapter {
            hub: ControlLab::new(serial_path),
            display_name: "LEGO Control Lab".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            sensor_types: HashMap::new(),
        }
    }
}

impl HardwareAdapter for ControlLabAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &self.input_ports }
    fn connected(&self) -> bool { self.hub.is_connected() }

    fn connect(&mut self) -> Result<(), String> {
        self.hub.connect()
    }

    fn disconnect(&mut self) {
        self.hub.disconnect();
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if OUTPUT_PORTS.contains(&port) { Ok(()) }
        else { Err(format!("Unknown output port \"{}\"", port)) }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        let input_port: usize = port.parse().map_err(|_| format!("Unknown sensor port \"{}\"", port))?;
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

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        self.hub.set_power(&port.to_uppercase(), to_signed_power(direction, power))
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        self.hub.set_power(&port.to_uppercase(), 0)
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("Control Lab does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("Control Lab does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("Control Lab does not support position reset".to_string())
    }

    fn rotate_to_home(&mut self, _port: &str, _direction: PortDirection, _power: u8) -> Result<(), String> {
        Err("Control Lab does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let input_port: usize = port.parse().map_err(|_| format!("Unknown sensor port \"{}\"", port))?;
        if input_port < 1 || input_port > 8 {
            return Err(format!("Unknown sensor port \"{}\"", port));
        }

        // Poll for fresh data
        let _ = self.hub.poll();

        // Set sensor type if mode specified and changed
        if let Some(m) = mode {
            if m != "raw" {
                if let Some((_, sensor_type)) = SENSOR_MODE_MAP.iter().find(|(name, _)| *name == m) {
                    if self.sensor_types.get(&input_port) != Some(sensor_type) {
                        self.hub.set_sensor_type(input_port, *sensor_type);
                        self.sensor_types.insert(input_port, *sensor_type);
                    }
                }
            }
        }

        let payload = self.hub.read(input_port);
        if payload.is_none() {
            // No reading yet — return sensible default
            if mode == Some("touch") { return Ok(Some(LogoValue::Word("false".to_string()))); }
            return Ok(Some(LogoValue::Number(0.0)));
        }

        let payload = payload.unwrap();
        if mode == Some("raw") {
            return match &payload {
                ControlLabSensorPayload::Touch(p) => Ok(Some(LogoValue::Number(p.raw_value as f64))),
                ControlLabSensorPayload::Temperature(p) => Ok(Some(LogoValue::Number(p.raw_value as f64))),
                ControlLabSensorPayload::Light(p) => Ok(Some(LogoValue::Number(p.raw_value as f64))),
                ControlLabSensorPayload::Rotation(p) => Ok(Some(LogoValue::Number(p.raw_value as f64))),
            };
        }

        match payload {
            ControlLabSensorPayload::Touch(p) => Ok(Some(LogoValue::Word(if p.pressed { "true" } else { "false" }.to_string()))),
            ControlLabSensorPayload::Temperature(p) => Ok(Some(LogoValue::Number(p.celsius))),
            ControlLabSensorPayload::Light(p) => Ok(Some(LogoValue::Number(p.intensity as f64))),
            ControlLabSensorPayload::Rotation(p) => Ok(Some(LogoValue::Number(p.rotations as f64))),
        }
    }
}
