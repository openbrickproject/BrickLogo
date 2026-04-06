use std::collections::HashMap;
use crate::constants::*;
use crate::protocol::*;

#[derive(Debug, Clone)]
pub struct CoralDeviceInfo {
    pub kind: CoralDeviceKind,
    pub firmware_version: (u8, u8, u16),
    pub bootloader_version: (u8, u8, u16),
}

/// Represents a connection to a Coral (LEGO Education Science) device.
///
/// BLE operations require an async runtime (btleplug + tokio).
/// This struct provides the protocol layer — BLE transport is injected.
pub struct Coral {
    connected: bool,
    device_kind: Option<CoralDeviceKind>,
    device_info: Option<CoralDeviceInfo>,
    last_payloads: HashMap<String, DeviceSensorPayload>,
}

impl Coral {
    pub fn new() -> Self {
        Coral {
            connected: false,
            device_kind: None,
            device_info: None,
            last_payloads: HashMap::new(),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn device_kind(&self) -> Option<CoralDeviceKind> {
        self.device_kind
    }

    pub fn device_info(&self) -> Option<&CoralDeviceInfo> {
        self.device_info.as_ref()
    }

    /// Call when the BLE connection is established and device kind is known.
    pub fn on_connected(&mut self, kind: CoralDeviceKind) {
        self.connected = true;
        self.device_kind = Some(kind);
        self.last_payloads.clear();
    }

    /// Call when the BLE connection is lost.
    pub fn on_disconnected(&mut self) {
        self.connected = false;
        self.device_kind = None;
        self.device_info = None;
        self.last_payloads.clear();
    }

    /// Call when info response is received.
    pub fn on_info_response(&mut self, fw_major: u8, fw_minor: u8, fw_build: u16, bl_major: u8, bl_minor: u8, bl_build: u16) {
        self.device_info = Some(CoralDeviceInfo {
            kind: self.device_kind.unwrap_or(CoralDeviceKind::SingleMotor),
            firmware_version: (fw_major, fw_minor, fw_build),
            bootloader_version: (bl_major, bl_minor, bl_build),
        });
    }

    /// Process incoming BLE notification data.
    /// Returns the decoded sensor payloads.
    pub fn process_notification(&mut self, data: &[u8]) -> Vec<DeviceSensorPayload> {
        if let Some((id, payloads)) = decode_incoming(data) {
            if id == 60 { // DeviceNotification
                for payload in &payloads {
                    let key = payload.cache_key();
                    self.last_payloads.insert(key, payload.clone());
                }
                return payloads;
            }
        }
        Vec::new()
    }

    /// Read the last cached payload for an event type.
    /// For motor events, matches by prefix (e.g., "motor" matches "motor:1" and "motor:2").
    pub fn read(&self, event: &str) -> Option<&DeviceSensorPayload> {
        let kind = match event {
            "motion" => "motion-sensor",
            other => other,
        };

        if kind == "motor" || kind == "motor-gesture" {
            for (key, value) in &self.last_payloads {
                if key.starts_with(&format!("{}:", kind)) || key == kind {
                    return Some(value);
                }
            }
            return None;
        }

        self.last_payloads.get(kind)
    }

    /// Read a specific motor's cached payload by bitmask.
    pub fn read_motor(&self, motor_bit_mask: u8) -> Option<&DeviceSensorPayload> {
        let key = format!("motor:{}", motor_bit_mask);
        self.last_payloads.get(&key)
    }

    // ── Command encoding helpers ────────────────

    pub fn cmd_info_request(&self) -> Vec<u8> {
        encode_info_request()
    }

    pub fn cmd_notification_request(&self, interval_ms: u16) -> Vec<u8> {
        encode_notification_request(interval_ms)
    }

    pub fn cmd_set_motor_speed(&self, motor_bits: u8, speed: i8) -> Vec<u8> {
        encode_motor_set_speed(motor_bits, speed)
    }

    pub fn cmd_motor_run(&self, motor_bits: u8, direction: MotorDirection) -> Vec<u8> {
        encode_motor_run(motor_bits, direction as u8)
    }

    pub fn cmd_motor_stop(&self, motor_bits: u8) -> Vec<u8> {
        encode_motor_stop(motor_bits)
    }

    pub fn cmd_motor_run_for_time(&self, motor_bits: u8, time_ms: u32, direction: MotorDirection) -> Vec<u8> {
        encode_motor_run_for_time(motor_bits, time_ms, direction as u8)
    }

    pub fn cmd_motor_run_for_degrees(&self, motor_bits: u8, degrees: i32, direction: MotorDirection) -> Vec<u8> {
        encode_motor_run_for_degrees(motor_bits, degrees, direction as u8)
    }

    pub fn cmd_motor_run_to_absolute_position(&self, motor_bits: u8, position: u16, direction: MotorDirection) -> Vec<u8> {
        encode_motor_run_to_absolute_position(motor_bits, position, direction as u8)
    }

    pub fn cmd_motor_run_to_relative_position(&self, motor_bits: u8, position: i32) -> Vec<u8> {
        encode_motor_run_to_relative_position(motor_bits, position)
    }

    pub fn cmd_motor_reset_relative_position(&self, motor_bits: u8, position: i32) -> Vec<u8> {
        encode_motor_reset_relative_position(motor_bits, position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let coral = Coral::new();
        assert!(!coral.is_connected());
        assert!(coral.device_kind().is_none());
    }

    #[test]
    fn test_connect_disconnect() {
        let mut coral = Coral::new();
        coral.on_connected(CoralDeviceKind::DoubleMotor);
        assert!(coral.is_connected());
        assert_eq!(coral.device_kind(), Some(CoralDeviceKind::DoubleMotor));

        coral.on_disconnected();
        assert!(!coral.is_connected());
        assert!(coral.device_kind().is_none());
    }

    #[test]
    fn test_process_motor_notification() {
        let mut coral = Coral::new();
        coral.on_connected(CoralDeviceKind::DoubleMotor);

        // Build a DeviceNotification (id=60) with a motor payload
        let mut data = vec![60]; // DeviceNotification
        data.extend_from_slice(&0u16.to_le_bytes()); // reserved
        data.push(10); // DEVICE_MSG_MOTOR
        data.push(1);  // motor_bit_mask = Left
        data.push(1);  // state = Running
        data.extend_from_slice(&100u16.to_le_bytes()); // absolute_position
        data.extend_from_slice(&50i16.to_le_bytes());  // power
        data.push(25i8 as u8); // speed
        data.extend_from_slice(&360i32.to_le_bytes()); // position

        let payloads = coral.process_notification(&data);
        assert_eq!(payloads.len(), 1);

        // Check cached
        let cached = coral.read_motor(1);
        assert!(cached.is_some());
        if let Some(DeviceSensorPayload::Motor(m)) = cached {
            assert_eq!(m.motor_bit_mask, 1);
            assert_eq!(m.position, 360);
        }
    }

    #[test]
    fn test_process_button_notification() {
        let mut coral = Coral::new();
        coral.on_connected(CoralDeviceKind::Controller);

        let mut data = vec![60]; // DeviceNotification
        data.extend_from_slice(&0u16.to_le_bytes());
        data.push(4); // DEVICE_MSG_BUTTON
        data.push(1); // pressed

        let payloads = coral.process_notification(&data);
        assert_eq!(payloads.len(), 1);

        let cached = coral.read("button");
        assert!(cached.is_some());
        if let Some(DeviceSensorPayload::Button(b)) = cached {
            assert!(b.pressed);
        }
    }

    #[test]
    fn test_read_empty() {
        let coral = Coral::new();
        assert!(coral.read("motor").is_none());
        assert!(coral.read("color").is_none());
    }

    #[test]
    fn test_read_motion() {
        let mut coral = Coral::new();
        coral.on_connected(CoralDeviceKind::DoubleMotor);

        let mut data = vec![60];
        data.extend_from_slice(&0u16.to_le_bytes());
        data.push(1); // DEVICE_MSG_IMU_HUB
        data.push(4); // orientation
        data.push(0); // yaw_face
        for _ in 0..11 {
            data.extend_from_slice(&0i16.to_le_bytes());
        }

        coral.process_notification(&data);

        // "motion" maps to "motion-sensor" internally
        let cached = coral.read("motion");
        assert!(cached.is_some());
        assert!(matches!(cached, Some(DeviceSensorPayload::MotionSensor(_))));
    }

    #[test]
    fn test_motor_per_port() {
        let mut coral = Coral::new();
        coral.on_connected(CoralDeviceKind::DoubleMotor);

        // Left motor notification
        let mut data1 = vec![60, 0, 0];
        data1.push(10); // motor
        data1.push(1);  // Left
        data1.push(0);  // Ready
        data1.extend_from_slice(&0u16.to_le_bytes());
        data1.extend_from_slice(&0i16.to_le_bytes());
        data1.push(0);
        data1.extend_from_slice(&100i32.to_le_bytes());
        coral.process_notification(&data1);

        // Right motor notification
        let mut data2 = vec![60, 0, 0];
        data2.push(10); // motor
        data2.push(2);  // Right
        data2.push(0);  // Ready
        data2.extend_from_slice(&0u16.to_le_bytes());
        data2.extend_from_slice(&0i16.to_le_bytes());
        data2.push(0);
        data2.extend_from_slice(&200i32.to_le_bytes());
        coral.process_notification(&data2);

        // Read each motor separately
        let left = coral.read_motor(1);
        let right = coral.read_motor(2);
        assert!(left.is_some());
        assert!(right.is_some());
        if let Some(DeviceSensorPayload::Motor(m)) = left {
            assert_eq!(m.position, 100);
        }
        if let Some(DeviceSensorPayload::Motor(m)) = right {
            assert_eq!(m.position, 200);
        }
    }

    #[test]
    fn test_cmd_encoding() {
        let coral = Coral::new();
        let cmd = coral.cmd_motor_run(3, MotorDirection::Clockwise);
        assert_eq!(cmd, vec![122, 3, 0]);

        let cmd = coral.cmd_motor_stop(3);
        assert_eq!(cmd, vec![138, 3]);

        let cmd = coral.cmd_set_motor_speed(1, 50);
        assert_eq!(cmd, vec![140, 1, 50]);
    }

    #[test]
    fn test_disconnect_clears_cache() {
        let mut coral = Coral::new();
        coral.on_connected(CoralDeviceKind::Controller);

        let mut data = vec![60, 0, 0, 4, 1]; // button pressed
        coral.process_notification(&data);
        assert!(coral.read("button").is_some());

        coral.on_disconnected();
        assert!(coral.read("button").is_none());
    }

    #[test]
    fn test_info_response() {
        let mut coral = Coral::new();
        coral.on_connected(CoralDeviceKind::DoubleMotor);
        coral.on_info_response(1, 2, 3, 4, 5, 6);

        let info = coral.device_info().unwrap();
        assert_eq!(info.firmware_version, (1, 2, 3));
        assert_eq!(info.bootloader_version, (4, 5, 6));
    }
}
