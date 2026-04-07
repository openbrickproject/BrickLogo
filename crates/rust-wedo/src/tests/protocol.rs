use super::*;

#[test]
fn test_encode_motor_command() {
    let cmd = encode_motor_command(0x00, 100, -50);
    assert_eq!(cmd[0], 0x00);
    assert_eq!(cmd[1], 0x00);
    assert_eq!(cmd[2], 100u8);
    assert_eq!(cmd[3], (-50i8) as u8);
}

#[test]
fn test_encode_high_power() {
    let cmd = encode_motor_command(HUB_CTL_BIT_HIGH_POWER, 127, 0);
    assert_eq!(cmd[1], 0x40);
}

#[test]
fn test_normalize_power() {
    assert_eq!(normalize_power(100), 127);
    assert_eq!(normalize_power(-100), -127);
    assert_eq!(normalize_power(0), 0);
    assert_eq!(normalize_power(50), 64);
}

#[test]
fn test_decode_sensor_notification() {
    let data = [0x00, 0x00, 150, 180, 100, 35, 0x00, 0x00];
    let notif = decode_sensor_notification(&data).unwrap();
    assert_eq!(notif.samples.len(), 2);
    assert_eq!(notif.samples[0].port, "A");
    assert_eq!(notif.samples[0].raw_value, 150);
    assert_eq!(notif.samples[0].sensor_type, SensorType::Distance);
    assert_eq!(notif.samples[1].port, "B");
    assert_eq!(notif.samples[1].raw_value, 100);
    assert_eq!(notif.samples[1].sensor_type, SensorType::Tilt);
}

#[test]
fn test_decode_short_message() {
    assert!(decode_sensor_notification(&[0x00, 0x00, 0x00]).is_none());
}
