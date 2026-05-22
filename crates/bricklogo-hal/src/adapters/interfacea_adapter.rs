use std::sync::Arc;
use std::time::Duration;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortDirection};
use crate::shared_brick_interface::SharedBrickInterface;

const OUTPUT_PORTS: &[&str] = &["0", "1", "2", "3", "4", "5"];
const INPUT_PORTS: &[&str] = &["6", "7"];
const MAX_POWER: u8 = 255;

fn output_index(port: &str) -> Option<usize> {
    port.parse::<usize>().ok().filter(|&i| i < 6)
}

pub struct InterfaceAAdapter {
    shared: Option<Arc<SharedBrickInterface>>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    duties: [u8; 6],
}

impl InterfaceAAdapter {
    pub fn new_with_shared(shared: Arc<SharedBrickInterface>) -> Self {
        InterfaceAAdapter {
            shared: Some(shared),
            display_name: "LEGO Interface A (BrickInterface)".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            duties: [0u8; 6],
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

impl HardwareAdapter for InterfaceAAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &self.input_ports }
    fn connected(&self) -> bool { self.shared.is_some() }
    fn max_power(&self) -> u8 { MAX_POWER }

    fn connect(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(ref shared) = self.shared {
            if let Ok(mut dev) = shared.device.lock() {
                let _ = dev.set_outputs(&[0u8; 6]);
            }
            shared.release_interfacea();
        }
        self.shared = None;
        self.duties = [0u8; 6];
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if output_index(port).is_some() { Ok(()) }
        else { Err(format!("Unknown output port \"{}\"", port)) }
    }

    fn validate_sensor_port(&self, port: &str, _mode: Option<&str>) -> Result<(), String> {
        if INPUT_PORTS.contains(&port) { Ok(()) }
        else { Err(format!("Unknown sensor port \"{}\"", port)) }
    }

    fn start_port(&mut self, port: &str, _direction: PortDirection, power: u8) -> Result<(), String> {
        let idx = output_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.duties[idx] = power;
        let duties = self.duties;
        self.device_lock()?.set_output(idx, power, &duties)
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let idx = output_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        self.duties[idx] = 0;
        let duties = self.duties;
        self.device_lock()?.set_output(idx, 0, &duties)
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("Interface A does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("Interface A does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("Interface A does not support position reset".to_string())
    }

    fn rotate_to_abs(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("Interface A does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let bit: u8 = match port {
            "6" => 0,
            "7" => 1,
            _ => return Err(format!("Unknown sensor port \"{}\"", port)),
        };
        let state = self.device_lock()?.get_inputs()?;
        // Firmware: bit=1 means open/pulled-up (Logo "false"), bit=0 means closed (Logo "true").
        let pressed = (state & (1 << bit)) == 0;
        Ok(Some(LogoValue::Word(if pressed { "true" } else { "false" }.to_string())))
    }

    fn read_counter(&mut self, port: &str) -> Result<u32, String> {
        let (c6, c7) = self.device_lock()?.get_counts()?;
        match port {
            "6" => Ok(c6),
            "7" => Ok(c7),
            _ => Err(format!("Unknown sensor port \"{}\"", port)),
        }
    }

    fn reset_counter(&mut self, port: &str) -> Result<(), String> {
        let hw: u8 = match port {
            "6" => 6,
            "7" => 7,
            _ => return Err(format!("Unknown sensor port \"{}\"", port)),
        };
        self.device_lock()?.reset_count(hw)
    }
}

#[cfg(test)]
#[path = "../tests/interfacea_adapter.rs"]
mod tests;
