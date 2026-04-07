use crate::constants::*;
use crate::protocol::*;
use hidapi::{HidApi, HidDevice};
use std::collections::HashMap;

/// Pre-flight check for WeDo USB device presence on macOS.
/// hidapi's IOKit backend can SIGTRAP on macOS Sequoia when no HID device
/// is present, so we check via ioreg before touching hidapi.
#[cfg(target_os = "macos")]
pub fn wedo_usb_present() -> bool {
    std::process::Command::new("ioreg")
        .args(["-r", "-c", "IOUSBHostDevice", "-l"])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains(&format!("\"idVendor\" = {}", WEDO_VENDOR_ID))
                && s.contains(&format!("\"idProduct\" = {}", WEDO_PRODUCT_ID))
        })
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn wedo_usb_present() -> bool {
    true // Only macOS needs the pre-flight; other platforms go straight to hidapi
}

#[derive(Debug, Clone)]
pub struct WeDoDeviceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: Option<String>,
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DistanceSensorPayload {
    pub port: String,
    pub raw_value: u8,
    pub distance: u8,
}

#[derive(Debug, Clone)]
pub struct TiltSensorPayload {
    pub port: String,
    pub raw_value: u8,
    pub tilt: TiltEvent,
}

#[derive(Debug, Clone)]
pub enum WeDoSensorPayload {
    Distance(DistanceSensorPayload),
    Tilt(TiltSensorPayload),
}

pub struct WeDo {
    state: WeDoState,
    device: Option<HidDevice>,
    device_info: Option<WeDoDeviceInfo>,
    output_bits: u8,
    motor_values: [i8; 2], // A, B
    last_sensor_payloads: HashMap<String, WeDoSensorPayload>,
    target_path: Option<String>,
    target_id: Option<String>,
}

impl WeDo {
    pub fn new() -> Self {
        WeDo {
            state: WeDoState::NotReady,
            device: None,
            device_info: None,
            output_bits: 0,
            motor_values: [0, 0],
            last_sensor_payloads: HashMap::new(),
            target_path: None,
            target_id: None,
        }
    }

    pub fn with_path(path: &str) -> Self {
        let mut w = Self::new();
        w.target_path = Some(path.to_string());
        w
    }

    pub fn with_id(id: &str) -> Self {
        let mut w = Self::new();
        w.target_id = Some(id.to_string());
        w
    }

    pub fn state(&self) -> WeDoState {
        self.state
    }

    pub fn device_info(&self) -> Option<&WeDoDeviceInfo> {
        self.device_info.as_ref()
    }

    pub fn is_connected(&self) -> bool {
        self.state == WeDoState::Ready && self.device.is_some()
    }

    /// Discover all connected WeDo hubs.
    pub fn discover() -> Result<Vec<WeDoDeviceInfo>, String> {
        if !wedo_usb_present() {
            return Ok(Vec::new());
        }
        let api = HidApi::new().map_err(|e| format!("Failed to init HID: {}", e))?;
        let devices = api
            .device_list()
            .filter(|d| d.vendor_id() == WEDO_VENDOR_ID && d.product_id() == WEDO_PRODUCT_ID)
            .filter_map(|d| {
                d.path().to_str().ok().map(|p| WeDoDeviceInfo {
                    path: p.to_string(),
                    vendor_id: d.vendor_id(),
                    product_id: d.product_id(),
                    product: d.product_string().map(|s| s.to_string()),
                    manufacturer: d.manufacturer_string().map(|s| s.to_string()),
                    serial_number: d.serial_number().map(|s| s.to_string()),
                })
            })
            .collect();
        Ok(devices)
    }

    /// Connect to a WeDo hub.
    pub fn connect(&mut self) -> Result<(), String> {
        if !wedo_usb_present() {
            return Err("No WeDo device found".to_string());
        }
        let api = HidApi::new().map_err(|e| format!("Failed to init HID: {}", e))?;

        let device = if let Some(ref path) = self.target_path {
            let c_path = std::ffi::CString::new(path.as_str()).map_err(|e| e.to_string())?;
            api.open_path(&c_path)
                .map_err(|e| format!("Failed to open WeDo at {}: {}", path, e))?
        } else {
            // Find first matching device
            let dev_info = api
                .device_list()
                .find(|d| d.vendor_id() == WEDO_VENDOR_ID && d.product_id() == WEDO_PRODUCT_ID)
                .ok_or_else(|| "No WeDo device found".to_string())?;
            let path = dev_info.path();
            api.open_path(path)
                .map_err(|e| format!("Failed to open WeDo: {}", e))?
        };

        // Set non-blocking for sensor reads
        device
            .set_blocking_mode(false)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;

        self.device = Some(device);
        self.state = WeDoState::Ready;
        self.motor_values = [0, 0];
        self.last_sensor_payloads.clear();

        Ok(())
    }

    /// Disconnect from the WeDo hub.
    pub fn disconnect(&mut self) {
        self.device = None;
        self.state = WeDoState::NotReady;
    }

    /// Set motor power for a port.
    pub fn set_power(&mut self, port: &str, power: i32) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("WeDo not connected")?;
        let port_idx = self.normalize_port(port)?;
        let raw_power = normalize_power(power);
        self.motor_values[port_idx] = raw_power;

        let cmd =
            encode_motor_command(self.output_bits, self.motor_values[0], self.motor_values[1]);
        device
            .write(&cmd)
            .map_err(|e| format!("Failed to write: {}", e))?;
        Ok(())
    }

    /// Set motor power for multiple ports in a single HID write.
    pub fn set_powers(&mut self, ports: &[(&str, i32)]) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("WeDo not connected")?;
        for (port, power) in ports {
            let port_idx = self.normalize_port(port)?;
            self.motor_values[port_idx] = normalize_power(*power);
        }
        let cmd =
            encode_motor_command(self.output_bits, self.motor_values[0], self.motor_values[1]);
        device
            .write(&cmd)
            .map_err(|e| format!("Failed to write: {}", e))?;
        Ok(())
    }

    /// Poll for sensor data and update the cache.
    /// Call this periodically to keep sensor data fresh.
    pub fn poll_sensors(&mut self) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("WeDo not connected")?;
        let mut buf = [0u8; 8];
        match device.read(&mut buf) {
            Ok(n) if n >= SENSOR_MESSAGE_LENGTH => {
                if let Some(notification) = decode_sensor_notification(&buf) {
                    self.process_sensor_notification(&notification);
                }
            }
            Ok(_) => {} // partial or no data
            Err(e) => return Err(format!("Failed to read: {}", e)),
        }
        Ok(())
    }

    /// Read the last cached sensor payload for a port/event.
    pub fn read(&self, port: &str, event: &str) -> Option<WeDoSensorPayload> {
        let port_upper = port.to_uppercase();
        let key = format!("{}:{}", event, port_upper);
        self.last_sensor_payloads.get(&key).cloned()
    }

    fn process_sensor_notification(&mut self, notification: &SensorNotification) {
        for sample in &notification.samples {
            match sample.sensor_type {
                SensorType::Distance => {
                    let distance = get_distance(sample.raw_value);
                    let payload = WeDoSensorPayload::Distance(DistanceSensorPayload {
                        port: sample.port.clone(),
                        raw_value: sample.raw_value,
                        distance,
                    });
                    let key = format!("distance:{}", sample.port);
                    self.last_sensor_payloads.insert(key, payload);
                }
                SensorType::Tilt => {
                    let tilt = get_tilt_event(sample.raw_value);
                    let payload = WeDoSensorPayload::Tilt(TiltSensorPayload {
                        port: sample.port.clone(),
                        raw_value: sample.raw_value,
                        tilt,
                    });
                    let key = format!("tilt:{}", sample.port);
                    self.last_sensor_payloads.insert(key, payload);
                }
                SensorType::Unknown => {}
            }
        }
    }

    fn normalize_port(&self, port: &str) -> Result<usize, String> {
        match port.to_uppercase().as_str() {
            "A" => Ok(0),
            "B" => Ok(1),
            _ => Err(format!("Unknown WeDo port '{}'", port)),
        }
    }
}

#[cfg(test)]
#[path = "tests/wedo.rs"]
mod tests;
