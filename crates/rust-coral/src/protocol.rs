use crate::constants::*;

// ── Message type IDs ────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    InfoRequest = 0,
    InfoResponse = 1,
    DeviceNotificationRequest = 40,
    DeviceNotificationResponse = 41,
    DeviceNotification = 60,
    MotorResetRelativePositionCommand = 120,
    MotorRunCommand = 122,
    MotorRunForDegreesCommand = 124,
    MotorRunForTimeCommand = 126,
    MotorRunToAbsolutePositionCommand = 128,
    MotorRunToRelativePositionCommand = 130,
    MotorSetDutyCycleCommand = 132,
    MotorStopCommand = 138,
    MotorSetSpeedCommand = 140,
    MotorSetEndStateCommand = 142,
    MotorSetAccelerationCommand = 144,
}

// ── Sensor payloads ─────────────────────────────

#[derive(Debug, Clone)]
pub struct MotorNotificationPayload {
    pub motor_bit_mask: u8,
    pub state: MotorState,
    pub absolute_position: u16,
    pub power: i16,
    pub speed: i8,
    pub position: i32,
}

#[derive(Debug, Clone)]
pub struct ColorSensorPayload {
    pub color: i8,
    pub reflection: u8,
    pub raw_red: u16,
    pub raw_green: u16,
    pub raw_blue: u16,
    pub hue: u16,
    pub saturation: u8,
    pub value: u8,
}

#[derive(Debug, Clone)]
pub struct JoystickPayload {
    pub left_percent: i8,
    pub right_percent: i8,
    pub left_angle: i16,
    pub right_angle: i16,
}

#[derive(Debug, Clone)]
pub struct ButtonPayload {
    pub pressed: bool,
}

#[derive(Debug, Clone)]
pub struct BatteryPayload {
    pub level: u8,
    pub usb_power_state: u8,
}

#[derive(Debug, Clone)]
pub struct MotionSensorPayload {
    pub orientation: u8,
    pub yaw_face: u8,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub accelerometer_x: i16,
    pub accelerometer_y: i16,
    pub accelerometer_z: i16,
    pub gyroscope_x: i16,
    pub gyroscope_y: i16,
    pub gyroscope_z: i16,
}

#[derive(Debug, Clone)]
pub struct MotionGesturePayload {
    pub gesture: i8,
}

#[derive(Debug, Clone)]
pub struct MotorGesturePayload {
    pub motor_bit_mask: u8,
    pub gesture: i8,
}

#[derive(Debug, Clone)]
pub struct TagPayload {
    pub color: i8,
    pub id: u16,
}

#[derive(Debug, Clone)]
pub enum DeviceSensorPayload {
    Motor(MotorNotificationPayload),
    Color(ColorSensorPayload),
    Joystick(JoystickPayload),
    Button(ButtonPayload),
    Battery(BatteryPayload),
    MotionSensor(MotionSensorPayload),
    MotionGesture(MotionGesturePayload),
    MotorGesture(MotorGesturePayload),
    Tag(TagPayload),
}

impl DeviceSensorPayload {
    pub fn kind(&self) -> &str {
        match self {
            DeviceSensorPayload::Motor(_) => "motor",
            DeviceSensorPayload::Color(_) => "color",
            DeviceSensorPayload::Joystick(_) => "joystick",
            DeviceSensorPayload::Button(_) => "button",
            DeviceSensorPayload::Battery(_) => "battery",
            DeviceSensorPayload::MotionSensor(_) => "motion-sensor",
            DeviceSensorPayload::MotionGesture(_) => "motion-gesture",
            DeviceSensorPayload::MotorGesture(_) => "motor-gesture",
            DeviceSensorPayload::Tag(_) => "tag",
        }
    }

    /// Get a composite cache key for this payload.
    pub fn cache_key(&self) -> String {
        match self {
            DeviceSensorPayload::Motor(m) => format!("motor:{}", m.motor_bit_mask),
            DeviceSensorPayload::MotorGesture(m) => format!("motor-gesture:{}", m.motor_bit_mask),
            _ => self.kind().to_string(),
        }
    }
}

// ── Buffer reader ───────────────────────────────

struct BufferReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BufferReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BufferReader { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn peek_u8(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn read_u8(&mut self) -> u8 {
        let v = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        v
    }

    fn read_i8(&mut self) -> i8 {
        self.read_u8() as i8
    }

    fn read_u16(&mut self) -> u16 {
        let lo = self.read_u8() as u16;
        let hi = self.read_u8() as u16;
        lo | (hi << 8)
    }

    fn read_i16(&mut self) -> i16 {
        self.read_u16() as i16
    }

    fn read_u32(&mut self) -> u32 {
        let b0 = self.read_u8() as u32;
        let b1 = self.read_u8() as u32;
        let b2 = self.read_u8() as u32;
        let b3 = self.read_u8() as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    fn read_i32(&mut self) -> i32 {
        self.read_u32() as i32
    }

    fn read_remaining(&mut self) -> &'a [u8] {
        let rest = &self.data[self.pos..];
        self.pos = self.data.len();
        rest
    }
}

// ── Device notification types ───────────────────

const DEVICE_MSG_INFO_HUB: u8 = 0;
const DEVICE_MSG_IMU_HUB: u8 = 1;
const DEVICE_MSG_TAG_HUB: u8 = 3;
const DEVICE_MSG_BUTTON: u8 = 4;
const DEVICE_MSG_MOTOR: u8 = 10;
const DEVICE_MSG_COLOR: u8 = 12;
const DEVICE_MSG_JOYSTICK: u8 = 15;
const DEVICE_MSG_IMU_GESTURE: u8 = 16;
const DEVICE_MSG_MOTOR_GESTURE: u8 = 17;

const DEVICE_MSG_TYPES: &[u8] = &[
    DEVICE_MSG_INFO_HUB,
    DEVICE_MSG_IMU_HUB,
    DEVICE_MSG_TAG_HUB,
    DEVICE_MSG_BUTTON,
    DEVICE_MSG_MOTOR,
    DEVICE_MSG_COLOR,
    DEVICE_MSG_JOYSTICK,
    DEVICE_MSG_IMU_GESTURE,
    DEVICE_MSG_MOTOR_GESTURE,
];

fn is_device_msg_type(v: u8) -> bool {
    DEVICE_MSG_TYPES.contains(&v)
}

// ── Encoding ────────────────────────────────────

fn push_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}
fn push_i8(buf: &mut Vec<u8>, v: i8) {
    buf.push(v as u8);
}
fn push_u16(buf: &mut Vec<u8>, v: u16) {
    buf.push(v as u8);
    buf.push((v >> 8) as u8);
}
fn push_i16(buf: &mut Vec<u8>, v: i16) {
    push_u16(buf, v as u16);
}
fn push_u32(buf: &mut Vec<u8>, v: u32) {
    buf.push(v as u8);
    buf.push((v >> 8) as u8);
    buf.push((v >> 16) as u8);
    buf.push((v >> 24) as u8);
}
fn push_i32(buf: &mut Vec<u8>, v: i32) {
    push_u32(buf, v as u32);
}

pub fn encode_info_request() -> Vec<u8> {
    vec![MessageType::InfoRequest as u8]
}

pub fn encode_notification_request(interval_ms: u16) -> Vec<u8> {
    let mut buf = vec![MessageType::DeviceNotificationRequest as u8];
    push_u16(&mut buf, interval_ms);
    buf
}

pub fn encode_motor_set_speed(motor_bits: u8, speed: i8) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorSetSpeedCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_i8(&mut buf, speed);
    buf
}

pub fn encode_motor_run(motor_bits: u8, direction: u8) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorRunCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_u8(&mut buf, direction);
    buf
}

pub fn encode_motor_stop(motor_bits: u8) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorStopCommand as u8];
    push_u8(&mut buf, motor_bits);
    buf
}

pub fn encode_motor_run_for_time(motor_bits: u8, time_ms: u32, direction: u8) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorRunForTimeCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_u32(&mut buf, time_ms);
    push_u8(&mut buf, direction);
    buf
}

pub fn encode_motor_run_for_degrees(motor_bits: u8, degrees: i32, direction: u8) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorRunForDegreesCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_i32(&mut buf, degrees);
    push_u8(&mut buf, direction);
    buf
}

pub fn encode_motor_run_to_absolute_position(
    motor_bits: u8,
    position: u16,
    direction: u8,
) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorRunToAbsolutePositionCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_u16(&mut buf, position);
    push_u8(&mut buf, direction);
    buf
}

pub fn encode_motor_run_to_relative_position(motor_bits: u8, position: i32) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorRunToRelativePositionCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_i32(&mut buf, position);
    buf
}

pub fn encode_motor_reset_relative_position(motor_bits: u8, position: i32) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorResetRelativePositionCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_i32(&mut buf, position);
    buf
}

pub fn encode_motor_set_duty_cycle(motor_bits: u8, duty_cycle: i16) -> Vec<u8> {
    let mut buf = vec![MessageType::MotorSetDutyCycleCommand as u8];
    push_u8(&mut buf, motor_bits);
    push_i16(&mut buf, duty_cycle);
    buf
}

// ── Decoding ────────────────────────────────────

/// Decode device notification data into sensor payloads.
pub fn decode_device_data(data: &[u8]) -> Vec<DeviceSensorPayload> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut reader = BufferReader::new(data);
    let mut events = Vec::new();

    while reader.remaining() > 0 {
        let msg_type = reader.read_u8();
        match msg_type {
            DEVICE_MSG_INFO_HUB => {
                let level = reader.read_u8();
                let usb = reader.read_u8();
                events.push(DeviceSensorPayload::Battery(BatteryPayload {
                    level,
                    usb_power_state: usb,
                }));
                // Optional embedded joystick data
                if reader.remaining() >= 6 {
                    if let Some(next) = reader.peek_u8() {
                        if !is_device_msg_type(next) {
                            events.push(DeviceSensorPayload::Joystick(JoystickPayload {
                                left_percent: reader.read_i8(),
                                right_percent: reader.read_i8(),
                                left_angle: reader.read_i16(),
                                right_angle: reader.read_i16(),
                            }));
                        }
                    }
                }
            }
            DEVICE_MSG_BUTTON => {
                events.push(DeviceSensorPayload::Button(ButtonPayload {
                    pressed: reader.read_u8() == 1,
                }));
            }
            DEVICE_MSG_MOTOR => {
                events.push(DeviceSensorPayload::Motor(MotorNotificationPayload {
                    motor_bit_mask: reader.read_u8(),
                    state: MotorState::from_u8(reader.read_u8()),
                    absolute_position: reader.read_u16(),
                    power: reader.read_i16(),
                    speed: reader.read_i8(),
                    position: reader.read_i32(),
                }));
            }
            DEVICE_MSG_COLOR => {
                events.push(DeviceSensorPayload::Color(ColorSensorPayload {
                    color: reader.read_i8(),
                    reflection: reader.read_u8(),
                    raw_red: reader.read_u16(),
                    raw_green: reader.read_u16(),
                    raw_blue: reader.read_u16(),
                    hue: reader.read_u16(),
                    saturation: reader.read_u8(),
                    value: reader.read_u8(),
                }));
            }
            DEVICE_MSG_JOYSTICK => {
                events.push(DeviceSensorPayload::Joystick(JoystickPayload {
                    left_percent: reader.read_i8(),
                    right_percent: reader.read_i8(),
                    left_angle: reader.read_i16(),
                    right_angle: reader.read_i16(),
                }));
            }
            DEVICE_MSG_IMU_HUB => {
                events.push(DeviceSensorPayload::MotionSensor(MotionSensorPayload {
                    orientation: reader.read_u8(),
                    yaw_face: reader.read_u8(),
                    yaw: reader.read_i16(),
                    pitch: reader.read_i16(),
                    roll: reader.read_i16(),
                    accelerometer_x: reader.read_i16(),
                    accelerometer_y: reader.read_i16(),
                    accelerometer_z: reader.read_i16(),
                    gyroscope_x: reader.read_i16(),
                    gyroscope_y: reader.read_i16(),
                    gyroscope_z: reader.read_i16(),
                }));
            }
            DEVICE_MSG_TAG_HUB => {
                events.push(DeviceSensorPayload::Tag(TagPayload {
                    color: reader.read_i8(),
                    id: reader.read_u16(),
                }));
            }
            DEVICE_MSG_IMU_GESTURE => {
                events.push(DeviceSensorPayload::MotionGesture(MotionGesturePayload {
                    gesture: reader.read_i8(),
                }));
            }
            DEVICE_MSG_MOTOR_GESTURE => {
                events.push(DeviceSensorPayload::MotorGesture(MotorGesturePayload {
                    motor_bit_mask: reader.read_u8(),
                    gesture: reader.read_i8(),
                }));
            }
            _ => {
                // Skip unknown bytes until next known type
                while reader.remaining() > 0 {
                    if let Some(next) = reader.peek_u8() {
                        if is_device_msg_type(next) {
                            break;
                        }
                    }
                    reader.read_u8();
                }
            }
        }
    }

    events
}

/// A decoded incoming message from the device.
#[derive(Debug, Clone)]
pub enum IncomingMessage {
    /// Device notification containing sensor payloads (id 60).
    Notification(Vec<DeviceSensorPayload>),
    /// Motor command result with motor_bit_mask and status.
    MotorResult {
        command_id: u8,
        motor_bit_mask: u8,
        status: u8,
    },
    /// Status-only result (light, sound, movement, IMU commands).
    StatusResult { command_id: u8, status: u8 },
    /// Other message (info response, notification ack, etc.).
    Other { id: u8 },
}

impl IncomingMessage {
    /// Map a result message ID back to the command that produced it.
    /// Motor and status results have ID = command_id + 1.
    pub fn command_id(&self) -> Option<u8> {
        match self {
            IncomingMessage::MotorResult { command_id, .. } => Some(*command_id),
            IncomingMessage::StatusResult { command_id, .. } => Some(*command_id),
            _ => None,
        }
    }
}

// Motor result IDs (command_id + 1 for each motor command)
const MOTOR_RESULT_IDS: &[u8] = &[121, 123, 125, 127, 129, 131, 133, 139, 141, 143, 145];

// Status-only result IDs
const STATUS_ONLY_RESULT_IDS: &[u8] = &[
    111, 113, 115, 151, 153, 155, 157, 159, 161, 169, 171, 173, 175, 177, 191, 193,
];

/// Decode a full incoming BLE message.
pub fn decode_incoming(data: &[u8]) -> Option<IncomingMessage> {
    if data.is_empty() {
        return None;
    }
    let mut reader = BufferReader::new(data);
    let id = reader.read_u8();

    if id == 60 {
        // DeviceNotification
        let _reserved = reader.read_u16();
        let device_data = decode_device_data(reader.read_remaining());
        return Some(IncomingMessage::Notification(device_data));
    }

    if MOTOR_RESULT_IDS.contains(&id) {
        let motor_bit_mask = reader.read_u8();
        let status = reader.read_u8();
        // command_id is result_id - 1
        return Some(IncomingMessage::MotorResult {
            command_id: id - 1,
            motor_bit_mask,
            status,
        });
    }

    if STATUS_ONLY_RESULT_IDS.contains(&id) {
        let status = reader.read_u8();
        return Some(IncomingMessage::StatusResult {
            command_id: id - 1,
            status,
        });
    }

    // Info response, device notification response, etc.
    Some(IncomingMessage::Other { id })
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
