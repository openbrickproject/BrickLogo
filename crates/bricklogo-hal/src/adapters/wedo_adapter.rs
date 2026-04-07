use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use rust_wedo::wedo::{WeDo, WeDoSensorPayload};

fn to_signed_power(direction: PortDirection, power: u8) -> i32 {
    let p = power as i32;
    match direction {
        PortDirection::Even => p,
        PortDirection::Odd => -p,
    }
}

pub struct WeDoAdapter {
    hub: WeDo,
    display_name: String,
    output_ports: Vec<String>,
}

impl WeDoAdapter {
    pub fn new(identifier: Option<&str>) -> Self {
        let hub = match identifier {
            Some(id) if id.starts_with('/') || id.contains('\\') => WeDo::with_path(id),
            Some(id) => WeDo::with_id(id),
            None => WeDo::new(),
        };
        WeDoAdapter {
            hub,
            display_name: "LEGO WeDo".to_string(),
            output_ports: vec!["a".to_string(), "b".to_string()],
        }
    }
}

impl HardwareAdapter for WeDoAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.hub.is_connected() }

    fn connect(&mut self) -> Result<(), String> {
        self.hub.connect()
    }

    fn disconnect(&mut self) {
        self.hub.disconnect();
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        match port {
            "a" | "b" => Ok(()),
            _ => Err(format!("Unknown port \"{}\"", port)),
        }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        self.validate_output_port(port)?;
        if let Some(m) = mode {
            match m {
                "distance" | "tilt" | "raw" => Ok(()),
                _ => Err(format!("Unsupported sensor mode \"{}\" for WeDo", m)),
            }
        } else {
            Ok(())
        }
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let hub_port = port.to_uppercase();
        self.hub.set_power(&hub_port, to_signed_power(direction, power))
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let hub_port = port.to_uppercase();
        self.hub.set_power(&hub_port, 0)
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("WeDo does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("WeDo does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("WeDo does not support position reset".to_string())
    }

    fn rotate_to_home(&mut self, _port: &str, _direction: PortDirection, _power: u8) -> Result<(), String> {
        Err("WeDo does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        // Poll for fresh data
        let _ = self.hub.poll_sensors();

        let hub_port = port.to_uppercase();
        let effective_mode = mode.unwrap_or("distance");

        match effective_mode {
            "distance" => {
                match self.hub.read(&hub_port, "distance") {
                    Some(WeDoSensorPayload::Distance(d)) => Ok(Some(LogoValue::Number(d.distance as f64))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "tilt" => {
                match self.hub.read(&hub_port, "tilt") {
                    Some(WeDoSensorPayload::Tilt(t)) => Ok(Some(LogoValue::Number(t.tilt as u8 as f64))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "raw" => {
                if let Some(p) = self.hub.read(&hub_port, "distance") {
                    match p {
                        WeDoSensorPayload::Distance(d) => return Ok(Some(LogoValue::Number(d.raw_value as f64))),
                        _ => {}
                    }
                }
                if let Some(p) = self.hub.read(&hub_port, "tilt") {
                    match p {
                        WeDoSensorPayload::Tilt(t) => return Ok(Some(LogoValue::Number(t.raw_value as f64))),
                        _ => {}
                    }
                }
                Ok(Some(LogoValue::Number(0.0)))
            }
            _ => Err(format!("Unsupported sensor mode \"{}\" for WeDo", effective_mode)),
        }
    }

    // ── Batch overrides (single HID write for both motors) ──

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        let upper: Vec<(String, i32)> = commands.iter()
            .map(|c| (c.port.to_uppercase(), to_signed_power(c.direction, c.power)))
            .collect();
        let pairs: Vec<(&str, i32)> = upper.iter().map(|(p, v)| (p.as_str(), *v)).collect();
        self.hub.set_powers(&pairs)
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        let upper: Vec<(String, i32)> = ports.iter().map(|p| (p.to_uppercase(), 0_i32)).collect();
        let pairs: Vec<(&str, i32)> = upper.iter().map(|(p, v)| (p.as_str(), *v)).collect();
        self.hub.set_powers(&pairs)
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        self.start_ports(commands)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        let ports: Vec<&str> = commands.iter().map(|c| c.port).collect();
        self.stop_ports(&ports)
    }
}
