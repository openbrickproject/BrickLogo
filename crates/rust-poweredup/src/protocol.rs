use crate::constants::*;

// ── LWP3 Message Framing ────────────────────────

/// Frame an LWP3 message with the length prefix.
/// Input: raw payload (without length byte).
/// Output: [length, 0x00, ...payload] where length includes itself.
pub fn frame_message(payload: &[u8]) -> Vec<u8> {
    let len = (payload.len() + 2) as u8; // +2 for length byte and padding
    let mut msg = Vec::with_capacity(len as usize);
    msg.push(len);
    msg.push(0x00); // padding/hub ID
    msg.extend_from_slice(payload);
    msg
}

/// Extract complete messages from a buffer. Returns (messages, remaining_bytes).
pub fn extract_messages(buf: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut messages = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let len = buf[pos] as usize;
        if len == 0 || pos + len > buf.len() {
            break;
        }
        messages.push(buf[pos..pos + len].to_vec());
        pos += len;
    }
    (messages, buf[pos..].to_vec())
}

/// Get the message type from a complete LWP3 message.
pub fn message_type(msg: &[u8]) -> Option<MessageType> {
    if msg.len() >= 3 {
        MessageType::from_u8(msg[2])
    } else {
        None
    }
}

// ── Hub Attached IO Parsing ─────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AttachedIoEvent {
    Attached {
        port_id: u8,
        device_type: DeviceType,
    },
    Detached {
        port_id: u8,
    },
    AttachedVirtual {
        port_id: u8,
        device_type: DeviceType,
        first_port: u8,
        second_port: u8,
    },
}

pub fn parse_attached_io(msg: &[u8]) -> Option<AttachedIoEvent> {
    if msg.len() < 5 {
        return None;
    }
    let port_id = msg[3];
    let event = msg[4];
    match event {
        0x00 => Some(AttachedIoEvent::Detached { port_id }),
        0x01 => {
            if msg.len() < 7 {
                return None;
            }
            let device_type_raw = u16::from_le_bytes([msg[5], msg[6]]);
            Some(AttachedIoEvent::Attached {
                port_id,
                device_type: DeviceType::from_u16(device_type_raw),
            })
        }
        0x02 => {
            if msg.len() < 9 {
                return None;
            }
            let device_type_raw = u16::from_le_bytes([msg[5], msg[6]]);
            Some(AttachedIoEvent::AttachedVirtual {
                port_id,
                device_type: DeviceType::from_u16(device_type_raw),
                first_port: msg[7],
                second_port: msg[8],
            })
        }
        _ => None,
    }
}

// ── Hub Properties Parsing ──────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum HubPropertyValue {
    BatteryVoltage(u8),
    Rssi(i8),
    Button(bool),
    FwVersion(String),
    HwVersion(String),
    Name(String),
}

pub fn parse_hub_property(msg: &[u8]) -> Option<HubPropertyValue> {
    if msg.len() < 5 {
        return None;
    }
    let property = msg[3];
    // msg[4] = operation (0x06 = update)
    match property {
        0x06 => Some(HubPropertyValue::BatteryVoltage(msg[5])),
        0x05 => Some(HubPropertyValue::Rssi(msg[5] as i8)),
        0x02 => Some(HubPropertyValue::Button(msg[5] != 0)),
        0x01 => {
            let name = String::from_utf8_lossy(&msg[5..])
                .trim_end_matches('\0')
                .to_string();
            Some(HubPropertyValue::Name(name))
        }
        0x03 => {
            if msg.len() >= 9 {
                let raw = i32::from_le_bytes([msg[5], msg[6], msg[7], msg[8]]);
                Some(HubPropertyValue::FwVersion(format_version(raw)))
            } else {
                None
            }
        }
        0x04 => {
            if msg.len() >= 9 {
                let raw = i32::from_le_bytes([msg[5], msg[6], msg[7], msg[8]]);
                Some(HubPropertyValue::HwVersion(format_version(raw)))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn format_version(raw: i32) -> String {
    let major = (raw >> 28) & 0x7;
    let minor = (raw >> 24) & 0xF;
    let bugfix = (raw >> 16) & 0xFF;
    let build = raw & 0xFFFF;
    format!("{}.{}.{:02}.{:04}", major, minor, bugfix, build)
}

// ── Sensor Value Parsing ────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SensorValue {
    Int8(i8),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Float(f64),
    Pair(f64, f64),
    Triple(f64, f64, f64),
    Quad(f64, f64, f64, f64),
    Bool(bool),
}

/// Parse a PORT_VALUE_SINGLE message. Returns (port_id, raw_data).
pub fn parse_port_value(msg: &[u8]) -> Option<(u8, &[u8])> {
    if msg.len() < 5 {
        return None;
    }
    let port_id = msg[3];
    Some((port_id, &msg[4..]))
}

/// Parse a PORT_VALUE_SINGLE message for WeDo 2.0. Offset starts at 0.
pub fn parse_wedo2_sensor_value(msg: &[u8]) -> Option<(u8, &[u8])> {
    if msg.len() < 3 {
        return None;
    }
    let port_id = msg[1];
    Some((port_id, &msg[2..]))
}

// ── Port Output Command Feedback ────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct PortFeedback {
    pub port_id: u8,
    pub feedback: u8,
}

impl PortFeedback {
    pub fn is_completed(&self) -> bool {
        self.feedback & FEEDBACK_COMPLETED != 0
    }

    pub fn is_discarded(&self) -> bool {
        self.feedback & FEEDBACK_DISCARDED != 0
    }

    pub fn is_buffer_empty(&self) -> bool {
        self.feedback & FEEDBACK_BUFFER_EMPTY != 0
    }
}

pub fn parse_port_feedback(msg: &[u8]) -> Vec<PortFeedback> {
    let mut result = Vec::new();
    if msg.len() < 5 {
        return result;
    }
    let mut i = 3;
    while i + 1 < msg.len() {
        result.push(PortFeedback {
            port_id: msg[i],
            feedback: msg[i + 1],
        });
        i += 2;
    }
    result
}

// ── Motor Command Encoding ──────────────────────

/// Clamp speed to [-100, 100].
fn map_speed(speed: i8) -> i8 {
    speed.clamp(-100, 100)
}

/// Build acceleration/deceleration profile byte.
fn profile_byte(use_acc: bool, use_dec: bool) -> u8 {
    let mut b: u8 = 0;
    if use_acc {
        b |= 0x01;
    }
    if use_dec {
        b |= 0x02;
    }
    b
}

/// Encode a PORT_OUTPUT_COMMAND for direct power write.
/// Used by BasicMotor.setPower.
pub fn cmd_set_power(port_id: u8, power: i8, interrupt: bool) -> Vec<u8> {
    let sc = if interrupt { 0x11 } else { 0x01 };
    frame_message(&[
        MessageType::PortOutputCommand as u8,
        port_id,
        sc,
        SUBCMD_WRITE_DIRECT_MODE,
        0x00, // mode 0 = power
        map_speed(power) as u8,
    ])
}

/// Encode motor stop (power = 0).
pub fn cmd_motor_stop(port_id: u8, interrupt: bool) -> Vec<u8> {
    cmd_set_power(port_id, 0, interrupt)
}

/// Encode motor brake (power = 127 = hold).
pub fn cmd_motor_brake(port_id: u8, interrupt: bool) -> Vec<u8> {
    let sc = if interrupt { 0x11 } else { 0x01 };
    frame_message(&[
        MessageType::PortOutputCommand as u8,
        port_id,
        sc,
        SUBCMD_WRITE_DIRECT_MODE,
        0x00,
        127_u8, // hold
    ])
}

/// Encode setSpeed (run indefinitely at speed).
pub fn cmd_start_speed(port_id: u8, speed: i8, max_power: u8, interrupt: bool) -> Vec<u8> {
    let sc = if interrupt { 0x11 } else { 0x01 };
    let prof = profile_byte(true, true);
    frame_message(&[
        MessageType::PortOutputCommand as u8,
        port_id,
        sc,
        SUBCMD_START_SPEED,
        map_speed(speed) as u8,
        max_power,
        prof,
    ])
}

/// Encode setSpeed with time limit (milliseconds).
pub fn cmd_start_speed_for_time(
    port_id: u8,
    time_ms: u16,
    speed: i8,
    max_power: u8,
    braking: BrakingStyle,
    interrupt: bool,
) -> Vec<u8> {
    let sc = if interrupt { 0x11 } else { 0x01 };
    let prof = profile_byte(true, true);
    let time_bytes = time_ms.to_le_bytes();
    frame_message(&[
        MessageType::PortOutputCommand as u8,
        port_id,
        sc,
        SUBCMD_START_SPEED_FOR_TIME,
        time_bytes[0],
        time_bytes[1],
        map_speed(speed) as u8,
        max_power,
        braking as u8,
        prof,
    ])
}

/// Encode rotateByDegrees.
pub fn cmd_start_speed_for_degrees(
    port_id: u8,
    degrees: u32,
    speed: i8,
    max_power: u8,
    braking: BrakingStyle,
    interrupt: bool,
) -> Vec<u8> {
    let sc = if interrupt { 0x11 } else { 0x01 };
    let prof = profile_byte(true, true);
    let deg_bytes = degrees.to_le_bytes();
    frame_message(&[
        MessageType::PortOutputCommand as u8,
        port_id,
        sc,
        SUBCMD_START_SPEED_FOR_DEGREES,
        deg_bytes[0],
        deg_bytes[1],
        deg_bytes[2],
        deg_bytes[3],
        map_speed(speed) as u8,
        max_power,
        braking as u8,
        prof,
    ])
}

/// Encode gotoAbsolutePosition.
pub fn cmd_goto_absolute(
    port_id: u8,
    position: i32,
    speed: i8,
    max_power: u8,
    braking: BrakingStyle,
    interrupt: bool,
) -> Vec<u8> {
    let sc = if interrupt { 0x11 } else { 0x01 };
    let prof = profile_byte(true, true);
    let pos_bytes = position.to_le_bytes();
    frame_message(&[
        MessageType::PortOutputCommand as u8,
        port_id,
        sc,
        SUBCMD_GOTO_ABSOLUTE,
        pos_bytes[0],
        pos_bytes[1],
        pos_bytes[2],
        pos_bytes[3],
        map_speed(speed) as u8,
        max_power,
        braking as u8,
        prof,
    ])
}

/// Encode resetZero (preset encoder).
pub fn cmd_reset_zero(port_id: u8, interrupt: bool) -> Vec<u8> {
    let sc = if interrupt { 0x11 } else { 0x01 };
    frame_message(&[
        MessageType::PortOutputCommand as u8,
        port_id,
        sc,
        SUBCMD_WRITE_DIRECT_MODE,
        0x02, // mode 2 = position reset
        0x00,
        0x00,
        0x00,
        0x00,
    ])
}

// ── Sensor Subscription Encoding ────────────────

/// Subscribe to a sensor mode on a port.
pub fn cmd_subscribe(port_id: u8, mode: u8) -> Vec<u8> {
    frame_message(&[
        MessageType::PortInputFormatSetupSingle as u8,
        port_id,
        mode,
        0x01,
        0x00,
        0x00,
        0x00, // delta interval = 1
        0x01, // enable notifications
    ])
}

/// Unsubscribe from a sensor mode on a port.
pub fn cmd_unsubscribe(port_id: u8, mode: u8) -> Vec<u8> {
    frame_message(&[
        MessageType::PortInputFormatSetupSingle as u8,
        port_id,
        mode,
        0x01,
        0x00,
        0x00,
        0x00,
        0x00, // disable notifications
    ])
}

// ── Hub Property Request Encoding ───────────────

pub fn cmd_request_property(property: HubProperty) -> Vec<u8> {
    frame_message(&[
        MessageType::HubProperties as u8,
        property as u8,
        HubPropertyOperation::RequestUpdate as u8,
    ])
}

pub fn cmd_enable_property_updates(property: HubProperty) -> Vec<u8> {
    frame_message(&[
        MessageType::HubProperties as u8,
        property as u8,
        HubPropertyOperation::EnableUpdates as u8,
    ])
}

/// Disconnect command.
pub fn cmd_disconnect() -> Vec<u8> {
    frame_message(&[MessageType::HubActions as u8, 0x02])
}

/// Switch off hub.
pub fn cmd_switch_off() -> Vec<u8> {
    frame_message(&[MessageType::HubActions as u8, 0x01])
}

// ── WeDo 2.0 Command Encoding ──────────────────

/// WeDo 2.0 motor command.
pub fn wedo2_cmd_motor(port_id: u8, power: i8) -> Vec<u8> {
    vec![port_id, 0x01, 0x02, map_speed(power) as u8]
}

/// WeDo 2.0 subscribe to sensor.
pub fn wedo2_cmd_subscribe(port_id: u8, device_type: u8, mode: u8) -> Vec<u8> {
    vec![
        0x01,
        0x02,
        port_id,
        device_type,
        mode,
        0x01,
        0x00,
        0x00,
        0x00,
        0x00,
        0x01,
    ]
}

/// WeDo 2.0 unsubscribe.
pub fn wedo2_cmd_unsubscribe(port_id: u8, device_type: u8, mode: u8) -> Vec<u8> {
    vec![
        0x01,
        0x02,
        port_id,
        device_type,
        mode,
        0x01,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
    ]
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
