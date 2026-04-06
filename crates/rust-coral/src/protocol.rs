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
    DEVICE_MSG_INFO_HUB, DEVICE_MSG_IMU_HUB, DEVICE_MSG_TAG_HUB,
    DEVICE_MSG_BUTTON, DEVICE_MSG_MOTOR, DEVICE_MSG_COLOR,
    DEVICE_MSG_JOYSTICK, DEVICE_MSG_IMU_GESTURE, DEVICE_MSG_MOTOR_GESTURE,
];

fn is_device_msg_type(v: u8) -> bool {
    DEVICE_MSG_TYPES.contains(&v)
}

// ── Encoding ────────────────────────────────────

fn push_u8(buf: &mut Vec<u8>, v: u8) { buf.push(v); }
fn push_i8(buf: &mut Vec<u8>, v: i8) { buf.push(v as u8); }
fn push_u16(buf: &mut Vec<u8>, v: u16) { buf.push(v as u8); buf.push((v >> 8) as u8); }
fn push_i16(buf: &mut Vec<u8>, v: i16) { push_u16(buf, v as u16); }
fn push_u32(buf: &mut Vec<u8>, v: u32) { buf.push(v as u8); buf.push((v >> 8) as u8); buf.push((v >> 16) as u8); buf.push((v >> 24) as u8); }
fn push_i32(buf: &mut Vec<u8>, v: i32) { push_u32(buf, v as u32); }

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

pub fn encode_motor_run_to_absolute_position(motor_bits: u8, position: u16, direction: u8) -> Vec<u8> {
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
    if data.is_empty() { return Vec::new(); }
    let mut reader = BufferReader::new(data);
    let mut events = Vec::new();

    while reader.remaining() > 0 {
        let msg_type = reader.read_u8();
        match msg_type {
            DEVICE_MSG_INFO_HUB => {
                let level = reader.read_u8();
                let usb = reader.read_u8();
                events.push(DeviceSensorPayload::Battery(BatteryPayload {
                    level, usb_power_state: usb,
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
                        if is_device_msg_type(next) { break; }
                    }
                    reader.read_u8();
                }
            }
        }
    }

    events
}

/// Decode a full incoming BLE notification.
/// Returns the message type ID and the decoded device data (if notification).
pub fn decode_incoming(data: &[u8]) -> Option<(u8, Vec<DeviceSensorPayload>)> {
    if data.is_empty() { return None; }
    let mut reader = BufferReader::new(data);
    let id = reader.read_u8();

    if id == 60 { // DeviceNotification
        let _reserved = reader.read_u16();
        let device_data = decode_device_data(reader.read_remaining());
        return Some((id, device_data));
    }

    // Motor status results (have motorBitMask + status)
    let motor_result_ids: &[u8] = &[121, 123, 125, 127, 129, 131, 133, 139, 141, 143, 145];
    if motor_result_ids.contains(&id) {
        let _motor_bit_mask = reader.read_u8();
        let _status = reader.read_u8();
        return Some((id, Vec::new()));
    }

    // Status-only results
    let status_only_ids: &[u8] = &[111, 113, 115, 151, 153, 155, 157, 159, 161, 169, 171, 173, 175, 177, 191, 193];
    if status_only_ids.contains(&id) {
        let _status = reader.read_u8();
        return Some((id, Vec::new()));
    }

    // Info response, device notification response, etc.
    Some((id, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_info_request() {
        assert_eq!(encode_info_request(), vec![0]);
    }

    #[test]
    fn test_encode_notification_request() {
        let msg = encode_notification_request(50);
        assert_eq!(msg[0], 40); // DeviceNotificationRequest
        assert_eq!(msg[1], 50); // low byte
        assert_eq!(msg[2], 0);  // high byte
    }

    #[test]
    fn test_encode_motor_set_speed() {
        let msg = encode_motor_set_speed(1, 50);
        assert_eq!(msg[0], 140); // MotorSetSpeedCommand
        assert_eq!(msg[1], 1);   // Left motor
        assert_eq!(msg[2], 50);  // Speed
    }

    #[test]
    fn test_encode_motor_run() {
        let msg = encode_motor_run(3, 0); // Both, Clockwise
        assert_eq!(msg[0], 122); // MotorRunCommand
        assert_eq!(msg[1], 3);   // Both
        assert_eq!(msg[2], 0);   // Clockwise
    }

    #[test]
    fn test_encode_motor_stop() {
        let msg = encode_motor_stop(3);
        assert_eq!(msg[0], 138); // MotorStopCommand
        assert_eq!(msg[1], 3);   // Both
    }

    #[test]
    fn test_encode_motor_run_for_time() {
        let msg = encode_motor_run_for_time(1, 1000, 0);
        assert_eq!(msg[0], 126); // MotorRunForTimeCommand
        assert_eq!(msg[1], 1);   // Left
        // 1000 as u32 LE = 0xE8, 0x03, 0x00, 0x00
        assert_eq!(msg[2], 0xE8);
        assert_eq!(msg[3], 0x03);
        assert_eq!(msg[4], 0x00);
        assert_eq!(msg[5], 0x00);
        assert_eq!(msg[6], 0);   // Clockwise
    }

    #[test]
    fn test_encode_motor_run_for_degrees() {
        let msg = encode_motor_run_for_degrees(2, 360, 1);
        assert_eq!(msg[0], 124); // MotorRunForDegreesCommand
        assert_eq!(msg[1], 2);   // Right
        // 360 as i32 LE
        assert_eq!(msg[2], 0x68);
        assert_eq!(msg[3], 0x01);
        assert_eq!(msg[6], 1);   // Counterclockwise
    }

    #[test]
    fn test_encode_motor_reset_relative_position() {
        let msg = encode_motor_reset_relative_position(1, 0);
        assert_eq!(msg[0], 120);
        assert_eq!(msg[1], 1);
        assert_eq!(msg[2..6], [0, 0, 0, 0]);
    }

    #[test]
    fn test_decode_motor_notification() {
        // Build a motor notification payload
        let mut data = vec![DEVICE_MSG_MOTOR];
        data.push(1); // motor_bit_mask = Left
        data.push(1); // state = Running
        data.extend_from_slice(&100u16.to_le_bytes()); // absolute_position
        data.extend_from_slice(&50i16.to_le_bytes()); // power
        data.push(25i8 as u8); // speed
        data.extend_from_slice(&360i32.to_le_bytes()); // position

        let events = decode_device_data(&data);
        assert_eq!(events.len(), 1);
        if let DeviceSensorPayload::Motor(m) = &events[0] {
            assert_eq!(m.motor_bit_mask, 1);
            assert_eq!(m.state, MotorState::Running);
            assert_eq!(m.absolute_position, 100);
            assert_eq!(m.power, 50);
            assert_eq!(m.speed, 25);
            assert_eq!(m.position, 360);
        } else {
            panic!("Expected motor payload");
        }
    }

    #[test]
    fn test_decode_color_sensor() {
        let mut data = vec![DEVICE_MSG_COLOR];
        data.push(9u8); // color = Red (as i8)
        data.push(75); // reflection
        data.extend_from_slice(&200u16.to_le_bytes()); // raw_red
        data.extend_from_slice(&100u16.to_le_bytes()); // raw_green
        data.extend_from_slice(&50u16.to_le_bytes());  // raw_blue
        data.extend_from_slice(&30u16.to_le_bytes());  // hue
        data.push(80); // saturation
        data.push(90); // value

        let events = decode_device_data(&data);
        assert_eq!(events.len(), 1);
        if let DeviceSensorPayload::Color(c) = &events[0] {
            assert_eq!(c.color, 9); // Red
            assert_eq!(c.reflection, 75);
            assert_eq!(c.raw_red, 200);
            assert_eq!(c.saturation, 80);
        } else {
            panic!("Expected color payload");
        }
    }

    #[test]
    fn test_decode_button() {
        let data = vec![DEVICE_MSG_BUTTON, 1];
        let events = decode_device_data(&data);
        assert_eq!(events.len(), 1);
        if let DeviceSensorPayload::Button(b) = &events[0] {
            assert!(b.pressed);
        } else {
            panic!("Expected button payload");
        }
    }

    #[test]
    fn test_decode_motion_sensor() {
        let mut data = vec![DEVICE_MSG_IMU_HUB];
        data.push(4); // orientation
        data.push(0); // yaw_face
        // yaw, pitch, roll, accelX, accelY, accelZ, gyroX, gyroY, gyroZ = 9 x i16
        for _ in 0..9 {
            data.extend_from_slice(&0i16.to_le_bytes());
        }

        let events = decode_device_data(&data);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], DeviceSensorPayload::MotionSensor(_)));
    }

    #[test]
    fn test_decode_motion_gesture() {
        let data = vec![DEVICE_MSG_IMU_GESTURE, 3]; // Shake
        let events = decode_device_data(&data);
        assert_eq!(events.len(), 1);
        if let DeviceSensorPayload::MotionGesture(g) = &events[0] {
            assert_eq!(g.gesture, 3);
        } else {
            panic!("Expected motion gesture");
        }
    }

    #[test]
    fn test_decode_motor_gesture() {
        let data = vec![DEVICE_MSG_MOTOR_GESTURE, 1, 2]; // Left, FastClockwise
        let events = decode_device_data(&data);
        assert_eq!(events.len(), 1);
        if let DeviceSensorPayload::MotorGesture(g) = &events[0] {
            assert_eq!(g.motor_bit_mask, 1);
            assert_eq!(g.gesture, 2);
        } else {
            panic!("Expected motor gesture");
        }
    }

    #[test]
    fn test_decode_multiple_payloads() {
        let mut data = Vec::new();
        // Button pressed
        data.push(DEVICE_MSG_BUTTON);
        data.push(1);
        // Motion gesture
        data.push(DEVICE_MSG_IMU_GESTURE);
        data.push(0); // Tapped

        let events = decode_device_data(&data);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], DeviceSensorPayload::Button(_)));
        assert!(matches!(&events[1], DeviceSensorPayload::MotionGesture(_)));
    }

    #[test]
    fn test_cache_key() {
        let motor = DeviceSensorPayload::Motor(MotorNotificationPayload {
            motor_bit_mask: 1, state: MotorState::Ready,
            absolute_position: 0, power: 0, speed: 0, position: 0,
        });
        assert_eq!(motor.cache_key(), "motor:1");

        let button = DeviceSensorPayload::Button(ButtonPayload { pressed: true });
        assert_eq!(button.cache_key(), "button");
    }

    #[test]
    fn test_decode_empty() {
        assert!(decode_device_data(&[]).is_empty());
    }
}
