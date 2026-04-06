use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortDirection};
use rust_coral::ble::CoralBle;
use rust_coral::constants::*;
use rust_coral::protocol::DeviceSensorPayload;

fn map_direction(direction: PortDirection, port: &str) -> MotorDirection {
    // Left motor (port "a") is physically mirrored
    let invert = port == "a";
    let forward = direction == PortDirection::Even;
    let clockwise = if invert { !forward } else { forward };
    if clockwise { MotorDirection::Clockwise } else { MotorDirection::Counterclockwise }
}

fn motor_bits_for_port(port: &str) -> Result<u8, String> {
    match port {
        "a" => Ok(MotorBits::Left as u8),
        "b" => Ok(MotorBits::Right as u8),
        _ => Err(format!("Unknown port \"{}\"", port)),
    }
}

pub struct CoralAdapter {
    ble: CoralBle,
    output_ports: Vec<String>,
    display_name: String,
}

impl CoralAdapter {
    pub fn new() -> Self {
        CoralAdapter {
            ble: CoralBle::new(),
            output_ports: Vec::new(),
            display_name: "LEGO Education Science".to_string(),
        }
    }

    pub fn set_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.ble.set_stop_flag(flag);
    }
}

impl HardwareAdapter for CoralAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.ble.is_connected() }

    fn connect(&mut self) -> Result<(), String> {
        self.ble.connect()?;

        if let Some(kind) = self.ble.coral.device_kind() {
            self.display_name = kind.display_name().to_string();
            self.output_ports = match kind {
                CoralDeviceKind::DoubleMotor => vec!["a".to_string(), "b".to_string()],
                CoralDeviceKind::SingleMotor => vec!["a".to_string()],
                _ => Vec::new(),
            };
        }
        Ok(())
    }

    fn disconnect(&mut self) {
        self.ble.disconnect();
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if self.output_ports.contains(&port.to_string()) {
            Ok(())
        } else {
            Err(format!("Unknown port \"{}\"", port))
        }
    }

    fn validate_sensor_port(&self, _port: &str, _mode: Option<&str>) -> Result<(), String> {
        // TODO: per-port mode validation
        Ok(())
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self.ble.coral.cmd_motor_run(bits, map_direction(direction, port));
        self.ble.send(&cmd)
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_motor_stop(bits);
        self.ble.send(&cmd)
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self.ble.coral.cmd_motor_run_for_time(bits, tenths * 100, map_direction(direction, port));
        self.ble.request(&cmd)
    }

    fn rotate_port_by_degrees(&mut self, port: &str, direction: PortDirection, power: u8, degrees: i32) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self.ble.coral.cmd_motor_run_for_degrees(bits, degrees, map_direction(direction, port));
        self.ble.request(&cmd)
    }

    fn rotate_port_to_position(&mut self, port: &str, direction: PortDirection, power: u8, position: i32) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        // Read current position, calculate delta, use run_for_degrees
        let _ = self.ble.poll();
        let current = self.ble.coral.read_motor(bits as u8)
            .and_then(|p| {
                if let DeviceSensorPayload::Motor(m) = p {
                    Some(m.position)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let norm_current = ((current % 360) + 360) % 360;
        let norm_target = ((position % 360) + 360) % 360;
        let delta = norm_target - norm_current;
        if delta == 0 { return Ok(()); }

        let abs_delta = delta.abs();
        let dir = map_direction(direction, port);
        let naturally_positive = delta > 0;
        let user_wants_clockwise = dir == MotorDirection::Clockwise;
        let degrees = if naturally_positive == user_wants_clockwise { abs_delta } else { 360 - abs_delta };

        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self.ble.coral.cmd_motor_run_for_degrees(bits, degrees, dir);
        self.ble.request(&cmd)
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_motor_reset_relative_position(bits, 0);
        self.ble.send(&cmd)
    }

    fn rotate_to_home(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self.ble.coral.cmd_motor_run_to_absolute_position(bits, 0, map_direction(direction, port));
        self.ble.request(&cmd)
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let _ = self.ble.poll();

        let motor_bit_mask: Option<u8> = match port {
            "a" => Some(MotorBits::Left as u8),
            "b" => Some(MotorBits::Right as u8),
            _ => None,
        };

        let effective_mode = mode.unwrap_or("rotate");

        match effective_mode {
            "color" => {
                match self.ble.coral.read("color") {
                    Some(DeviceSensorPayload::Color(c)) => Ok(Some(LogoValue::Number(c.color as f64))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "reflect" => {
                match self.ble.coral.read("color") {
                    Some(DeviceSensorPayload::Color(c)) => Ok(Some(LogoValue::Number(c.reflection as f64))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "rgb" => {
                match self.ble.coral.read("color") {
                    Some(DeviceSensorPayload::Color(c)) => Ok(Some(LogoValue::List(vec![
                        LogoValue::Number(c.raw_red as f64),
                        LogoValue::Number(c.raw_green as f64),
                        LogoValue::Number(c.raw_blue as f64),
                    ]))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "rotate" | "angle" => {
                if let Some(mask) = motor_bit_mask {
                    match self.ble.coral.read_motor(mask) {
                        Some(DeviceSensorPayload::Motor(m)) => Ok(Some(LogoValue::Number(m.position as f64))),
                        _ => Ok(Some(LogoValue::Number(0.0))),
                    }
                } else {
                    Ok(Some(LogoValue::Number(0.0)))
                }
            }
            "speed" => {
                if let Some(mask) = motor_bit_mask {
                    match self.ble.coral.read_motor(mask) {
                        Some(DeviceSensorPayload::Motor(m)) => Ok(Some(LogoValue::Number(m.speed as f64))),
                        _ => Ok(Some(LogoValue::Number(0.0))),
                    }
                } else {
                    Ok(Some(LogoValue::Number(0.0)))
                }
            }
            "tilt" => {
                match self.ble.coral.read("motion") {
                    Some(DeviceSensorPayload::MotionSensor(m)) => Ok(Some(LogoValue::List(vec![
                        LogoValue::Number(m.pitch as f64),
                        LogoValue::Number(m.roll as f64),
                    ]))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "gyro" => {
                match self.ble.coral.read("motion") {
                    Some(DeviceSensorPayload::MotionSensor(m)) => Ok(Some(LogoValue::List(vec![
                        LogoValue::Number(m.gyroscope_x as f64),
                        LogoValue::Number(m.gyroscope_y as f64),
                        LogoValue::Number(m.gyroscope_z as f64),
                    ]))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "accel" => {
                match self.ble.coral.read("motion") {
                    Some(DeviceSensorPayload::MotionSensor(m)) => Ok(Some(LogoValue::List(vec![
                        LogoValue::Number(m.accelerometer_x as f64),
                        LogoValue::Number(m.accelerometer_y as f64),
                        LogoValue::Number(m.accelerometer_z as f64),
                    ]))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "yaw" => {
                match self.ble.coral.read("motion") {
                    Some(DeviceSensorPayload::MotionSensor(m)) => Ok(Some(LogoValue::Number(m.yaw as f64))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            "button" | "touch" => {
                match self.ble.coral.read("button") {
                    Some(DeviceSensorPayload::Button(b)) => Ok(Some(LogoValue::Word(
                        if b.pressed { "true" } else { "false" }.to_string()
                    ))),
                    _ => Ok(Some(LogoValue::Word("false".to_string()))),
                }
            }
            "joystick" => {
                match self.ble.coral.read("joystick") {
                    Some(DeviceSensorPayload::Joystick(j)) => Ok(Some(LogoValue::List(vec![
                        LogoValue::Number(j.left_percent as f64),
                        LogoValue::Number(j.right_percent as f64),
                    ]))),
                    _ => Ok(Some(LogoValue::Number(0.0))),
                }
            }
            _ => Err(format!("Unsupported sensor mode \"{}\"", effective_mode)),
        }
    }
}
