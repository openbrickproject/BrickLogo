use bricklogo_lang::value::LogoValue;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortDirection {
    Even,
    Odd,
}

/// Compute the mod-360 delta for `rotateto`. Given the current raw encoder
/// position and a target angle (0..359), returns the number of degrees to
/// move in the direction specified by `direction`:
///
///   - `Even`: forward (positive delta, 0..359)
///   - `Odd`: backward (negative delta, -359..0)
///
/// Returns 0 if the motor is already at the target angle.
pub fn rotateto_delta(current_position: i32, target: i32, direction: PortDirection) -> i32 {
    let current_angle = current_position.rem_euclid(360);
    let raw = (target - current_angle).rem_euclid(360);
    match direction {
        PortDirection::Even => raw,
        PortDirection::Odd => if raw == 0 { 0 } else { raw - 360 },
    }
}

/// Compute the rotation delta for `rotatetoabs` on an absolute-encoder
/// motor, given the current mechanical angle `apos` (in [-180, 180]) and
/// a `target` angle. The result moves the shaft to `target` by less than
/// one revolution, in the direction specified.
///
///   - `Even` (forward): delta in `[0, 360)` — smallest forward rotation.
///   - `Odd` (backward): delta in `(-360, 0]` — smallest backward rotation.
///
/// Bounded via `rem_euclid(360)` so a noisy or out-of-range `apos` never
/// causes more than a single revolution.
pub fn rotate_abs_delta(apos: i32, target: i32, direction: PortDirection) -> i32 {
    let diff = target - apos;
    match direction {
        PortDirection::Even => diff.rem_euclid(360),
        PortDirection::Odd => -((-diff).rem_euclid(360)),
    }
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
    fn rotate_to_abs(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
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

    /// Rotate multiple ports to an absolute position simultaneously.
    fn rotate_ports_to_abs(&mut self, commands: &[PortCommand], position: i32) -> Result<(), String> {
        for cmd in commands {
            self.rotate_to_abs(cmd.port, cmd.direction, cmd.power, position)?;
        }
        Ok(())
    }

}

#[cfg(test)]
#[path = "tests/adapter.rs"]
mod tests;
