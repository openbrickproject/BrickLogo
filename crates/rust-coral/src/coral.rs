use crate::constants::*;
use crate::protocol::*;
use std::collections::HashMap;

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
    pub fn on_info_response(
        &mut self,
        fw_major: u8,
        fw_minor: u8,
        fw_build: u16,
        bl_major: u8,
        bl_minor: u8,
        bl_build: u16,
    ) {
        self.device_info = Some(CoralDeviceInfo {
            kind: self.device_kind.unwrap_or(CoralDeviceKind::SingleMotor),
            firmware_version: (fw_major, fw_minor, fw_build),
            bootloader_version: (bl_major, bl_minor, bl_build),
        });
    }

    /// Process incoming BLE data. Caches sensor payloads from notifications.
    /// Returns the parsed message for the caller to inspect.
    pub fn process_notification(&mut self, data: &[u8]) -> Option<IncomingMessage> {
        let msg = decode_incoming(data)?;
        if let IncomingMessage::Notification(ref payloads) = msg {
            for payload in payloads {
                let key = payload.cache_key();
                self.last_payloads.insert(key, payload.clone());
            }
        }
        Some(msg)
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

    pub fn cmd_motor_run_for_time(
        &self,
        motor_bits: u8,
        time_ms: u32,
        direction: MotorDirection,
    ) -> Vec<u8> {
        encode_motor_run_for_time(motor_bits, time_ms, direction as u8)
    }

    pub fn cmd_motor_run_for_degrees(
        &self,
        motor_bits: u8,
        degrees: i32,
        direction: MotorDirection,
    ) -> Vec<u8> {
        encode_motor_run_for_degrees(motor_bits, degrees, direction as u8)
    }

    pub fn cmd_motor_run_to_absolute_position(
        &self,
        motor_bits: u8,
        position: u16,
        direction: MotorDirection,
    ) -> Vec<u8> {
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
#[path = "tests/coral.rs"]
mod tests;
