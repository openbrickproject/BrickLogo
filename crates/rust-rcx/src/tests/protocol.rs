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
    let msg = cmd_set_motor_state(MOTOR_A, MOTOR_ON, false);
    assert_eq!(&msg[0..3], &HEADER);
    assert_eq!(msg[3], OP_SET_MOTOR_ON_OFF); // 0x21
    assert_eq!(msg[4], !OP_SET_MOTOR_ON_OFF);
    assert_eq!(msg[5], MOTOR_A | MOTOR_ON); // 0x81
    assert_eq!(msg[6], !(MOTOR_A | MOTOR_ON));
}

#[test]
fn test_frame_message_set_power() {
    let msg = cmd_set_power(MOTOR_A, 5, false);
    assert_eq!(msg[3], OP_SET_MOTOR_POWER); // 0x13
    assert_eq!(msg[5], MOTOR_A); // motor bitmask
    assert_eq!(msg[7], 2); // source = immediate
    assert_eq!(msg[9], 5); // power level
}

#[test]
fn test_frame_message_direction() {
    let msg = cmd_set_direction(MOTOR_A | MOTOR_B, DIR_FORWARD, false);
    assert_eq!(msg[3], OP_SET_MOTOR_DIRECTION);
    assert_eq!(msg[5], MOTOR_A | MOTOR_B | DIR_FORWARD); // 0x83
}

#[test]
fn test_frame_message_power_clamp() {
    let msg = cmd_set_power(MOTOR_A, 10, false);
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
    let msg = cmd_set_sensor_type(0, SENSOR_TYPE_TOUCH, false);
    assert_eq!(msg[3], OP_SET_SENSOR_TYPE);
    assert_eq!(msg[5], 0); // sensor 0
    assert_eq!(msg[7], SENSOR_TYPE_TOUCH);
}

#[test]
fn test_sensor_mode_command() {
    let msg = cmd_set_sensor_mode(1, SENSOR_MODE_PERCENT, false);
    assert_eq!(msg[3], OP_SET_SENSOR_MODE);
    assert_eq!(msg[5], 1); // sensor 1
    assert_eq!(msg[7], SENSOR_MODE_PERCENT);
}

#[test]
fn test_get_value_command() {
    let msg = cmd_get_value(SOURCE_SENSOR_VALUE, 0, false);
    assert_eq!(msg[3], OP_GET_VALUE);
    assert_eq!(msg[5], SOURCE_SENSOR_VALUE);
    assert_eq!(msg[7], 0); // sensor 0
}

#[test]
fn test_play_tone() {
    let msg = cmd_play_tone(440, 50, false);
    assert_eq!(msg[3], OP_PLAY_TONE);
    assert_eq!(msg[5], (440 & 0xFF) as u8); // freq low
    assert_eq!(msg[7], (440 >> 8) as u8); // freq high
    assert_eq!(msg[9], 50); // duration
}

// ── Toggle bit ──────────────────────────────────

#[test]
fn test_toggle_true_sets_bit3_with_valid_complement_and_checksum() {
    let msg = cmd_set_motor_state(MOTOR_A, MOTOR_ON, true);
    let toggled_op = OP_SET_MOTOR_ON_OFF | TOGGLE_BIT;
    assert_eq!(msg[3], toggled_op);
    assert_eq!(msg[4], !toggled_op);
    // Checksum covers the toggled opcode byte.
    let code = MOTOR_A | MOTOR_ON;
    assert_eq!(msg[7], toggled_op.wrapping_add(code));
    assert_eq!(msg[8], !toggled_op.wrapping_add(code));
}

#[test]
fn test_toggle_false_leaves_opcode_unchanged() {
    let msg = cmd_set_motor_state(MOTOR_A, MOTOR_ON, false);
    assert_eq!(msg[3], OP_SET_MOTOR_ON_OFF);
    assert_eq!(msg[3] & TOGGLE_BIT, 0);
}

#[test]
fn test_toggle_distinguishes_consecutive_same_opcode_commands() {
    // The point of the toggle: two same-opcode commands in a row must differ
    // in their opcode byte, or the brick drops the second as a retransmission.
    let first = cmd_set_motor_state(MOTOR_A, MOTOR_ON, false);
    let second = cmd_set_motor_state(MOTOR_B, MOTOR_ON, true);
    assert_ne!(first[3], second[3]);
}

#[test]
fn test_toggle_applies_to_every_builder() {
    let framed: Vec<Vec<u8>> = vec![
        cmd_set_direction(MOTOR_A, DIR_FORWARD, true),
        cmd_set_power(MOTOR_A, 5, true),
        cmd_set_motor_state(MOTOR_A, MOTOR_ON, true),
        cmd_set_sensor_type(0, SENSOR_TYPE_TOUCH, true),
        cmd_set_sensor_mode(0, SENSOR_MODE_RAW, true),
        cmd_clear_sensor(0, true),
        cmd_get_value(SOURCE_RAW_SENSOR, 0, true),
        cmd_alive(true),
        cmd_get_battery(true),
        cmd_play_sound(1, true),
        cmd_play_tone(440, 50, true),
        cmd_delete_firmware(true),
        cmd_start_firmware_download(0x8000, 0, true),
        cmd_transfer_data(1, &[0xAA], true),
        cmd_unlock_firmware(true),
    ];
    for msg in framed {
        assert_ne!(msg[3] & TOGGLE_BIT, 0, "toggle bit missing in opcode 0x{:02X}", msg[3]);
        assert_eq!(msg[4], !msg[3], "complement must track the toggled opcode");
    }
}
