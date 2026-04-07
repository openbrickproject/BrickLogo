use super::*;

#[test]
fn test_encode_power_off() {
    let cmd = encode_output_power(0x01, 0);
    assert_eq!(cmd, vec![0x90, 0x01]);
}

#[test]
fn test_encode_power_forward() {
    let cmd = encode_output_power(0x01, 4);
    assert_eq!(cmd.len(), 6);
    assert_eq!(cmd[0], ControlLabCommand::DirectionLeft as u8);
    assert_eq!(cmd[1], 0x01);
    assert_eq!(cmd[2], ControlLabCommand::PowerLevel as u8 | 3);
    assert_eq!(cmd[3], 0x01);
    assert_eq!(cmd[4], ControlLabCommand::PowerOn as u8);
    assert_eq!(cmd[5], 0x01);
}

#[test]
fn test_encode_power_reverse() {
    let cmd = encode_output_power(0x01, -4);
    assert_eq!(cmd[0], ControlLabCommand::DirectionRight as u8);
}

#[test]
fn test_encode_power_clamp() {
    let cmd = encode_output_power(0x01, 10);
    assert_eq!(cmd[2], ControlLabCommand::PowerLevel as u8 | 7);
}

#[test]
fn test_port_mask() {
    assert_eq!(get_output_port_mask("A"), Some(0x01));
    assert_eq!(get_output_port_mask("B"), Some(0x02));
    assert_eq!(get_output_port_mask("H"), Some(0x80));
    assert_eq!(get_output_port_mask("a"), Some(0x01));
    assert_eq!(get_output_port_mask("X"), None);
}

#[test]
fn test_rotation_delta() {
    assert_eq!(extract_rotation_delta(0b00000100), 0);
    assert_eq!(extract_rotation_delta(0b00000101), 1);
    assert_eq!(extract_rotation_delta(0b00000001), -1);
    assert_eq!(extract_rotation_delta(0b00000110), 2);
}

#[test]
fn test_verify_checksum() {
    let mut msg = vec![0u8; 19];
    msg[0] = 0x00;
    msg[18] = 0xff;
    assert!(verify_sensor_message(&msg));
}
