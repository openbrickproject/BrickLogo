use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use bricklogo_lang::value::LogoValue;
use rust_coral::ble::CoralBle;
use rust_coral::constants::*;
use rust_coral::protocol::{DeviceSensorPayload, MessageType};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn map_direction(direction: PortDirection, port: &str) -> MotorDirection {
    // Left motor (port "a") is physically mirrored
    let invert = port == "a";
    let forward = direction == PortDirection::Even;
    let clockwise = if invert { !forward } else { forward };
    if clockwise {
        MotorDirection::Clockwise
    } else {
        MotorDirection::Counterclockwise
    }
}

fn motor_bits_for_port(port: &str) -> Result<u8, String> {
    match port {
        "a" => Ok(MotorBits::Left as u8),
        "b" => Ok(MotorBits::Right as u8),
        _ => Err(format!("Unknown motor port \"{}\"", port)),
    }
}

pub struct CoralAdapter {
    ble: CoralBle,
    output_ports: Vec<String>,
    port_modes: HashMap<String, Vec<String>>,
    display_name: String,
}

impl CoralAdapter {
    pub fn new() -> Self {
        CoralAdapter {
            ble: CoralBle::new(),
            output_ports: Vec::new(),
            port_modes: HashMap::new(),
            display_name: "LEGO Education Science".to_string(),
        }
    }

    pub fn set_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.ble.set_stop_flag(flag);
    }
}

impl HardwareAdapter for CoralAdapter {
    fn display_name(&self) -> &str {
        &self.display_name
    }
    fn output_ports(&self) -> &[String] {
        &self.output_ports
    }
    fn input_ports(&self) -> &[String] {
        &[]
    }
    fn connected(&self) -> bool {
        self.ble.is_connected()
    }

    fn connect(&mut self) -> Result<(), String> {
        super::ble_connect_with_retry(|| self.ble.connect(), 3)?;

        if let Some(kind) = self.ble.coral.device_kind() {
            self.display_name = kind.display_name().to_string();
            self.port_modes.clear();

            match kind {
                CoralDeviceKind::DoubleMotor => {
                    self.output_ports = vec!["a".to_string(), "b".to_string()];
                    self.port_modes.insert(
                        "a".into(),
                        vec!["rotate".into(), "angle".into(), "speed".into()],
                    );
                    self.port_modes.insert(
                        "b".into(),
                        vec!["rotate".into(), "angle".into(), "speed".into()],
                    );
                    self.port_modes.insert("tilt".into(), vec!["tilt".into()]);
                    self.port_modes.insert("gyro".into(), vec!["gyro".into()]);
                    self.port_modes.insert("accel".into(), vec!["accel".into()]);
                    self.port_modes.insert("yaw".into(), vec!["yaw".into()]);
                }
                CoralDeviceKind::SingleMotor => {
                    self.output_ports = vec!["a".to_string()];
                    self.port_modes.insert(
                        "a".into(),
                        vec!["rotate".into(), "angle".into(), "speed".into()],
                    );
                    self.port_modes.insert("tilt".into(), vec!["tilt".into()]);
                    self.port_modes.insert("gyro".into(), vec!["gyro".into()]);
                    self.port_modes.insert("accel".into(), vec!["accel".into()]);
                    self.port_modes.insert("yaw".into(), vec!["yaw".into()]);
                }
                CoralDeviceKind::ColorSensor => {
                    self.output_ports = Vec::new();
                    self.port_modes.insert("color".into(), vec!["color".into()]);
                    self.port_modes
                        .insert("reflect".into(), vec!["reflect".into()]);
                    self.port_modes.insert("rgb".into(), vec!["rgb".into()]);
                }
                CoralDeviceKind::Controller => {
                    self.output_ports = Vec::new();
                    self.port_modes
                        .insert("button".into(), vec!["button".into(), "touch".into()]);
                    self.port_modes
                        .insert("joystick".into(), vec!["joystick".into()]);
                }
            }
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

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        let valid_modes = self
            .port_modes
            .get(port)
            .ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        if let Some(m) = mode {
            if !valid_modes.iter().any(|v| v == m) {
                return Err(format!("Port \"{}\" does not support mode \"{}\"", port, m));
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
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self
            .ble
            .coral
            .cmd_motor_run(bits, map_direction(direction, port));
        self.ble.send(&cmd)
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_motor_stop(bits);
        self.ble.send(&cmd)
    }

    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self.ble.coral.cmd_motor_run_for_time(
            bits,
            tenths * 100,
            map_direction(direction, port),
        );
        self.ble.request(&cmd)
    }

    fn rotate_port_by_degrees(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        degrees: i32,
    ) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd =
            self.ble
                .coral
                .cmd_motor_run_for_degrees(bits, degrees, map_direction(direction, port));
        self.ble.request(&cmd)
    }

    fn rotate_port_to_position(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
    ) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let dir = map_direction(direction, port);
        let cmd = self
            .ble
            .coral
            .cmd_motor_run_to_absolute_position(bits, position as u16, dir);
        self.ble.request(&cmd)
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_motor_reset_relative_position(bits, 0);
        self.ble.send(&cmd)
    }

    fn rotate_to_home(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
    ) -> Result<(), String> {
        let bits = motor_bits_for_port(port)?;
        let cmd = self.ble.coral.cmd_set_motor_speed(bits, power as i8);
        self.ble.send(&cmd)?;
        let cmd = self.ble.coral.cmd_motor_run_to_absolute_position(
            bits,
            0,
            map_direction(direction, port),
        );
        self.ble.request(&cmd)
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let _ = self.ble.poll();

        // Default to first valid mode for this port
        let valid_modes = self.port_modes.get(port);
        let default_mode = valid_modes
            .and_then(|v| v.first().map(|s| s.as_str()))
            .unwrap_or("rotate");
        let effective_mode = mode.unwrap_or(default_mode);

        let motor_bit_mask: Option<u8> = match port {
            "a" => Some(MotorBits::Left as u8),
            "b" => Some(MotorBits::Right as u8),
            _ => None,
        };

        match effective_mode {
            "color" => match self.ble.coral.read("color") {
                Some(DeviceSensorPayload::Color(c)) => Ok(Some(LogoValue::Number(c.color as f64))),
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "reflect" => match self.ble.coral.read("color") {
                Some(DeviceSensorPayload::Color(c)) => {
                    Ok(Some(LogoValue::Number(c.reflection as f64)))
                }
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "rgb" => match self.ble.coral.read("color") {
                Some(DeviceSensorPayload::Color(c)) => Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(c.raw_red as f64),
                    LogoValue::Number(c.raw_green as f64),
                    LogoValue::Number(c.raw_blue as f64),
                ]))),
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "rotate" | "angle" => {
                if let Some(mask) = motor_bit_mask {
                    match self.ble.coral.read_motor(mask) {
                        Some(DeviceSensorPayload::Motor(m)) => {
                            Ok(Some(LogoValue::Number(m.position as f64)))
                        }
                        _ => Ok(Some(LogoValue::Number(0.0))),
                    }
                } else {
                    Ok(Some(LogoValue::Number(0.0)))
                }
            }
            "speed" => {
                if let Some(mask) = motor_bit_mask {
                    match self.ble.coral.read_motor(mask) {
                        Some(DeviceSensorPayload::Motor(m)) => {
                            Ok(Some(LogoValue::Number(m.speed as f64)))
                        }
                        _ => Ok(Some(LogoValue::Number(0.0))),
                    }
                } else {
                    Ok(Some(LogoValue::Number(0.0)))
                }
            }
            "tilt" => match self.ble.coral.read("motion") {
                Some(DeviceSensorPayload::MotionSensor(m)) => Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(m.pitch as f64),
                    LogoValue::Number(m.roll as f64),
                ]))),
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "gyro" => match self.ble.coral.read("motion") {
                Some(DeviceSensorPayload::MotionSensor(m)) => Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(m.gyroscope_x as f64),
                    LogoValue::Number(m.gyroscope_y as f64),
                    LogoValue::Number(m.gyroscope_z as f64),
                ]))),
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "accel" => match self.ble.coral.read("motion") {
                Some(DeviceSensorPayload::MotionSensor(m)) => Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(m.accelerometer_x as f64),
                    LogoValue::Number(m.accelerometer_y as f64),
                    LogoValue::Number(m.accelerometer_z as f64),
                ]))),
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "yaw" => match self.ble.coral.read("motion") {
                Some(DeviceSensorPayload::MotionSensor(m)) => {
                    Ok(Some(LogoValue::Number(m.yaw as f64)))
                }
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "button" | "touch" => match self.ble.coral.read("button") {
                Some(DeviceSensorPayload::Button(b)) => Ok(Some(LogoValue::Word(
                    if b.pressed { "true" } else { "false" }.to_string(),
                ))),
                _ => Ok(Some(LogoValue::Word("false".to_string()))),
            },
            "joystick" => match self.ble.coral.read("joystick") {
                Some(DeviceSensorPayload::Joystick(j)) => Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(j.left_percent as f64),
                    LogoValue::Number(j.right_percent as f64),
                ]))),
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            _ => Err(format!("Unsupported sensor mode \"{}\"", effective_mode)),
        }
    }

    // ── Batch overrides using motor bitmask ──────

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        // Set speed — batch if all same power
        let powers: Vec<i8> = commands.iter().map(|c| c.power as i8).collect();
        if powers.windows(2).all(|w| w[0] == w[1]) {
            let combined_bits: u8 = commands
                .iter()
                .map(|c| motor_bits_for_port(c.port).unwrap())
                .fold(0u8, |acc, b| acc | b);
            let cmd = self.ble.coral.cmd_set_motor_speed(combined_bits, powers[0]);
            self.ble.send(&cmd)?;
        } else {
            for cmd in commands {
                let bits = motor_bits_for_port(cmd.port)?;
                let speed_cmd = self.ble.coral.cmd_set_motor_speed(bits, cmd.power as i8);
                self.ble.send(&speed_cmd)?;
            }
        }

        // Run — batch if all same direction
        let dirs: Vec<MotorDirection> = commands
            .iter()
            .map(|c| map_direction(c.direction, c.port))
            .collect();
        if dirs.windows(2).all(|w| w[0] == w[1]) {
            let combined_bits: u8 = commands
                .iter()
                .map(|c| motor_bits_for_port(c.port).unwrap())
                .fold(0u8, |acc, b| acc | b);
            let cmd = self.ble.coral.cmd_motor_run(combined_bits, dirs[0]);
            self.ble.send(&cmd)
        } else {
            for cmd in commands {
                let bits = motor_bits_for_port(cmd.port)?;
                let run_cmd = self
                    .ble
                    .coral
                    .cmd_motor_run(bits, map_direction(cmd.direction, cmd.port));
                self.ble.send(&run_cmd)?;
            }
            Ok(())
        }
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        let combined_bits: u8 = ports
            .iter()
            .map(|p| motor_bits_for_port(p).unwrap_or(0))
            .fold(0u8, |acc, b| acc | b);
        let cmd = self.ble.coral.cmd_motor_stop(combined_bits);
        self.ble.send(&cmd)
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        // Set speed per motor
        for cmd in commands {
            let bits = motor_bits_for_port(cmd.port)?;
            let speed_cmd = self.ble.coral.cmd_set_motor_speed(bits, cmd.power as i8);
            self.ble.send(&speed_cmd)?;
        }

        // Check if all directions are the same — can use combined bitmask
        if commands.len() > 1 {
            let dirs: Vec<MotorDirection> = commands
                .iter()
                .map(|c| map_direction(c.direction, c.port))
                .collect();
            if dirs.windows(2).all(|w| w[0] == w[1]) {
                let combined_bits: u8 = commands
                    .iter()
                    .map(|c| motor_bits_for_port(c.port).unwrap())
                    .fold(0u8, |acc, b| acc | b);
                let cmd =
                    self.ble
                        .coral
                        .cmd_motor_run_for_time(combined_bits, tenths * 100, dirs[0]);
                return self.ble.request(&cmd);
            }
        }

        // Different directions: send all, then wait for all results
        let cmd_id = MessageType::MotorRunForTimeCommand as u8;
        let reqs: Vec<(u8, u8, Vec<u8>)> = commands
            .iter()
            .map(|cmd| {
                let bits = motor_bits_for_port(cmd.port).unwrap();
                let dir = map_direction(cmd.direction, cmd.port);
                let msg = self
                    .ble
                    .coral
                    .cmd_motor_run_for_time(bits, tenths * 100, dir);
                (cmd_id, bits, msg)
            })
            .collect();
        self.ble.request_all(&reqs)
    }

    fn rotate_ports_to_position(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        let cmd_id = MessageType::MotorRunToAbsolutePositionCommand as u8;
        let mut reqs: Vec<(u8, u8, Vec<u8>)> = Vec::new();

        for cmd in commands {
            let bits = motor_bits_for_port(cmd.port)?;
            let speed_cmd = self.ble.coral.cmd_set_motor_speed(bits, cmd.power as i8);
            self.ble.send(&speed_cmd)?;
            let dir = map_direction(cmd.direction, cmd.port);
            let msg = self
                .ble
                .coral
                .cmd_motor_run_to_absolute_position(bits, position as u16, dir);
            reqs.push((cmd_id, bits, msg));
        }

        if !reqs.is_empty() {
            self.ble.request_all(&reqs)?;
        }
        Ok(())
    }

    fn rotate_ports_to_home(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        let cmd_id = MessageType::MotorRunToAbsolutePositionCommand as u8;
        let mut reqs: Vec<(u8, u8, Vec<u8>)> = Vec::new();

        for cmd in commands {
            let bits = motor_bits_for_port(cmd.port)?;
            let speed_cmd = self.ble.coral.cmd_set_motor_speed(bits, cmd.power as i8);
            self.ble.send(&speed_cmd)?;
            let msg = self.ble.coral.cmd_motor_run_to_absolute_position(
                bits,
                0,
                map_direction(cmd.direction, cmd.port),
            );
            reqs.push((cmd_id, bits, msg));
        }

        if !reqs.is_empty() {
            self.ble.request_all(&reqs)?;
        }
        Ok(())
    }

    fn rotate_ports_by_degrees(
        &mut self,
        commands: &[PortCommand],
        degrees: i32,
    ) -> Result<(), String> {
        // Set speed per motor
        for cmd in commands {
            let bits = motor_bits_for_port(cmd.port)?;
            let speed_cmd = self.ble.coral.cmd_set_motor_speed(bits, cmd.power as i8);
            self.ble.send(&speed_cmd)?;
        }

        // Send all rotation commands, then wait for all results
        let cmd_id = MessageType::MotorRunForDegreesCommand as u8;
        let reqs: Vec<(u8, u8, Vec<u8>)> = commands
            .iter()
            .map(|cmd| {
                let bits = motor_bits_for_port(cmd.port).unwrap();
                let dir = map_direction(cmd.direction, cmd.port);
                let msg = self.ble.coral.cmd_motor_run_for_degrees(bits, degrees, dir);
                (cmd_id, bits, msg)
            })
            .collect();
        self.ble.request_all(&reqs)
    }
}
