//! LCP (LEGO Communication Protocol) wire format for the NXT.
//!
//! Each packet starts with a type byte (`0x00` direct / `0x01` system /
//! `0x02` reply, optionally ORed with `0x80` to suppress the reply) and a
//! one-byte opcode. The rest of the payload is opcode-specific and never
//! exceeds 64 bytes. On Bluetooth the transport wraps each packet with a
//! 2-byte little-endian length prefix; on USB the bulk endpoint boundaries
//! handle framing. This module deals in the raw LCP bytes only.

// ── Type byte ────────────────────────────────────

pub const TYPE_DIRECT:    u8 = 0x00;
pub const TYPE_SYSTEM:    u8 = 0x01;
pub const TYPE_REPLY:     u8 = 0x02;
/// OR into the type byte to tell the brick not to send a reply.
pub const NO_REPLY_FLAG:  u8 = 0x80;

// ── Direct command opcodes ───────────────────────

pub const OP_START_PROGRAM:         u8 = 0x00;
pub const OP_STOP_PROGRAM:          u8 = 0x01;
pub const OP_PLAY_TONE:             u8 = 0x03;
pub const OP_SET_OUTPUT_STATE:      u8 = 0x04;
pub const OP_SET_INPUT_MODE:        u8 = 0x05;
pub const OP_GET_OUTPUT_STATE:      u8 = 0x06;
pub const OP_GET_INPUT_VALUES:      u8 = 0x07;
pub const OP_RESET_INPUT_SCALED:    u8 = 0x08;
pub const OP_RESET_MOTOR_POSITION:  u8 = 0x0A;
pub const OP_GET_BATTERY_LEVEL:     u8 = 0x0B;
pub const OP_KEEP_ALIVE:            u8 = 0x0D;

// ── System command opcodes ───────────────────────

pub const SYS_GET_FIRMWARE_VERSION: u8 = 0x88;
pub const SYS_GET_DEVICE_INFO:      u8 = 0x9B;

// ── SetOutputState flags / modes ─────────────────

pub const MODE_MOTORON:   u8 = 0x01;
pub const MODE_BRAKE:     u8 = 0x02;
pub const MODE_REGULATED: u8 = 0x04;

pub const REG_IDLE:        u8 = 0x00;
pub const REG_MOTOR_SPEED: u8 = 0x01;
pub const REG_MOTOR_SYNC:  u8 = 0x02;

pub const RUN_IDLE:     u8 = 0x00;
pub const RUN_RAMPUP:   u8 = 0x10;
pub const RUN_RUNNING:  u8 = 0x20;
pub const RUN_RAMPDOWN: u8 = 0x40;

/// Port `0xFF` means "all ports" to `SetOutputState` and `ResetMotorPosition`.
pub const PORT_ALL: u8 = 0xFF;

// ── Typed results ────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputState {
    pub port: u8,
    pub power: i8,
    pub mode: u8,
    pub regulation: u8,
    pub turn_ratio: i8,
    pub run_state: u8,
    pub tacho_limit: u32,
    pub tacho_count: i32,
    pub block_tacho_count: i32,
    pub rotation_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputValues {
    pub port: u8,
    pub valid: bool,
    pub calibrated: bool,
    pub sensor_type: u8,
    pub sensor_mode: u8,
    pub raw_ad: u16,
    pub normalized_ad: u16,
    pub scaled: i16,
    pub calibrated_value: i16,
}

// ── Command builders ─────────────────────────────
//
// Every builder returns raw LCP bytes (no length prefix). The transport
// takes care of framing on Bluetooth. Size budget: each packet fits in a
// single 64-byte USB transfer.

fn type_byte(is_system: bool, reply_required: bool) -> u8 {
    let base = if is_system { TYPE_SYSTEM } else { TYPE_DIRECT };
    if reply_required { base } else { base | NO_REPLY_FLAG }
}

#[allow(clippy::too_many_arguments)]
pub fn cmd_set_output_state(
    port: u8,
    power: i8,
    mode: u8,
    regulation: u8,
    turn_ratio: i8,
    run_state: u8,
    tacho_limit: u32,
    reply_required: bool,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(14);
    buf.push(type_byte(false, reply_required));
    buf.push(OP_SET_OUTPUT_STATE);
    buf.push(port);
    buf.push(power as u8);
    buf.push(mode);
    buf.push(regulation);
    buf.push(turn_ratio as u8);
    buf.push(run_state);
    buf.extend_from_slice(&tacho_limit.to_le_bytes());
    buf
}

pub fn cmd_get_output_state(port: u8) -> Vec<u8> {
    vec![type_byte(false, true), OP_GET_OUTPUT_STATE, port]
}

pub fn cmd_reset_motor_position(port: u8, relative: bool) -> Vec<u8> {
    vec![
        type_byte(false, true),
        OP_RESET_MOTOR_POSITION,
        port,
        if relative { 1 } else { 0 },
    ]
}

pub fn cmd_set_input_mode(
    port: u8,
    sensor_type: u8,
    sensor_mode: u8,
    reply_required: bool,
) -> Vec<u8> {
    vec![
        type_byte(false, reply_required),
        OP_SET_INPUT_MODE,
        port,
        sensor_type,
        sensor_mode,
    ]
}

pub fn cmd_get_input_values(port: u8) -> Vec<u8> {
    vec![type_byte(false, true), OP_GET_INPUT_VALUES, port]
}

pub fn cmd_reset_input_scaled(port: u8) -> Vec<u8> {
    vec![type_byte(false, true), OP_RESET_INPUT_SCALED, port]
}

pub fn cmd_get_battery_level() -> Vec<u8> {
    vec![type_byte(false, true), OP_GET_BATTERY_LEVEL]
}

pub fn cmd_keep_alive() -> Vec<u8> {
    vec![type_byte(false, true), OP_KEEP_ALIVE]
}

pub fn cmd_stop_program() -> Vec<u8> {
    vec![type_byte(false, true), OP_STOP_PROGRAM]
}

pub fn cmd_get_firmware_version() -> Vec<u8> {
    vec![type_byte(true, true), SYS_GET_FIRMWARE_VERSION]
}

pub fn cmd_get_device_info() -> Vec<u8> {
    vec![type_byte(true, true), SYS_GET_DEVICE_INFO]
}

pub fn cmd_play_tone(freq_hz: u16, duration_ms: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(6);
    buf.push(type_byte(false, true));
    buf.push(OP_PLAY_TONE);
    buf.extend_from_slice(&freq_hz.to_le_bytes());
    buf.extend_from_slice(&duration_ms.to_le_bytes());
    buf
}

// ── Reply parsing ────────────────────────────────

/// Validate a reply's framing and return the payload slice (bytes after
/// the 3-byte header: type + echoed opcode + status). Maps nonzero status
/// codes to a readable message.
pub fn check_reply(reply: &[u8], expected_op: u8) -> Result<&[u8], String> {
    if reply.len() < 3 {
        return Err(format!("NXT reply too short ({} bytes)", reply.len()));
    }
    if reply[0] != TYPE_REPLY {
        return Err(format!("NXT reply type 0x{:02X} (want 0x02)", reply[0]));
    }
    if reply[1] != expected_op {
        return Err(format!(
            "NXT reply opcode 0x{:02X} (want 0x{:02X})",
            reply[1], expected_op
        ));
    }
    let status = reply[2];
    if status != 0x00 {
        return Err(status_message(status));
    }
    Ok(&reply[3..])
}

/// Human-readable text for LCP status codes (BDK Appendix 1).
pub fn status_message(code: u8) -> String {
    let text = match code {
        0x20 => "pending communication transaction in progress",
        0x40 => "mailbox queue is empty",
        0x81 => "no more handles",
        0x82 => "no space",
        0x83 => "no more files",
        0x84 => "end of file expected",
        0x85 => "end of file",
        0x86 => "not a linear file",
        0x87 => "file not found",
        0x88 => "handle already closed",
        0x89 => "no linear space",
        0x8A => "undefined error",
        0x8B => "file is busy",
        0x8C => "no write buffers",
        0x8D => "append not possible",
        0x8E => "file is full",
        0x8F => "file exists",
        0x90 => "module not found",
        0x91 => "out of bounds",
        0x92 => "illegal file name",
        0x93 => "illegal handle",
        0xBD => "request failed (e.g. no program running)",
        0xBE => "unknown command opcode",
        0xBF => "insane packet",
        0xC0 => "data contains out-of-range values",
        0xDD => "communication bus error",
        0xDE => "no free memory in comms buffer",
        0xDF => "invalid channel or connection",
        0xE0 => "channel not configured or busy",
        0xEC => "no active program",
        0xED => "illegal size specified",
        0xEE => "illegal mailbox queue id",
        0xEF => "invalid structure field access",
        0xF0 => "bad input or output",
        0xFB => "insufficient memory available",
        0xFF => "bad arguments",
        _ => return format!("NXT error 0x{:02X}", code),
    };
    format!("NXT error 0x{:02X}: {}", code, text)
}

pub fn parse_output_state(reply: &[u8]) -> Result<OutputState, String> {
    let payload = check_reply(reply, OP_GET_OUTPUT_STATE)?;
    if payload.len() < 22 {
        return Err(format!(
            "GetOutputState payload too short ({} bytes, want 22)",
            payload.len()
        ));
    }
    Ok(OutputState {
        port: payload[0],
        power: payload[1] as i8,
        mode: payload[2],
        regulation: payload[3],
        turn_ratio: payload[4] as i8,
        run_state: payload[5],
        tacho_limit: u32::from_le_bytes([payload[6], payload[7], payload[8], payload[9]]),
        tacho_count: i32::from_le_bytes([payload[10], payload[11], payload[12], payload[13]]),
        block_tacho_count: i32::from_le_bytes([
            payload[14], payload[15], payload[16], payload[17],
        ]),
        rotation_count: i32::from_le_bytes([
            payload[18], payload[19], payload[20], payload[21],
        ]),
    })
}

pub fn parse_input_values(reply: &[u8]) -> Result<InputValues, String> {
    let payload = check_reply(reply, OP_GET_INPUT_VALUES)?;
    if payload.len() < 13 {
        return Err(format!(
            "GetInputValues payload too short ({} bytes, want 13)",
            payload.len()
        ));
    }
    Ok(InputValues {
        port: payload[0],
        valid: payload[1] != 0,
        calibrated: payload[2] != 0,
        sensor_type: payload[3],
        sensor_mode: payload[4],
        raw_ad: u16::from_le_bytes([payload[5], payload[6]]),
        normalized_ad: u16::from_le_bytes([payload[7], payload[8]]),
        scaled: i16::from_le_bytes([payload[9], payload[10]]),
        calibrated_value: i16::from_le_bytes([payload[11], payload[12]]),
    })
}

pub fn parse_battery_level(reply: &[u8]) -> Result<u16, String> {
    let payload = check_reply(reply, OP_GET_BATTERY_LEVEL)?;
    if payload.len() < 2 {
        return Err("GetBatteryLevel payload too short".to_string());
    }
    Ok(u16::from_le_bytes([payload[0], payload[1]]))
}

/// Returns (protocol_major, protocol_minor, firmware_major, firmware_minor).
pub fn parse_firmware_version(reply: &[u8]) -> Result<(u8, u8, u8, u8), String> {
    let payload = check_reply(reply, SYS_GET_FIRMWARE_VERSION)?;
    if payload.len() < 4 {
        return Err("GetFirmwareVersion payload too short".to_string());
    }
    // Wire order: [protocol_minor, protocol_major, firmware_minor, firmware_major].
    Ok((payload[1], payload[0], payload[3], payload[2]))
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
