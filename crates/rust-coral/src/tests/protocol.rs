use super::*;

#[test]
fn test_encode_info_request() {
    assert_eq!(encode_info_request(), vec![0]);
}

#[test]
fn test_encode_notification_request() {
    let msg = encode_notification_request(50);
    assert_eq!(msg[0], 40);
    assert_eq!(msg[1], 50);
    assert_eq!(msg[2], 0);
}

#[test]
fn test_encode_motor_set_speed() {
    let msg = encode_motor_set_speed(1, 50);
    assert_eq!(msg[0], 140);
    assert_eq!(msg[1], 1);
    assert_eq!(msg[2], 50);
}

#[test]
fn test_encode_motor_run() {
    let msg = encode_motor_run(3, 0);
    assert_eq!(msg[0], 122);
    assert_eq!(msg[1], 3);
    assert_eq!(msg[2], 0);
}

#[test]
fn test_encode_motor_stop() {
    let msg = encode_motor_stop(3);
    assert_eq!(msg[0], 138);
    assert_eq!(msg[1], 3);
}

#[test]
fn test_encode_motor_run_for_time() {
    let msg = encode_motor_run_for_time(1, 1000, 0);
    assert_eq!(msg[0], 126);
    assert_eq!(msg[1], 1);
    assert_eq!(msg[2], 0xE8);
    assert_eq!(msg[3], 0x03);
    assert_eq!(msg[4], 0x00);
    assert_eq!(msg[5], 0x00);
    assert_eq!(msg[6], 0);
}

#[test]
fn test_encode_motor_run_for_degrees() {
    let msg = encode_motor_run_for_degrees(2, 360, 1);
    assert_eq!(msg[0], 124);
    assert_eq!(msg[1], 2);
    assert_eq!(msg[2], 0x68);
    assert_eq!(msg[3], 0x01);
    assert_eq!(msg[6], 1);
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
    let mut data = vec![DEVICE_MSG_MOTOR];
    data.push(1);
    data.push(1);
    data.extend_from_slice(&100u16.to_le_bytes());
    data.extend_from_slice(&50i16.to_le_bytes());
    data.push(25i8 as u8);
    data.extend_from_slice(&360i32.to_le_bytes());

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
    data.push(9u8);
    data.push(75);
    data.extend_from_slice(&200u16.to_le_bytes());
    data.extend_from_slice(&100u16.to_le_bytes());
    data.extend_from_slice(&50u16.to_le_bytes());
    data.extend_from_slice(&30u16.to_le_bytes());
    data.push(80);
    data.push(90);

    let events = decode_device_data(&data);
    assert_eq!(events.len(), 1);
    if let DeviceSensorPayload::Color(c) = &events[0] {
        assert_eq!(c.color, 9);
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
    data.push(4);
    data.push(0);
    for _ in 0..9 {
        data.extend_from_slice(&0i16.to_le_bytes());
    }

    let events = decode_device_data(&data);
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], DeviceSensorPayload::MotionSensor(_)));
}

#[test]
fn test_decode_motion_gesture() {
    let data = vec![DEVICE_MSG_IMU_GESTURE, 3];
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
    let data = vec![DEVICE_MSG_MOTOR_GESTURE, 1, 2];
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
    data.push(DEVICE_MSG_BUTTON);
    data.push(1);
    data.push(DEVICE_MSG_IMU_GESTURE);
    data.push(0);

    let events = decode_device_data(&data);
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0], DeviceSensorPayload::Button(_)));
    assert!(matches!(&events[1], DeviceSensorPayload::MotionGesture(_)));
}

#[test]
fn test_cache_key() {
    let motor = DeviceSensorPayload::Motor(MotorNotificationPayload {
        motor_bit_mask: 1,
        state: MotorState::Ready,
        absolute_position: 0,
        power: 0,
        speed: 0,
        position: 0,
    });
    assert_eq!(motor.cache_key(), "motor:1");

    let button = DeviceSensorPayload::Button(ButtonPayload { pressed: true });
    assert_eq!(button.cache_key(), "button");
}

#[test]
fn test_decode_empty() {
    assert!(decode_device_data(&[]).is_empty());
}
