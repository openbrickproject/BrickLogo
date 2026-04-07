use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use bricklogo_lang::value::LogoValue;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use rust_poweredup::ble::PoweredUpBle;
use rust_poweredup::constants::*;
use rust_poweredup::devices::{self, SensorReading};
use rust_poweredup::protocol;

fn to_signed_speed(direction: PortDirection, power: u8) -> i8 {
    let speed = power.min(100) as i8;
    if direction == PortDirection::Even { speed } else { -speed }
}

pub struct PoweredUpAdapter {
    ble: PoweredUpBle,
}

impl PoweredUpAdapter {
    pub fn new() -> Self {
        PoweredUpAdapter {
            ble: PoweredUpBle::new(),
        }
    }

    pub fn set_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.ble.set_stop_flag(flag);
    }

    /// Get port name → port ID mapping for the current hub type.
    fn port_letters(&self) -> Vec<(&'static str, u8)> {
        let hub = self.ble.hub.lock().unwrap();
        match hub.hub_type {
            HubType::WeDo2SmartHub => vec![("a", 1), ("b", 2)],
            HubType::MoveHub => vec![("a", 0), ("b", 1), ("c", 2), ("d", 3)],
            HubType::Hub => vec![("a", 0), ("b", 1)],
            HubType::RemoteControl => vec![("a", 0), ("b", 1)],
            HubType::DuploTrainBase => vec![("a", 0)],
            HubType::TechnicMediumHub => vec![("a", 0), ("b", 1), ("c", 2), ("d", 3)],
            HubType::TechnicSmallHub => vec![("a", 0), ("b", 1)],
            _ => vec![],
        }
    }

    /// Build the full port map (name → port ID) including internal sensors.
    fn build_port_map(&self) -> HashMap<String, u8> {
        let mut map = HashMap::new();
        let letters = self.port_letters();
        for (name, id) in &letters {
            map.insert(name.to_string(), *id);
        }

        let hub = self.ble.hub.lock().unwrap();
        for device in hub.get_attached_devices() {
            // If port ID doesn't match any letter, it's internal
            if !letters.iter().any(|(_, id)| *id == device.port_id) {
                let name = internal_port_name(device.device_type);
                map.insert(name, device.port_id);
            }
        }
        map
    }

    fn resolve_port_id(&self, port: &str) -> Result<u8, String> {
        let map = self.build_port_map();
        map.get(port)
            .copied()
            .ok_or_else(|| format!("Unknown port \"{}\"", port))
    }
}

/// Map internal device types to port names for sensors built into the hub.
fn internal_port_name(device_type: DeviceType) -> String {
    match device_type {
        DeviceType::TiltSensor | DeviceType::MoveHubTiltSensor |
        DeviceType::TechnicMediumHubTiltSensor => "tilt".to_string(),
        DeviceType::TechnicMediumHubAccelerometer => "accel".to_string(),
        DeviceType::TechnicMediumHubGyroSensor => "gyro".to_string(),
        DeviceType::VoltageSensor => "voltage".to_string(),
        DeviceType::CurrentSensor => "current".to_string(),
        DeviceType::HubLed => "led".to_string(),
        DeviceType::RemoteControlButton => "button".to_string(),
        DeviceType::TechnicMediumHubTemperatureSensor => "temp".to_string(),
        _ => format!("port_{}", device_type as u16),
    }
}

fn reading_to_logo(reading: &SensorReading) -> LogoValue {
    match reading {
        SensorReading::Number(n) => LogoValue::Number(*n),
        SensorReading::Bool(b) => LogoValue::Word(if *b { "true" } else { "false" }.to_string()),
        SensorReading::Pair(a, b) => LogoValue::List(vec![
            LogoValue::Number(*a), LogoValue::Number(*b),
        ]),
        SensorReading::Triple(a, b, c) => LogoValue::List(vec![
            LogoValue::Number(*a), LogoValue::Number(*b), LogoValue::Number(*c),
        ]),
        SensorReading::Quad(a, b, c, d) => LogoValue::List(vec![
            LogoValue::Number(*a), LogoValue::Number(*b),
            LogoValue::Number(*c), LogoValue::Number(*d),
        ]),
    }
}

impl HardwareAdapter for PoweredUpAdapter {
    fn display_name(&self) -> &str {
        // Can't return &str from locked mutex, so use a static-ish approach
        // The hub type doesn't change after connect
        let hub = self.ble.hub.lock().unwrap();
        match hub.hub_type {
            HubType::WeDo2SmartHub => "WeDo 2.0 Smart Hub",
            HubType::MoveHub => "Move Hub",
            HubType::Hub => "Powered UP Hub",
            HubType::RemoteControl => "Remote Control",
            HubType::DuploTrainBase => "Duplo Train Base",
            HubType::TechnicMediumHub => "Technic Medium Hub",
            HubType::TechnicSmallHub => "Technic Small Hub",
            _ => "Powered UP Hub",
        }
    }

    fn output_ports(&self) -> &[String] {
        // Can't return a reference to dynamically computed data.
        // This is called by the port manager for validation.
        // We'll handle validation in validate_output_port instead.
        &[]
    }

    fn input_ports(&self) -> &[String] { &[] }

    fn connected(&self) -> bool { self.ble.is_connected() }

    fn connect(&mut self) -> Result<(), String> {
        super::ble_connect_with_retry(|| self.ble.connect(), 3)?;
        Ok(())
    }

    fn disconnect(&mut self) {
        let _ = self.ble.send(&protocol::cmd_disconnect());
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.ble.disconnect();
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let hub = self.ble.hub.lock().unwrap();
        match hub.get_device(port_id) {
            Some(device) if device.device_type.is_motor() => Ok(()),
            Some(_) => Err(format!("Port \"{}\" is not a motor", port)),
            None => Err(format!("No device on port \"{}\"", port)),
        }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let hub = self.ble.hub.lock().unwrap();
        let device = hub.get_device(port_id)
            .ok_or_else(|| format!("No device on port \"{}\"", port))?;

        if let Some(m) = mode {
            if devices::mode_for_event(device.device_type, m).is_none() {
                return Err(format!("Port \"{}\" does not support mode \"{}\"", port, m));
            }
        }
        Ok(())
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let speed = to_signed_speed(direction, power);
        let cmd = protocol::cmd_set_power(port_id, speed, true);
        self.ble.send(&cmd)
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let cmd = protocol::cmd_motor_stop(port_id, true);
        self.ble.send(&cmd)
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let speed = to_signed_speed(direction, power);

        let is_tacho = {
            let hub = self.ble.hub.lock().unwrap();
            hub.get_device(port_id).map_or(false, |d| d.device_type.is_tacho_motor())
        };

        if is_tacho {
            let time_ms = tenths * 100;
            let cmd = protocol::cmd_start_speed_for_time(port_id, time_ms as u16, speed, 100, BrakingStyle::Float, true);
            self.ble.request(port_id, &cmd)?;
        } else {
            let cmd = protocol::cmd_set_power(port_id, speed, true);
            self.ble.send(&cmd)?;
            std::thread::sleep(std::time::Duration::from_millis((tenths * 100) as u64));
            let cmd = protocol::cmd_motor_stop(port_id, true);
            self.ble.send(&cmd)?;
        }
        Ok(())
    }

    fn rotate_port_by_degrees(&mut self, port: &str, direction: PortDirection, power: u8, degrees: i32) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let speed = to_signed_speed(direction, power);
        let cmd = protocol::cmd_start_speed_for_degrees(port_id, degrees.unsigned_abs(), speed, 100, BrakingStyle::Hold, true);
        self.ble.request(port_id, &cmd)?;
        Ok(())
    }

    fn rotate_port_to_position(&mut self, port: &str, direction: PortDirection, power: u8, position: i32) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let speed = to_signed_speed(direction, power);
        let cmd = protocol::cmd_goto_absolute(port_id, position, speed, 100, BrakingStyle::Hold, true);
        self.ble.request(port_id, &cmd)?;
        Ok(())
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let cmd = protocol::cmd_reset_zero(port_id, true);
        self.ble.send(&cmd)
    }

    fn rotate_to_home(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let port_id = self.resolve_port_id(port)?;
        let speed = to_signed_speed(direction, power);
        let cmd = protocol::cmd_goto_absolute(port_id, 0, speed, 100, BrakingStyle::Hold, true);
        self.ble.request(port_id, &cmd)?;
        Ok(())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let port_id = self.resolve_port_id(port)?;

        let (device_type, current_mode) = {
            let hub = self.ble.hub.lock().unwrap();
            let device = hub.get_device(port_id)
                .ok_or_else(|| format!("No device on port \"{}\"", port))?;
            (device.device_type, device.current_mode)
        };

        // Resolve mode name
        let default_mode_name = devices::default_event(device_type);
        let mode_name = mode.or(default_mode_name)
            .ok_or_else(|| format!("No sensor modes for port \"{}\"", port))?;

        let target_mode = devices::mode_for_event(device_type, mode_name)
            .ok_or_else(|| format!("Unknown mode \"{}\"", mode_name))?;

        // Subscribe if not already on this mode
        if current_mode != Some(target_mode) {
            self.ble.subscribe(port_id, target_mode)?;
            // Wait for first reading
            for _ in 0..20 {
                let hub = self.ble.hub.lock().unwrap();
                if hub.last_reading(port_id).is_some() {
                    let reading = hub.last_reading(port_id).map(reading_to_logo);
                    return Ok(reading);
                }
                drop(hub);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        let hub = self.ble.hub.lock().unwrap();
        let reading = hub.last_reading(port_id).map(reading_to_logo);
        Ok(reading)
    }

    // ── Batch overrides ─────────────────────────

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        // Check which ports are tacho vs basic
        let mut tacho_cmds: Vec<(u8, Vec<u8>)> = Vec::new();
        let mut basic_ports: Vec<(u8, PortDirection, u8)> = Vec::new();

        for cmd in commands {
            let port_id = self.resolve_port_id(cmd.port)?;
            let speed = to_signed_speed(cmd.direction, cmd.power);
            let is_tacho = {
                let hub = self.ble.hub.lock().unwrap();
                hub.get_device(port_id).map_or(false, |d| d.device_type.is_tacho_motor())
            };

            if is_tacho {
                let time_ms = tenths * 100;
                let msg = protocol::cmd_start_speed_for_time(port_id, time_ms as u16, speed, 100, BrakingStyle::Float, true);
                tacho_cmds.push((port_id, msg));
            } else {
                basic_ports.push((port_id, cmd.direction, cmd.power));
            }
        }

        // Start basic motors
        for (port_id, direction, power) in &basic_ports {
            let speed = to_signed_speed(*direction, *power);
            let msg = protocol::cmd_set_power(*port_id, speed, true);
            self.ble.send(&msg)?;
        }

        // Fire all tacho commands, then wait for all feedback
        if !tacho_cmds.is_empty() {
            self.ble.request_all(&tacho_cmds)?;
        }

        // If there were basic motors, they've been running during the tacho wait.
        // If there were ONLY basic motors, sleep for the duration.
        if tacho_cmds.is_empty() && !basic_ports.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        }

        // Stop basic motors
        for (port_id, _, _) in &basic_ports {
            let msg = protocol::cmd_motor_stop(*port_id, true);
            self.ble.send(&msg)?;
        }

        Ok(())
    }

    fn rotate_ports_to_position(&mut self, commands: &[PortCommand], position: i32) -> Result<(), String> {
        let mut cmds: Vec<(u8, Vec<u8>)> = Vec::new();
        for cmd in commands {
            let port_id = self.resolve_port_id(cmd.port)?;
            let speed = to_signed_speed(cmd.direction, cmd.power);
            let msg = protocol::cmd_goto_absolute(port_id, position, speed, 100, BrakingStyle::Hold, true);
            cmds.push((port_id, msg));
        }
        if !cmds.is_empty() {
            self.ble.request_all(&cmds)?;
        }
        Ok(())
    }

    fn rotate_ports_to_home(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        let mut cmds: Vec<(u8, Vec<u8>)> = Vec::new();
        for cmd in commands {
            let port_id = self.resolve_port_id(cmd.port)?;
            let speed = to_signed_speed(cmd.direction, cmd.power);
            let msg = protocol::cmd_goto_absolute(port_id, 0, speed, 100, BrakingStyle::Hold, true);
            cmds.push((port_id, msg));
        }
        if !cmds.is_empty() {
            self.ble.request_all(&cmds)?;
        }
        Ok(())
    }

    fn rotate_ports_by_degrees(&mut self, commands: &[PortCommand], degrees: i32) -> Result<(), String> {
        let mut cmds: Vec<(u8, Vec<u8>)> = Vec::new();
        for cmd in commands {
            let port_id = self.resolve_port_id(cmd.port)?;
            let speed = to_signed_speed(cmd.direction, cmd.power);
            let msg = protocol::cmd_start_speed_for_degrees(port_id, degrees.unsigned_abs(), speed, 100, BrakingStyle::Hold, true);
            cmds.push((port_id, msg));
        }
        self.ble.request_all(&cmds)?;
        Ok(())
    }
}
