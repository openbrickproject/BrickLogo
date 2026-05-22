use std::sync::Arc;
use std::time::Duration;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::shared_brick_interface::SharedBrickInterface;

const OUTPUT_PORTS: &[&str] = &[
    "red1", "blue1", "red2", "blue2", "red3", "blue3", "red4", "blue4",
];
const MAX_POWER: u8 = 7;

fn parse_port(port: &str) -> Option<(u8, bool)> {
    let (is_blue, num_str) = if let Some(n) = port.strip_prefix("blue") {
        (true, n)
    } else if let Some(n) = port.strip_prefix("red") {
        (false, n)
    } else {
        return None;
    };
    let ch: u8 = num_str.parse().ok()?;
    if ch < 1 || ch > 4 {
        return None;
    }
    Some((ch - 1, is_blue))
}

fn port_index(channel: u8, is_blue: bool) -> usize {
    channel as usize * 2 + if is_blue { 1 } else { 0 }
}

fn to_step(direction: PortDirection, power: u8) -> u8 {
    if power == 0 {
        return 0;
    }
    let p = power.min(7);
    match direction {
        PortDirection::Even => p,
        PortDirection::Odd => 16 - p,
    }
}

pub struct PowerFunctionsAdapter {
    shared: Option<Arc<SharedBrickInterface>>,
    display_name: String,
    output_ports: Vec<String>,
    steps: [u8; 8],
}

impl PowerFunctionsAdapter {
    pub fn new_with_shared(shared: Arc<SharedBrickInterface>) -> Self {
        PowerFunctionsAdapter {
            shared: Some(shared),
            display_name: "LEGO Power Functions (BrickInterface)".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            steps: [0u8; 8],
        }
    }

    fn device_lock(&self) -> Result<std::sync::MutexGuard<'_, rust_brickinterface::BrickInterface>, String> {
        self.shared
            .as_ref()
            .ok_or_else(|| "Not connected".to_string())?
            .device
            .lock()
            .map_err(|_| "Device mutex poisoned".to_string())
    }
}

impl HardwareAdapter for PowerFunctionsAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.shared.is_some() }
    fn max_power(&self) -> u8 { MAX_POWER }

    fn connect(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(ref shared) = self.shared {
            if let Ok(mut dev) = shared.device.lock() {
                let _ = dev.ir_abort_all();
            }
            shared.release_powerfunctions();
        }
        self.shared = None;
        self.steps = [0u8; 8];
    }

    fn parallel_safe(&self) -> bool { false }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if parse_port(port).is_some() {
            Ok(())
        } else {
            Err(format!("Unknown output port \"{}\"", port))
        }
    }

    fn validate_sensor_port(&self, port: &str, _mode: Option<&str>) -> Result<(), String> {
        Err(format!("Power Functions has no sensor ports (got \"{}\")", port))
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let (ch, is_blue) = parse_port(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let step = to_step(direction, power);
        self.steps[port_index(ch, is_blue)] = step;
        self.device_lock()?.pf_send_single_pwm(ch, is_blue, step)
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let (ch, is_blue) = parse_port(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.steps[port_index(ch, is_blue)] = 0;
        self.device_lock()?.pf_send_single_pwm(ch, is_blue, 0)
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("Power Functions does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("Power Functions does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("Power Functions does not support position reset".to_string())
    }

    fn rotate_to_abs(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("Power Functions does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        Err(format!("Power Functions has no sensor ports (got \"{}\")", port))
    }

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        for cmd in commands {
            self.start_port(cmd.port, cmd.direction, cmd.power)?;
        }
        Ok(())
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        for port in ports {
            self.stop_port(port)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/powerfunctions_adapter.rs"]
mod tests;
