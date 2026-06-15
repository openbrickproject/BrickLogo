use std::sync::Arc;
use std::time::Duration;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::shared_brick_interface::SharedBrickInterface;

const OUTPUT_PORTS: &[&str] = &["0", "1", "2", "3", "4", "5", "a", "b", "c"];
const INPUT_PORTS: &[&str] = &["6", "7"];
const MAX_POWER: u8 = 255;

fn output_index(port: &str) -> Option<usize> {
    port.parse::<usize>().ok().filter(|&i| i < 6)
}

// Returns (even_index, odd_index) for TC Logo motor pair ports "a"/"b"/"c".
// Even direction → even_index on, odd_index off.
// Odd direction  → even_index off, odd_index on.
fn pair_indices(port: &str) -> Option<(usize, usize)> {
    match port {
        "a" => Some((0, 1)),
        "b" => Some((2, 3)),
        "c" => Some((4, 5)),
        _ => None,
    }
}

pub struct InterfaceAAdapter {
    shared: Option<Arc<SharedBrickInterface>>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
}

impl InterfaceAAdapter {
    pub fn new_with_shared(shared: Arc<SharedBrickInterface>) -> Self {
        InterfaceAAdapter {
            shared: Some(shared),
            display_name: "LEGO Interface A (BrickInterface)".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
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
                let _ = dev.set_outputs_masked(0x3F, &[0u8; 6]);
            }
            shared.release_interfacea();
        }
        self.shared = None;
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if output_index(port).is_some() || matches!(port, "a" | "b" | "c") { Ok(()) }
        else { Err(format!("Unknown output port \"{}\"", port)) }
    }

    fn validate_sensor_port(&self, port: &str, _mode: Option<&str>) -> Result<(), String> {
        if INPUT_PORTS.contains(&port) { Ok(()) }
        else { Err(format!("Unknown sensor port \"{}\"", port)) }
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        if let Some((even_idx, odd_idx)) = pair_indices(port) {
            let (d_even, d_odd) = match direction {
                PortDirection::Even => (power, 0),
                PortDirection::Odd  => (0, power),
            };
            let mask = (1u8 << even_idx) | (1u8 << odd_idx);
            self.device_lock()?.set_outputs_masked(mask, &[d_even, d_odd])
        } else {
            let idx = output_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
            self.device_lock()?.set_outputs_masked(1 << idx, &[power])
        }
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        if let Some((even_idx, odd_idx)) = pair_indices(port) {
            let mask = (1u8 << even_idx) | (1u8 << odd_idx);
            self.device_lock()?.set_outputs_masked(mask, &[0, 0])
        } else {
            let idx = output_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
            self.device_lock()?.set_outputs_masked(1 << idx, &[0])
        }
    }

    // Coalesce every selected output into a single masked frame, so e.g.
    // `talkto [0 1] on` drives both motors in one set_outputs write rather than
    // two. set_outputs_masked carries a per-output duty array, so ports at
    // different powers still fit in one frame (no per-power grouping needed,
    // unlike Control Lab).
    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        if commands.is_empty() {
            return Ok(());
        }
        let mut mask = 0u8;
        let mut duty = [0u8; 6];
        for cmd in commands {
            if let Some((even_idx, odd_idx)) = pair_indices(cmd.port) {
                let (d_even, d_odd) = match cmd.direction {
                    PortDirection::Even => (cmd.power, 0),
                    PortDirection::Odd => (0, cmd.power),
                };
                mask |= (1u8 << even_idx) | (1u8 << odd_idx);
                duty[even_idx] = d_even;
                duty[odd_idx] = d_odd;
            } else {
                let idx = output_index(cmd.port)
                    .ok_or_else(|| format!("Unknown port \"{}\"", cmd.port))?;
                mask |= 1u8 << idx;
                duty[idx] = cmd.power;
            }
        }
        let values: Vec<u8> = (0u8..6).filter(|&i| mask & (1u8 << i) != 0).map(|i| duty[i as usize]).collect();
        self.device_lock()?.set_outputs_masked(mask, &values)
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        if ports.is_empty() {
            return Ok(());
        }
        let mut mask = 0u8;
        for port in ports {
            if let Some((even_idx, odd_idx)) = pair_indices(port) {
                mask |= (1u8 << even_idx) | (1u8 << odd_idx);
            } else {
                let idx = output_index(port)
                    .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
                mask |= 1u8 << idx;
            }
        }
        let values = vec![0u8; mask.count_ones() as usize];
        self.device_lock()?.set_outputs_masked(mask, &values)
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
        // Firmware: bit=1 means the switch is closed (pressed), bit=0 means open.
        // So a pressed sensor reads "true", matching Control Lab / RCX / Coral.
        let pressed = (state & (1 << bit)) != 0;
        Ok(Some(LogoValue::Word(if pressed { "true" } else { "false" }.to_string())))
    }

    // One `get_inputs` round-trip serves every selected port, so
    // `listento [6 7]; sensor "touch` reads both at once.
    fn read_sensors(&mut self, ports: &[&str], _mode: Option<&str>) -> Result<Vec<Option<LogoValue>>, String> {
        let state = self.device_lock()?.get_inputs()?;
        ports
            .iter()
            .map(|port| {
                let bit: u8 = match *port {
                    "6" => 0,
                    "7" => 1,
                    _ => return Err(format!("Unknown sensor port \"{}\"", port)),
                };
                let pressed = (state & (1 << bit)) != 0;
                Ok(Some(LogoValue::Word(if pressed { "true" } else { "false" }.to_string())))
            })
            .collect()
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
