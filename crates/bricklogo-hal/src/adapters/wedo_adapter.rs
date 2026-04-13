use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::driver::{self, DeviceSlot};
use bricklogo_lang::value::LogoValue;
use hidapi::{HidApi, HidDevice};
use rust_wedo::constants::*;
use rust_wedo::protocol::{decode_sensor_notification, encode_motor_command, normalize_power};
use rust_wedo::wedo::{
    DistanceSensorPayload, TiltSensorPayload, WeDoSensorPayload, wedo_usb_present,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};

fn to_signed_power(direction: PortDirection, power: u8) -> i32 {
    let p = power as i32;
    match direction {
        PortDirection::Even => p,
        PortDirection::Odd => -p,
    }
}

// ── Driver slot for WeDo ────────────────────────

type ReplyTx = Option<mpsc::Sender<Result<(), String>>>;

struct WeDoMotorCommand {
    motor_a: i8,
    motor_b: i8,
    reply_tx: ReplyTx,
}

/// Shared state between the adapter and the driver slot.
pub struct WeDoShared {
    pub last_payloads: HashMap<String, WeDoSensorPayload>,
}

impl WeDoShared {
    fn new() -> Self {
        WeDoShared {
            last_payloads: HashMap::new(),
        }
    }
}

struct WeDoSlot {
    device: HidDevice,
    rx: mpsc::Receiver<WeDoMotorCommand>,
    shared: Arc<Mutex<WeDoShared>>,
    output_bits: u8,
    motor_values: [i8; 2],
    alive: bool,
}

impl DeviceSlot for WeDoSlot {
    fn tick(&mut self) {
        // ── Drain HID sensor reports ─────────
        let mut buf = [0u8; 8];
        loop {
            match self.device.read(&mut buf) {
                Ok(n) if n >= SENSOR_MESSAGE_LENGTH => {
                    if let Some(notification) = decode_sensor_notification(&buf) {
                        let mut shared = self.shared.lock().unwrap();
                        for sample in &notification.samples {
                            match sample.sensor_type {
                                SensorType::Distance => {
                                    let distance = get_distance(sample.raw_value);
                                    let key = format!("distance:{}", sample.port);
                                    shared.last_payloads.insert(
                                        key,
                                        WeDoSensorPayload::Distance(DistanceSensorPayload {
                                            port: sample.port.clone(),
                                            raw_value: sample.raw_value,
                                            distance,
                                        }),
                                    );
                                }
                                SensorType::Tilt => {
                                    let tilt = get_tilt_event(sample.raw_value);
                                    let key = format!("tilt:{}", sample.port);
                                    shared.last_payloads.insert(
                                        key,
                                        WeDoSensorPayload::Tilt(TiltSensorPayload {
                                            port: sample.port.clone(),
                                            raw_value: sample.raw_value,
                                            tilt,
                                        }),
                                    );
                                }
                                SensorType::Unknown => {}
                            }
                        }
                    }
                }
                _ => break, // No more data (non-blocking)
            }
        }

        // ── Drain motor command queue ────────
        // Take the last command only (most recent state wins)
        // Superseded commands get Ok since they were intentionally replaced
        let mut last_cmd: Option<WeDoMotorCommand> = None;
        while let Ok(cmd) = self.rx.try_recv() {
            if let Some(prev) = last_cmd.take() {
                if let Some(tx) = prev.reply_tx {
                    let _ = tx.send(Ok(()));
                }
            }
            last_cmd = Some(cmd);
        }

        if let Some(cmd) = last_cmd {
            self.motor_values[0] = cmd.motor_a;
            self.motor_values[1] = cmd.motor_b;
            let encoded =
                encode_motor_command(self.output_bits, self.motor_values[0], self.motor_values[1]);
            let result = self.device.write(&encoded)
                .map(|_| ())
                .map_err(|e| format!("HID write failed: {}", e));
            if let Some(tx) = cmd.reply_tx {
                let _ = tx.send(result);
            }
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── Adapter ─────────────────────────────────────

pub struct WeDoAdapter {
    tx: Option<mpsc::Sender<WeDoMotorCommand>>,
    shared: Arc<Mutex<WeDoShared>>,
    slot_id: Option<usize>,
    display_name: String,
    output_ports: Vec<String>,
    motor_values: [i8; 2],
    identifier: Option<String>,
}

impl WeDoAdapter {
    pub fn new(identifier: Option<&str>) -> Self {
        WeDoAdapter {
            tx: None,
            shared: Arc::new(Mutex::new(WeDoShared::new())),
            slot_id: None,
            display_name: "LEGO WeDo".to_string(),
            output_ports: vec!["a".to_string(), "b".to_string()],
            motor_values: [0, 0],
            identifier: identifier.map(|s| s.to_string()),
        }
    }

    fn normalize_port(&self, port: &str) -> Result<usize, String> {
        match port.to_uppercase().as_str() {
            "A" => Ok(0),
            "B" => Ok(1),
            _ => Err(format!("Unknown WeDo port '{}'", port)),
        }
    }

    fn send_motor_state(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(WeDoMotorCommand {
                motor_a: self.motor_values[0],
                motor_b: self.motor_values[1],
                reply_tx: Some(tx),
            })
            .map_err(|_| "Send failed".to_string())?;
        rx.recv_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| "Command timed out".to_string())?
    }
}

impl HardwareAdapter for WeDoAdapter {
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
        self.tx.is_some()
    }

    fn connect(&mut self) -> Result<(), String> {
        if !wedo_usb_present() {
            return Err("No WeDo device found".to_string());
        }

        let api = HidApi::new().map_err(|e| format!("Failed to init HID: {}", e))?;

        let device = if let Some(ref id) = self.identifier {
            let c_path = std::ffi::CString::new(id.as_str()).map_err(|e| e.to_string())?;
            api.open_path(&c_path)
                .map_err(|e| format!("Failed to open WeDo at {}: {}", id, e))?
        } else {
            let dev_info = api
                .device_list()
                .find(|d| d.vendor_id() == WEDO_VENDOR_ID && d.product_id() == WEDO_PRODUCT_ID)
                .ok_or("No WeDo device found")?;
            api.open_path(dev_info.path())
                .map_err(|e| format!("Failed to open WeDo: {}", e))?
        };

        device
            .set_blocking_mode(false)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Mutex::new(WeDoShared::new()));

        let slot = WeDoSlot {
            device,
            rx,
            shared: shared.clone(),
            output_bits: 0,
            motor_values: [0, 0],
            alive: true,
        };

        let slot_id = driver::register(Box::new(slot));
        self.tx = Some(tx);
        self.shared = shared;
        self.slot_id = Some(slot_id);
        self.motor_values = [0, 0];
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(id) = self.slot_id.take() {
            driver::deregister(id);
        }
        self.tx = None;
        self.motor_values = [0, 0];
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

    fn start_port(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
    ) -> Result<(), String> {
        let idx = self.normalize_port(port)?;
        self.motor_values[idx] = normalize_power(to_signed_power(direction, power));
        self.send_motor_state()
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let idx = self.normalize_port(port)?;
        self.motor_values[idx] = 0;
        self.send_motor_state()
    }

    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn rotate_port_by_degrees(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _degrees: i32,
    ) -> Result<(), String> {
        Err("WeDo does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _position: i32,
    ) -> Result<(), String> {
        Err("WeDo does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("WeDo does not support position reset".to_string())
    }

    fn rotate_to_home(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
    ) -> Result<(), String> {
        Err("WeDo does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let hub_port = port.to_uppercase();
        let effective_mode = mode.unwrap_or("distance");

        let shared = self.shared.lock().unwrap();
        match effective_mode {
            "distance" => match shared.last_payloads.get(&format!("distance:{}", hub_port)) {
                Some(WeDoSensorPayload::Distance(d)) => {
                    Ok(Some(LogoValue::Number(d.distance as f64)))
                }
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "tilt" => match shared.last_payloads.get(&format!("tilt:{}", hub_port)) {
                Some(WeDoSensorPayload::Tilt(t)) => {
                    Ok(Some(LogoValue::Number(t.tilt as u8 as f64)))
                }
                _ => Ok(Some(LogoValue::Number(0.0))),
            },
            "raw" => {
                if let Some(WeDoSensorPayload::Distance(d)) =
                    shared.last_payloads.get(&format!("distance:{}", hub_port))
                {
                    return Ok(Some(LogoValue::Number(d.raw_value as f64)));
                }
                if let Some(WeDoSensorPayload::Tilt(t)) =
                    shared.last_payloads.get(&format!("tilt:{}", hub_port))
                {
                    return Ok(Some(LogoValue::Number(t.raw_value as f64)));
                }
                Ok(Some(LogoValue::Number(0.0)))
            }
            _ => Err(format!(
                "Unsupported sensor mode \"{}\" for WeDo",
                effective_mode
            )),
        }
    }

    // ── Batch overrides (single HID write for both motors) ──

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        for cmd in commands {
            let idx = self.normalize_port(cmd.port)?;
            self.motor_values[idx] = normalize_power(to_signed_power(cmd.direction, cmd.power));
        }
        self.send_motor_state()
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        for port in ports {
            let idx = self.normalize_port(port)?;
            self.motor_values[idx] = 0;
        }
        self.send_motor_state()
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        self.start_ports(commands)?;
        std::thread::sleep(std::time::Duration::from_millis(tenths as u64 * 100));
        let ports: Vec<&str> = commands.iter().map(|c| c.port).collect();
        self.stop_ports(&ports)
    }
}
