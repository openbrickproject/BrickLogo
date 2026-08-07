use crate::constants::*;

/// Frame a CyberMaster message: header + each byte followed by its one's
/// complement + checksum (sum of payload) followed by its complement.
pub fn frame_message(payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(HEADER.len() + payload.len() * 2 + 2);
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

/// Parse a CyberMaster reply, stripping the header, complements, and checksum.
/// Returns the payload bytes if valid, None if malformed. Replies are assumed
/// to use the same framing as commands (unverified against hardware).
pub fn parse_reply(data: &[u8]) -> Option<Vec<u8>> {
    let start = find_header(data)?;
    let body = &data[start + HEADER.len()..];

    if body.len() < 2 {
        return None; // Need at least opcode + complement
    }

    // Extract all valid (byte, complement) pairs.
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

    // Need at least opcode + checksum.
    if all_bytes.len() < 2 {
        return None;
    }

    let checksum = all_bytes.pop().unwrap();
    let expected: u8 = all_bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    if checksum != expected {
        return None;
    }

    Some(all_bytes)
}

fn find_header(data: &[u8]) -> Option<usize> {
    if data.len() < HEADER.len() {
        return None;
    }
    data.windows(HEADER.len()).position(|w| w == HEADER)
}

/// Apply the toggle bit to an opcode. The brick treats a command whose opcode
/// byte — toggle included — matches the previous one as a retransmission, so
/// callers alternate `toggle` between logical commands and keep it fixed across
/// retransmissions of the same command.
fn op(opcode: u8, toggle: bool) -> u8 {
    if toggle { opcode | TOGGLE_BIT } else { opcode }
}

// ── Session ─────────────────────────────────────

/// The CyberMaster unlock handshake, sent once before any other command.
pub fn cmd_unlock() -> Vec<u8> {
    let mut payload = vec![OP_UNLOCK];
    payload.extend_from_slice(UNLOCK_MESSAGE);
    frame_message(&payload)
}

/// Alive/ping check.
pub fn cmd_alive(toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_ALIVE, toggle)])
}

// ── Motor commands ──────────────────────────────

/// Set motor direction.
/// `motors`: bitmask (MOTOR_A | MOTOR_B | MOTOR_C), `direction`: DIR_*.
pub fn cmd_set_direction(motors: u8, direction: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_SET_MOTOR_DIRECTION, toggle), motors | direction])
}

/// Set motor power. `motors`: bitmask, `power`: 0-7.
pub fn cmd_set_power(motors: u8, power: u8, toggle: bool) -> Vec<u8> {
    let clamped = power.min(7);
    frame_message(&[op(OP_SET_MOTOR_POWER, toggle), motors, 2, clamped]) // source=2 (immediate)
}

/// Turn motors on, off, or float. `motors`: bitmask, `state`: MOTOR_*.
pub fn cmd_set_motor_state(motors: u8, state: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_SET_MOTOR_ON_OFF, toggle), motors | state])
}

/// Clear the tachometer counter(s) for the given motor mask.
pub fn cmd_clear_tacho(motors: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_CLEAR_TACHO, toggle), motors])
}

// ── Sensor commands ─────────────────────────────

/// Set a sensor's mode (raw, boolean, …).
pub fn cmd_set_sensor_mode(sensor: u8, mode: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_SET_SENSOR_MODE, toggle), sensor, mode])
}

/// Read a value from a source. For inputs, use SOURCE_INPUT_VALUE with the
/// sensor index as the argument.
pub fn cmd_get_value(source: u8, argument: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_GET_VALUE, toggle), source, argument])
}

// ── Misc ────────────────────────────────────────

/// Play a preset sound (0-5).
pub fn cmd_play_sound(sound: u8, toggle: bool) -> Vec<u8> {
    frame_message(&[op(OP_PLAY_SOUND, toggle), sound.min(5)])
}

// ── Reply parsing helpers ───────────────────────

/// Extract the reply opcode from a parsed payload.
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
