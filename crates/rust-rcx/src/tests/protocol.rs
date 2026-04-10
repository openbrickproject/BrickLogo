use super::*;

#[test]
fn test_frame_message_alive() {
    let msg = frame_message(&[OP_ALIVE]);
    // 55 FF 00 10 EF 10 EF
    assert_eq!(&msg[0..3], &HEADER);
    assert_eq!(msg[3], 0x10); // opcode
    assert_eq!(msg[4], !0x10u8); // complement
    assert_eq!(msg[5], 0x10); // checksum (just the opcode)
    assert_eq!(msg[6], !0x10u8); // checksum complement
}

#[test]
fn test_frame_message_motor_on() {
    let msg = cmd_set_motor_state(MOTOR_A, MOTOR_ON);
    assert_eq!(&msg[0..3], &HEADER);
    assert_eq!(msg[3], OP_SET_MOTOR_ON_OFF); // 0x21
    assert_eq!(msg[4], !OP_SET_MOTOR_ON_OFF);
    assert_eq!(msg[5], MOTOR_A | MOTOR_ON); // 0x81
    assert_eq!(msg[6], !(MOTOR_A | MOTOR_ON));
}

#[test]
fn test_frame_message_set_power() {
    let msg = cmd_set_power(MOTOR_A, 5);
    assert_eq!(msg[3], OP_SET_MOTOR_POWER); // 0x13
    assert_eq!(msg[5], MOTOR_A); // motor bitmask
    assert_eq!(msg[7], 2); // source = immediate
    assert_eq!(msg[9], 5); // power level
}

#[test]
fn test_frame_message_direction() {
    let msg = cmd_set_direction(MOTOR_A | MOTOR_B, DIR_FORWARD);
    assert_eq!(msg[3], OP_SET_MOTOR_DIRECTION);
    assert_eq!(msg[5], MOTOR_A | MOTOR_B | DIR_FORWARD); // 0x83
}

#[test]
fn test_frame_message_power_clamp() {
    let msg = cmd_set_power(MOTOR_A, 10);
    assert_eq!(msg[9], 7); // clamped to max
}

#[test]
fn test_parse_reply_alive() {
    // Alive reply: 55 FF 00 EF 10 EF 10
    let data = vec![0x55, 0xFF, 0x00, 0xEF, !0xEFu8, 0xEF, !0xEFu8];
    let payload = parse_reply(&data);
    assert!(payload.is_some());
    let payload = payload.unwrap();
    assert_eq!(payload[0], 0xEF); // ~OP_ALIVE
}

#[test]
fn test_parse_reply_with_value() {
    // Simulated get_value reply: opcode + 2 byte value
    let reply_op: u8 = !OP_GET_VALUE; // 0xED
    let val_lo: u8 = 0x2A;
    let val_hi: u8 = 0x00;
    let checksum = reply_op.wrapping_add(val_lo).wrapping_add(val_hi);
    let data = vec![
        0x55, 0xFF, 0x00,
        reply_op, !reply_op,
        val_lo, !val_lo,
        val_hi, !val_hi,
        checksum, !checksum,
    ];
    let payload = parse_reply(&data).unwrap();
    assert_eq!(payload.len(), 3);
    assert_eq!(reply_value(&payload), Some(42));
}

#[test]
fn test_parse_reply_invalid() {
    assert!(parse_reply(&[]).is_none());
    assert!(parse_reply(&[0x55, 0xFF]).is_none());
}

#[test]
fn test_sensor_type_command() {
    let msg = cmd_set_sensor_type(0, SENSOR_TYPE_TOUCH);
    assert_eq!(msg[3], OP_SET_SENSOR_TYPE);
    assert_eq!(msg[5], 0); // sensor 0
    assert_eq!(msg[7], SENSOR_TYPE_TOUCH);
}

#[test]
fn test_sensor_mode_command() {
    let msg = cmd_set_sensor_mode(1, SENSOR_MODE_PERCENT);
    assert_eq!(msg[3], OP_SET_SENSOR_MODE);
    assert_eq!(msg[5], 1); // sensor 1
    assert_eq!(msg[7], SENSOR_MODE_PERCENT);
}

#[test]
fn test_get_value_command() {
    let msg = cmd_get_value(SOURCE_SENSOR_VALUE, 0);
    assert_eq!(msg[3], OP_GET_VALUE);
    assert_eq!(msg[5], SOURCE_SENSOR_VALUE);
    assert_eq!(msg[7], 0); // sensor 0
}

#[test]
fn test_play_tone() {
    let msg = cmd_play_tone(440, 50);
    assert_eq!(msg[3], OP_PLAY_TONE);
    assert_eq!(msg[5], (440 & 0xFF) as u8); // freq low
    assert_eq!(msg[7], (440 >> 8) as u8); // freq high
    assert_eq!(msg[9], 50); // duration
}
