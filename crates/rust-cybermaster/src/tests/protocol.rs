use super::*;

#[test]
fn test_frame_uses_cybermaster_header() {
    let framed = frame_message(&[0x10]);
    assert_eq!(&framed[..4], &[0xFE, 0x00, 0x00, 0xFF]);
}

#[test]
fn test_frame_byte_complement_and_checksum() {
    // Payload [0x59, 0x05]: each byte + its complement, then checksum + complement.
    let framed = frame_message(&[0x59, 0x05]);
    assert_eq!(
        framed,
        vec![0xFE, 0x00, 0x00, 0xFF, 0x59, 0xA6, 0x05, 0xFA, 0x5E, 0xA1]
    );
}

#[test]
fn test_play_sound_matches_documented_example_packet() {
    // The documented CyberMaster example packet `FE 00 00 FF 59 A6 05 FA 5E A1`
    // is PlaySound(5) with the toggle bit set (0x51 | 0x08 = 0x59).
    let framed = cmd_play_sound(5, true);
    assert_eq!(
        framed,
        vec![0xFE, 0x00, 0x00, 0xFF, 0x59, 0xA6, 0x05, 0xFA, 0x5E, 0xA1]
    );
}

#[test]
fn test_unlock_message() {
    let framed = cmd_unlock();
    let payload = parse_reply(&framed).expect("unlock should round-trip through framing");
    assert_eq!(payload[0], OP_UNLOCK);
    assert_eq!(&payload[1..], b"Do you byte, when I knock?");
}

#[test]
fn test_motor_state_encoding() {
    // On, forward, combined mask A|B.
    let framed = cmd_set_motor_state(MOTOR_A | MOTOR_B, MOTOR_ON, false);
    let payload = parse_reply(&framed).unwrap();
    assert_eq!(payload, vec![OP_SET_MOTOR_ON_OFF, MOTOR_A | MOTOR_B | MOTOR_ON]);
}

#[test]
fn test_set_power_encoding() {
    // Power op carries the immediate-source marker (2) and clamps to 0-7.
    let framed = cmd_set_power(MOTOR_A, 9, false);
    let payload = parse_reply(&framed).unwrap();
    assert_eq!(payload, vec![OP_SET_MOTOR_POWER, MOTOR_A, 2, 7]);
}

#[test]
fn test_get_value_encoding() {
    let framed = cmd_get_value(SOURCE_INPUT_VALUE, 0, false);
    let payload = parse_reply(&framed).unwrap();
    assert_eq!(payload, vec![OP_GET_VALUE, SOURCE_INPUT_VALUE, 0]);
}

#[test]
fn test_tacho_read_encoding() {
    // Read motor B's tachometer count: source 5 (tacho count), argument 1.
    let framed = cmd_get_value(SOURCE_TACHO_COUNT, 1, false);
    let payload = parse_reply(&framed).unwrap();
    assert_eq!(payload, vec![OP_GET_VALUE, SOURCE_TACHO_COUNT, 1]);
}

#[test]
fn test_clear_tacho_encoding() {
    // Clear both internal motors' tachometers: opcode 0x11 + combined mask.
    let framed = cmd_clear_tacho(MOTOR_A | MOTOR_B, false);
    let payload = parse_reply(&framed).unwrap();
    assert_eq!(payload, vec![OP_CLEAR_TACHO, MOTOR_A | MOTOR_B]);
}

#[test]
fn test_tacho_count_is_signed() {
    // A negative cumulative count (reverse rotation) round-trips as a signed i16.
    let framed = frame_message(&[OP_GET_VALUE, 0xFF, 0xFF]); // 0xFFFF = -1
    let payload = parse_reply(&framed).unwrap();
    assert_eq!(reply_value(&payload), Some(-1));
}

#[test]
fn test_toggle_sets_bit_3() {
    let off = cmd_alive(false);
    let on = cmd_alive(true);
    assert_eq!(off[4], OP_ALIVE);
    assert_eq!(on[4], OP_ALIVE | TOGGLE_BIT);
    // Complement must track the toggled opcode.
    assert_eq!(on[5], !on[4]);
}

#[test]
fn test_parse_reply_extracts_value() {
    // Frame a 3-byte reply [op, lo, hi] and read it back.
    let framed = frame_message(&[OP_GET_VALUE, 0x2C, 0x01]); // 0x012C = 300
    let payload = parse_reply(&framed).unwrap();
    assert_eq!(reply_opcode(&payload), Some(OP_GET_VALUE));
    assert_eq!(reply_value(&payload), Some(300));
}

#[test]
fn test_parse_reply_skips_leading_garbage() {
    let mut data = vec![0x00, 0xAA, 0xFF];
    data.extend(frame_message(&[OP_ALIVE]));
    assert_eq!(parse_reply(&data).unwrap(), vec![OP_ALIVE]);
}

#[test]
fn test_parse_reply_rejects_bad_checksum() {
    let mut framed = frame_message(&[OP_ALIVE]);
    let last = framed.len() - 2;
    framed[last] = framed[last].wrapping_add(1); // corrupt checksum
    assert_eq!(parse_reply(&framed), None);
}
