use bricklogo_lang::value::LogoValue;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortDirection {
    Even,
    Odd,
}

impl PortDirection {
    pub fn toggle(&self) -> Self {
        match self {
            PortDirection::Even => PortDirection::Odd,
            PortDirection::Odd => PortDirection::Even,
        }
    }
}

impl fmt::Display for PortDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortDirection::Even => write!(f, "even"),
            PortDirection::Odd => write!(f, "odd"),
        }
    }
}

/// Per-port command parameters for batch operations.
pub struct PortCommand<'a> {
    pub port: &'a str,
    pub direction: PortDirection,
    pub power: u8,
}

/// Trait that all hardware adapters must implement.
/// Each adapter represents a connection to a specific LEGO device.
pub trait HardwareAdapter: Send {
    fn display_name(&self) -> &str;
    fn output_ports(&self) -> &[String];
    fn input_ports(&self) -> &[String];
    fn connected(&self) -> bool;

    fn connect(&mut self) -> Result<(), String>;
    fn disconnect(&mut self);

    fn validate_output_port(&self, port: &str) -> Result<(), String>;
    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String>;

    /// Maximum native power value this device accepts. `setpower N` is valid
    /// for `0 <= N <= max_power()`. Values are device-native — e.g. RCX uses
    /// 0-7 (3-bit PWM), Control Lab 0-8, Build HAT / Powered UP 0-100.
    fn max_power(&self) -> u8;

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8)
    -> Result<(), String>;
    fn stop_port(&mut self, port: &str) -> Result<(), String>;
    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String>;
    fn rotate_port_by_degrees(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        degrees: i32,
    ) -> Result<(), String>;
    fn rotate_port_to_position(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
    ) -> Result<(), String>;
    fn reset_port_zero(&mut self, port: &str) -> Result<(), String>;
    fn rotate_to_home(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
    ) -> Result<(), String>;

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String>;

    // ── Batch operations (default: sequential) ───

    /// Start multiple ports simultaneously.
    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        for cmd in commands {
            self.start_port(cmd.port, cmd.direction, cmd.power)?;
        }
        Ok(())
    }

    /// Stop multiple ports simultaneously.
    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        for port in ports {
            self.stop_port(port)?;
        }
        Ok(())
    }

    /// Run multiple ports for the same duration simultaneously.
    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        for cmd in commands {
            self.start_port(cmd.port, cmd.direction, cmd.power)?;
        }
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        for cmd in commands {
            self.stop_port(cmd.port)?;
        }
        Ok(())
    }

    /// Rotate multiple ports by the same degrees simultaneously.
    fn rotate_ports_by_degrees(
        &mut self,
        commands: &[PortCommand],
        degrees: i32,
    ) -> Result<(), String> {
        for cmd in commands {
            self.rotate_port_by_degrees(cmd.port, cmd.direction, cmd.power, degrees)?;
        }
        Ok(())
    }

    /// Rotate multiple ports to the same position simultaneously.
    fn rotate_ports_to_position(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        for cmd in commands {
            self.rotate_port_to_position(cmd.port, cmd.direction, cmd.power, position)?;
        }
        Ok(())
    }

    /// Rotate multiple ports to home (absolute zero) simultaneously.
    fn rotate_ports_to_home(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        for cmd in commands {
            self.rotate_to_home(cmd.port, cmd.direction, cmd.power)?;
        }
        Ok(())
    }

    // ── Firmware upload (default: unsupported) ──

    /// Prepare for firmware upload by disconnecting the driver slot.
    /// Returns transport config (e.g. serial path) for opening a fresh transport.
    fn prepare_firmware_upload(&mut self) -> Result<Option<String>, String> {
        Err("This device does not support firmware upload".to_string())
    }

    /// Reconnect after firmware upload.
    fn reconnect_after_firmware(&mut self) -> Result<(), String> {
        Err("This device does not support firmware reconnect".to_string())
    }
}
