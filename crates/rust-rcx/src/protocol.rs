use crate::constants::*;

/// Frame an RCX message: header + each byte followed by its complement + checksum.
pub fn frame_message(payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(3 + payload.len() * 2 + 2);
    msg.extend_from_slice(&HEADER);
    let mut checksum: u8 = 0;
    for &b in payload {
        msg.push(b);
        msg.push(!b);
        checksum = checksum.wrapping_add(b);
    }
    msg.push(checksum);
    msg.push(!checksum);
    msg
}

/// Parse an RCX reply, stripping header, complements, and checksum.
/// Returns the payload bytes if valid, None if malformed.
pub fn parse_reply(data: &[u8]) -> Option<Vec<u8>> {
    // Find header (skip any leading 55/FF/00 bytes)
    let start = find_header(data)?;
    let body = &data[start + 3..];

    if body.len() < 2 {
        return None; // Need at least opcode + complement
    }

    // Extract all valid pairs (byte, complement)
    let mut all_bytes = Vec::new();
    let mut i = 0;
    while i + 1 < body.len() {
        let b = body[i];
        let comp = body[i + 1];
        if b != !comp {
            break;
        }
        all_bytes.push(b);
        i += 2;
    }

    // Need at least opcode + checksum (2 bytes minimum)
    if all_bytes.len() < 2 {
        return None;
    }

    // Last byte is checksum, rest is payload
    let checksum = all_bytes.pop().unwrap();
    let expected: u8 = all_bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    if checksum != expected {
        return None;
    }

    Some(all_bytes)
}

fn find_header(data: &[u8]) -> Option<usize> {
    if data.len() < 3 {
        return None;
    }
    for i in 0..data.len() - 2 {
        if data[i] == 0x55 && data[i + 1] == 0xFF && data[i + 2] == 0x00 {
            return Some(i);
        }
    }
    None
}

// ── Motor commands ──────────────────────────────

/// Set motor direction.
/// `motors`: bitmask (MOTOR_A | MOTOR_B | MOTOR_C)
/// `direction`: DIR_FORWARD, DIR_REVERSE, or DIR_FLIP
pub fn cmd_set_direction(motors: u8, direction: u8) -> Vec<u8> {
    let code = motors | direction;
    frame_message(&[OP_SET_MOTOR_DIRECTION, code])
}

/// Set motor power.
/// `motors`: bitmask, `power`: 0-7
pub fn cmd_set_power(motors: u8, power: u8) -> Vec<u8> {
    let clamped = power.min(7);
    frame_message(&[OP_SET_MOTOR_POWER, motors, 2, clamped]) // source=2 (immediate)
}

/// Turn motors on, off, or float.
/// `motors`: bitmask, `state`: MOTOR_ON, MOTOR_OFF, or MOTOR_FLOAT
pub fn cmd_set_motor_state(motors: u8, state: u8) -> Vec<u8> {
    let code = motors | state;
    frame_message(&[OP_SET_MOTOR_ON_OFF, code])
}

// ── Sensor commands ─────────────────────────────

/// Set sensor type (Raw, Touch, Temperature, Light, Rotation).
pub fn cmd_set_sensor_type(sensor: u8, sensor_type: u8) -> Vec<u8> {
    frame_message(&[OP_SET_SENSOR_TYPE, sensor, sensor_type])
}

/// Set sensor mode (Raw, Boolean, Percent, Celsius, etc.) with slope.
pub fn cmd_set_sensor_mode(sensor: u8, mode: u8) -> Vec<u8> {
    frame_message(&[OP_SET_SENSOR_MODE, sensor, mode])
}

/// Clear sensor value (reset counter).
pub fn cmd_clear_sensor(sensor: u8) -> Vec<u8> {
    frame_message(&[OP_CLEAR_SENSOR, sensor])
}

/// Read a value from a source. For sensors, use SOURCE_SENSOR_VALUE with sensor index.
pub fn cmd_get_value(source: u8, argument: u8) -> Vec<u8> {
    frame_message(&[OP_GET_VALUE, source, argument])
}

// ── System commands ─────────────────────────────

/// Alive/ping check.
pub fn cmd_alive() -> Vec<u8> {
    frame_message(&[OP_ALIVE])
}

/// Get battery voltage.
pub fn cmd_get_battery() -> Vec<u8> {
    frame_message(&[OP_GET_BATTERY])
}

/// Play a preset sound (0-5).
pub fn cmd_play_sound(sound: u8) -> Vec<u8> {
    frame_message(&[OP_PLAY_SOUND, sound.min(5)])
}

/// Play a tone at the given frequency (Hz) and duration (1/100s).
pub fn cmd_play_tone(frequency: u16, duration: u8) -> Vec<u8> {
    frame_message(&[
        OP_PLAY_TONE,
        (frequency & 0xFF) as u8,
        (frequency >> 8) as u8,
        duration,
    ])
}

// ── Firmware commands ───────────────────────────

/// Delete firmware. Must be called before uploading new firmware.
pub fn cmd_delete_firmware() -> Vec<u8> {
    let mut payload = vec![OP_DELETE_FIRMWARE];
    payload.extend_from_slice(&FIRMWARE_DELETE_KEY);
    frame_message(&payload)
}

/// Start firmware download. `address` is typically 0x8000, `checksum` is the
/// sum of all firmware image bytes.
pub fn cmd_start_firmware_download(address: u16, checksum: u16) -> Vec<u8> {
    frame_message(&[
        OP_START_FIRMWARE_DOWNLOAD,
        (address & 0xFF) as u8,
        (address >> 8) as u8,
        (checksum & 0xFF) as u8,
        (checksum >> 8) as u8,
        0x00,
    ])
}

/// Transfer a block of firmware data. `index` is 1-based; use 0 for the final block.
/// `data` must be at most FIRMWARE_BLOCK_SIZE bytes.
pub fn cmd_transfer_data(index: u16, data: &[u8]) -> Vec<u8> {
    let mut payload = vec![
        OP_TRANSFER_DATA,
        (index & 0xFF) as u8,
        (index >> 8) as u8,
        (data.len() as u16 & 0xFF) as u8,
        ((data.len() as u16) >> 8) as u8,
    ];
    payload.extend_from_slice(data);
    // Block checksum: sum of data bytes
    let block_checksum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    payload.push(block_checksum);
    frame_message(&payload)
}

/// Unlock firmware after upload. The RCX verifies its ROM checksum before replying.
pub fn cmd_unlock_firmware() -> Vec<u8> {
    let mut payload = vec![OP_UNLOCK_FIRMWARE];
    payload.extend_from_slice(&FIRMWARE_UNLOCK_KEY);
    frame_message(&payload)
}

// ── Reply parsing helpers ───────────────────────

/// Extract a reply opcode from parsed payload.
pub fn reply_opcode(payload: &[u8]) -> Option<u8> {
    payload.first().copied()
}

/// Extract a 16-bit value from a reply (bytes 1-2, little-endian).
pub fn reply_value(payload: &[u8]) -> Option<i16> {
    if payload.len() >= 3 {
        Some(i16::from_le_bytes([payload[1], payload[2]]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
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
}
