use super::*;

#[test]
fn test_parse_version_firmware() {
    let line = "Firmware version: 1.4.3";
    assert_eq!(
        parse_version(line),
        Some(HatState::Firmware("1.4.3".to_string()))
    );
}

#[test]
fn test_parse_version_bootloader() {
    let line = "BuildHAT bootloader version 1.0";
    assert_eq!(parse_version(line), Some(HatState::Bootloader));
}

#[test]
fn test_parse_device_active() {
    let line = "P0: connected to active ID2e";
    let dev = parse_device_line(line).unwrap();
    assert_eq!(dev.port, 0);
    assert_eq!(dev.type_id, 0x2e); // 46 = Large Motor
    assert!(dev.active);
}

#[test]
fn test_parse_device_passive() {
    let line = "P1: connected to passive ID01";
    let dev = parse_device_line(line).unwrap();
    assert_eq!(dev.port, 1);
    assert_eq!(dev.type_id, 1);
    assert!(!dev.active);
}

#[test]
fn test_parse_device_none() {
    assert!(parse_device_line("P2: no device detected").is_none());
    assert!(parse_device_line("P3: disconnected").is_none());
}

#[test]
fn test_parse_sensor_data() {
    let line = "P0M1: 45 67.5 89";
    let data = parse_sensor_data(line).unwrap();
    assert_eq!(data.port, 0);
    assert_eq!(data.mode, 1);
    assert_eq!(data.values, vec![45.0, 67.5, 89.0]);
}

#[test]
fn test_parse_sensor_data_combined() {
    let line = "P1C0: 10 20 30";
    let data = parse_sensor_data(line).unwrap();
    assert_eq!(data.port, 1);
    assert_eq!(data.mode, 0);
    assert_eq!(data.values, vec![10.0, 20.0, 30.0]);
}

#[test]
fn test_parse_completion() {
    assert_eq!(parse_completion("P0: ramp done"), Some((0, "ramp")));
    assert_eq!(parse_completion("P1: pulse done"), Some((1, "pulse")));
    assert!(parse_completion("P0: some other message").is_none());
}

#[test]
fn test_checksum() {
    // Simple test: checksum of empty data should be 1 (initial value, no bytes)
    assert_eq!(firmware_checksum(&[]), 1);
    // Checksum of single byte
    let c = firmware_checksum(&[0x42]);
    assert_ne!(c, 0); // Just verify it produces something
}

#[test]
fn test_motor_commands() {
    assert_eq!(cmd_motor_set(0, 50), "port 0 ; pwm ; set 0.5\r");
    assert_eq!(cmd_motor_set(1, -100), "port 1 ; pwm ; set -1\r");
    assert_eq!(cmd_motor_coast(1), "port 1 ; coast\r");
    assert_eq!(cmd_motor_off(2), "port 2 ; pwm ; set 0\r");
    assert!(cmd_motor_speed(0, 75).contains("pid"));
    assert!(cmd_motor_speed(0, 75).contains("set 75"));
}

#[test]
fn test_sensor_commands() {
    assert_eq!(cmd_select_mode(0, 1, 100), "port 0 ; select 1 ; selrate 100\r");
    assert_eq!(cmd_deselect(3), "port 3 ; select\r");
}

#[test]
fn test_preset_uses_set_under_selonce() {
    // The firmware's `preset` verb doesn't reset the position counter for
    // a motor in combi mode — matches the RPi Python lib, which uses
    // `selonce 2 ; set V` to briefly target mode 2 (position) and write
    // the value via `set`.
    assert_eq!(cmd_preset(0, 2, 0.0), "port 0 ; selonce 2 ; set 0\r");
    assert_eq!(cmd_preset(3, 2, 42.5), "port 3 ; selonce 2 ; set 42.5\r");
}
