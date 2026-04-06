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

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String>;
    fn stop_port(&mut self, port: &str) -> Result<(), String>;
    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String>;
    fn rotate_port_by_degrees(&mut self, port: &str, direction: PortDirection, power: u8, degrees: i32) -> Result<(), String>;
    fn rotate_port_to_position(&mut self, port: &str, direction: PortDirection, power: u8, position: i32) -> Result<(), String>;
    fn reset_port_zero(&mut self, port: &str) -> Result<(), String>;
    fn rotate_to_home(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String>;

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String>;
}
