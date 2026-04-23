use super::*;

#[test]
fn test_set_output_state_byte_layout() {
    // Port B, +75% power, motor on + regulated + brake, speed-regulated,
    // no turn ratio, running to 720°, reply required.
    let cmd = cmd_set_output_state(
        1,
        75,
        MODE_MOTORON | MODE_REGULATED | MODE_BRAKE,
        REG_MOTOR_SPEED,
        0,
        RUN_RUNNING,
        720,
        true,
    );
    // 2-byte header (type + opcode) + 10-byte body (port, power, mode, regulation,
    // turn_ratio, run_state, tacho_limit[4]).
    assert_eq!(cmd.len(), 12);
    assert_eq!(cmd[0], TYPE_DIRECT);
    assert_eq!(cmd[1], OP_SET_OUTPUT_STATE);
    assert_eq!(cmd[2], 1);
    assert_eq!(cmd[3] as i8, 75);
    assert_eq!(cmd[4], MODE_MOTORON | MODE_REGULATED | MODE_BRAKE);
    assert_eq!(cmd[5], REG_MOTOR_SPEED);
    assert_eq!(cmd[6] as i8, 0);
    assert_eq!(cmd[7], RUN_RUNNING);
    assert_eq!(u32::from_le_bytes([cmd[8], cmd[9], cmd[10], cmd[11]]), 720);
}

#[test]
fn test_set_output_state_no_reply_flag() {
    let cmd = cmd_set_output_state(0, -50, MODE_MOTORON, REG_IDLE, 0, RUN_RUNNING, 0, false);
    assert_eq!(cmd[0], TYPE_DIRECT | NO_REPLY_FLAG);
    assert_eq!(cmd[3] as i8, -50);
}

#[test]
fn test_get_output_state_cmd() {
    let cmd = cmd_get_output_state(2);
    assert_eq!(cmd, vec![TYPE_DIRECT, OP_GET_OUTPUT_STATE, 2]);
}

#[test]
fn test_reset_motor_position_relative() {
    assert_eq!(
        cmd_reset_motor_position(1, true),
        vec![TYPE_DIRECT, OP_RESET_MOTOR_POSITION, 1, 1]
    );
    assert_eq!(
        cmd_reset_motor_position(1, false),
        vec![TYPE_DIRECT, OP_RESET_MOTOR_POSITION, 1, 0]
    );
}

#[test]
fn test_set_input_mode_cmd() {
    let cmd = cmd_set_input_mode(0, 0x01, 0x20, true);
    assert_eq!(cmd, vec![TYPE_DIRECT, OP_SET_INPUT_MODE, 0, 0x01, 0x20]);
}

#[test]
fn test_play_tone_440_500() {
    let cmd = cmd_play_tone(440, 500);
    // 440 = 0x01B8, 500 = 0x01F4 — both LE.
    assert_eq!(cmd, vec![TYPE_DIRECT, OP_PLAY_TONE, 0xB8, 0x01, 0xF4, 0x01]);
}

#[test]
fn test_get_firmware_version_cmd() {
    let cmd = cmd_get_firmware_version();
    assert_eq!(cmd, vec![TYPE_SYSTEM, SYS_GET_FIRMWARE_VERSION]);
}

#[test]
fn test_check_reply_success() {
    let reply = vec![TYPE_REPLY, OP_KEEP_ALIVE, 0x00, 0xAA, 0xBB];
    let payload = check_reply(&reply, OP_KEEP_ALIVE).unwrap();
    assert_eq!(payload, &[0xAA, 0xBB]);
}

#[test]
fn test_check_reply_wrong_opcode() {
    let reply = vec![TYPE_REPLY, 0x99, 0x00];
    let err = check_reply(&reply, OP_KEEP_ALIVE).unwrap_err();
    assert!(err.contains("0x99"));
    assert!(err.contains("0x0D"));
}

#[test]
fn test_check_reply_nonzero_status_is_error() {
    let reply = vec![TYPE_REPLY, OP_KEEP_ALIVE, 0x20];
    let err = check_reply(&reply, OP_KEEP_ALIVE).unwrap_err();
    assert!(err.contains("0x20"), "got {:?}", err);
    assert!(err.to_lowercase().contains("pending"));
}

#[test]
fn test_check_reply_unknown_error_code_formatted() {
    let reply = vec![TYPE_REPLY, OP_KEEP_ALIVE, 0x77];
    let err = check_reply(&reply, OP_KEEP_ALIVE).unwrap_err();
    assert!(err.contains("0x77"));
}

#[test]
fn test_check_reply_short_packet() {
    let reply = vec![TYPE_REPLY, OP_KEEP_ALIVE];
    let err = check_reply(&reply, OP_KEEP_ALIVE).unwrap_err();
    assert!(err.contains("too short"));
}

#[test]
fn test_parse_output_state_round_trip() {
    // Hand-craft a 25-byte reply: header (3) + payload (22).
    let mut reply = vec![TYPE_REPLY, OP_GET_OUTPUT_STATE, 0x00];
    reply.push(0);                // port
    reply.push(75);               // power
    reply.push(0x07);             // mode
    reply.push(1);                // regulation
    reply.push(0);                // turn ratio
    reply.push(0x20);             // run state
    reply.extend_from_slice(&720u32.to_le_bytes());
    reply.extend_from_slice(&360i32.to_le_bytes());
    reply.extend_from_slice(&360i32.to_le_bytes());
    reply.extend_from_slice(&1234i32.to_le_bytes());

    let state = parse_output_state(&reply).unwrap();
    assert_eq!(state.port, 0);
    assert_eq!(state.power, 75);
    assert_eq!(state.mode, 0x07);
    assert_eq!(state.regulation, 1);
    assert_eq!(state.run_state, 0x20);
    assert_eq!(state.tacho_limit, 720);
    assert_eq!(state.tacho_count, 360);
    assert_eq!(state.block_tacho_count, 360);
    assert_eq!(state.rotation_count, 1234);
}

#[test]
fn test_parse_input_values_boolean_touch() {
    // Header + 13-byte payload.
    let mut reply = vec![TYPE_REPLY, OP_GET_INPUT_VALUES, 0x00];
    reply.push(0);              // port
    reply.push(1);              // valid
    reply.push(0);              // calibrated
    reply.push(0x01);           // sensor type (SWITCH)
    reply.push(0x20);           // sensor mode (BOOLEAN)
    reply.extend_from_slice(&900u16.to_le_bytes());  // raw_ad
    reply.extend_from_slice(&900u16.to_le_bytes());  // normalized_ad
    reply.extend_from_slice(&1i16.to_le_bytes());    // scaled (pressed)
    reply.extend_from_slice(&1i16.to_le_bytes());    // calibrated

    let v = parse_input_values(&reply).unwrap();
    assert!(v.valid);
    assert_eq!(v.sensor_type, 0x01);
    assert_eq!(v.sensor_mode, 0x20);
    assert_eq!(v.scaled, 1);
}

#[test]
fn test_parse_battery_level() {
    let mut reply = vec![TYPE_REPLY, OP_GET_BATTERY_LEVEL, 0x00];
    reply.extend_from_slice(&8123u16.to_le_bytes());
    assert_eq!(parse_battery_level(&reply).unwrap(), 8123);
}

#[test]
fn test_parse_firmware_version_ordering() {
    // Wire order per BDK: [protocol_minor, protocol_major, fw_minor, fw_major].
    let reply = vec![
        TYPE_REPLY,
        SYS_GET_FIRMWARE_VERSION,
        0x00,
        0x7C, // protocol minor
        0x01, // protocol major
        0x05, // fw minor
        0x01, // fw major
    ];
    let (pmaj, pmin, fmaj, fmin) = parse_firmware_version(&reply).unwrap();
    assert_eq!((pmaj, pmin), (1, 0x7C));
    assert_eq!((fmaj, fmin), (1, 0x05));
}
