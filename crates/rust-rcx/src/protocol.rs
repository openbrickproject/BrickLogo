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

/// Apply the toggle bit to an opcode. The brick ignores (but re-acks) a
/// command whose opcode byte — toggle included — matches the previous one,
/// so callers alternate `toggle` between logical commands and keep it fixed
/// across retransmissions of the same command.
fn op(opcode: u8, toggle: bool) -> u8 {
    if toggle { opcode | TOGGLE_BIT } else { opcode }
}

// ── Motor commands ──────────────────────────────

/// Set motor direction.
/// `motors`: bitmask (MOTOR_A | MOTOR_B | MOTOR_C)
/// `direction`: DIR_FORWARD, DIR_REVERSE, or DIR_FLIP
pub fn cmd_set_direction(motors: u8, direction: u8, toggle: bool) -> Vec<u8> {
    let code = motors | direction;
    frame_message(&[op(OP_SET_MOTOR_DIRECTION, toggle), code])
}

/// Set motor power.
/// `motors`: bitmask, `power`: 0-7
pub fn cmd_set_power(motors: u8, power: u8, toggle: bool) -> Vec<u8> {
    let clamped = power.min(7);
    frame_message(&[op(OP_SET_MOTOR_POWER, toggle), motors, 2, clamped]) // source=2 (immediate)
}

/// Turn motors on, off, or float.
/// `motors`: bitmask, `state`: MOTOR_ON, MOTOR_OFF, or MOTOR_FLOAT
pub fn cmd_set_motor_state(motors: u8, state: u8, toggle: bool) -> Vec<u8> {
    let code = motors | state;
    frame_message(&[op(OP_SET_MOTOR_ON_OFF, toggle), code])
}

// ── Sensor commands ─────────────────────────────

/// Set sensor type (Raw, Touch, Temperature, Light, Rotation).
pub fn cmd_set_sensor_type(sensor: u8, sensor_type: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_SET_SENSOR_TYPE, toggle), sensor, sensor_type])
}

/// Set sensor mode (Raw, Boolean, Percent, Celsius, etc.) with slope.
pub fn cmd_set_sensor_mode(sensor: u8, mode: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_SET_SENSOR_MODE, toggle), sensor, mode])
}

/// Clear sensor value (reset counter).
pub fn cmd_clear_sensor(sensor: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_CLEAR_SENSOR, toggle), sensor])
}

/// Read a value from a source. For sensors, use SOURCE_SENSOR_VALUE with sensor index.
pub fn cmd_get_value(source: u8, argument: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_GET_VALUE, toggle), source, argument])
}

// ── System commands ─────────────────────────────

/// Alive/ping check.
pub fn cmd_alive(toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_ALIVE, toggle)])
}

/// Get battery voltage.
pub fn cmd_get_battery(toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_GET_BATTERY, toggle)])
}

/// Play a preset sound (0-5).
pub fn cmd_play_sound(sound: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_PLAY_SOUND, toggle), sound.min(5)])
}

/// Play a tone at the given frequency (Hz) and duration (1/100s).
pub fn cmd_play_tone(frequency: u16, duration: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[
        op(OP_PLAY_TONE, toggle),
        (frequency & 0xFF) as u8,
        (frequency >> 8) as u8,
        duration,
    ])
}

// ── Firmware commands ───────────────────────────

/// Delete firmware. Must be called before uploading new firmware.
pub fn cmd_delete_firmware(toggle: bool) -> Vec<u8> {
    let mut payload = vec![op(OP_DELETE_FIRMWARE, toggle)];
    payload.extend_from_slice(&FIRMWARE_DELETE_KEY);
    frame_message(&payload)
}

/// Start firmware download. `address` is typically 0x8000, `checksum` is the
/// sum of all firmware image bytes.
pub fn cmd_start_firmware_download(address: u16, checksum: u16, toggle: bool) -> Vec<u8> {
    frame_message(&[
        op(OP_START_FIRMWARE_DOWNLOAD, toggle),
        (address & 0xFF) as u8,
        (address >> 8) as u8,
        (checksum & 0xFF) as u8,
        (checksum >> 8) as u8,
        0x00,
    ])
}

/// Transfer a block of firmware data. `index` is 1-based; use 0 for the final block.
/// `data` must be at most FIRMWARE_BLOCK_SIZE bytes.
pub fn cmd_transfer_data(index: u16, data: &[u8], toggle: bool) -> Vec<u8> {
    let mut payload = vec![
        op(OP_TRANSFER_DATA, toggle),
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
pub fn cmd_unlock_firmware(toggle: bool) -> Vec<u8> {
    let mut payload = vec![op(OP_UNLOCK_FIRMWARE, toggle)];
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
#[path = "tests/protocol.rs"]
mod tests;
