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

    let mut data = vec![60];
    data.extend_from_slice(&0u16.to_le_bytes());
    data.push(10);
    data.push(1);
    data.push(1);
    data.extend_from_slice(&100u16.to_le_bytes());
    data.extend_from_slice(&50i16.to_le_bytes());
    data.push(25i8 as u8);
    data.extend_from_slice(&360i32.to_le_bytes());

    let msg = coral.process_notification(&data);
    assert!(matches!(msg, Some(IncomingMessage::Notification(ref p)) if p.len() == 1));

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

    let mut data = vec![60];
    data.extend_from_slice(&0u16.to_le_bytes());
    data.push(4);
    data.push(1);

    let msg = coral.process_notification(&data);
    assert!(matches!(msg, Some(IncomingMessage::Notification(ref p)) if p.len() == 1));

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
    data.push(1);
    data.push(4);
    data.push(0);
    for _ in 0..11 {
        data.extend_from_slice(&0i16.to_le_bytes());
    }

    coral.process_notification(&data);

    let cached = coral.read("motion");
    assert!(cached.is_some());
    assert!(matches!(cached, Some(DeviceSensorPayload::MotionSensor(_))));
}

#[test]
fn test_motor_per_port() {
    let mut coral = Coral::new();
    coral.on_connected(CoralDeviceKind::DoubleMotor);

    let mut data1 = vec![60, 0, 0];
    data1.push(10);
    data1.push(1);
    data1.push(0);
    data1.extend_from_slice(&0u16.to_le_bytes());
    data1.extend_from_slice(&0i16.to_le_bytes());
    data1.push(0);
    data1.extend_from_slice(&100i32.to_le_bytes());
    coral.process_notification(&data1);

    let mut data2 = vec![60, 0, 0];
    data2.push(10);
    data2.push(2);
    data2.push(0);
    data2.extend_from_slice(&0u16.to_le_bytes());
    data2.extend_from_slice(&0i16.to_le_bytes());
    data2.push(0);
    data2.extend_from_slice(&200i32.to_le_bytes());
    coral.process_notification(&data2);

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

    let data = vec![60, 0, 0, 4, 1];
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
