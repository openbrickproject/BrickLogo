//! EV3 Direct Command protocol — wire format and parameter encoding.
//!
//! A command frame, on the wire:
//!
//! ```text
//!   [length:u16 LE] [counter:u16 LE] [type:u8] [header:u16 LE] [body:u8..]
//! ```
//!
//! `length` counts bytes *after* the length field itself. `counter` is a
//! sequence number echoed back in replies. For Direct Commands the 2-byte
//! `header` packs the reply global-var buffer size in the low 10 bits and
//! the local scratch-var buffer size in the top 6 bits.
//!
//! The body is a sequence of bytecode opcodes interleaved with
//! self-describing arguments. Integer constants use a variable-length
//! LC0/LC1/LC2/LC4 encoding; output-variable addresses (where the VM
//! writes reply data) use GV0/GV1/GV2/GV4.
//!
//! References:
//!   - LEGO MINDSTORMS EV3 Firmware Developer Kit, section "Parameter encoding"
//!   - lms2012/source/bytecodes.h (opcode values)

// ── Message type ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    DirectCmdReply = 0x00,
    DirectCmdNoReply = 0x80,
    SystemCmdReply = 0x01,
    SystemCmdNoReply = 0x81,
}

// ── Opcode constants (from lms2012/source/bytecodes.h) ──

pub const OP_INPUT_DEVICE:      u8 = 0x99;
pub const OP_INPUT_READ:        u8 = 0x9A;

pub const OP_OUTPUT_STOP:       u8 = 0xA3;
pub const OP_OUTPUT_POWER:      u8 = 0xA4;
pub const OP_OUTPUT_START:      u8 = 0xA6;
pub const OP_OUTPUT_TEST:       u8 = 0xA9;
pub const OP_OUTPUT_READY:      u8 = 0xAA;
pub const OP_OUTPUT_STEP_POWER: u8 = 0xAC;
pub const OP_OUTPUT_TIME_POWER: u8 = 0xAD;
pub const OP_OUTPUT_CLR_COUNT:  u8 = 0xB2;
pub const OP_OUTPUT_GET_COUNT:  u8 = 0xB3;

/// `opINPUT_DEVICE` subcommand values.
pub const SUBCMD_GET_TYPEMODE: u8 = 5;
pub const SUBCMD_READY_PCT:    u8 = 27;
pub const SUBCMD_READY_RAW:    u8 = 28;
pub const SUBCMD_READY_SI:     u8 = 29;

/// Layer is 0 for a single, non-daisy-chained brick.
pub const LAYER_MASTER: i8 = 0;

// ── Parameter encoding (LC = "local constant") ────

/// Encode `v` as a 1-byte LC0 when it fits (-31..=31). Returns `None` otherwise.
///
/// Bit layout: `0b 00 S VVVVV` — top 2 bits zero mark it as a short constant,
/// bit 5 is the sign, bits 0..=4 are the magnitude.
pub fn lc0_try(v: i32) -> Option<u8> {
    if (-31..=31).contains(&v) {
        let mag = (v.unsigned_abs() as u8) & 0x1F;
        let sign_bit = if v < 0 { 0x20 } else { 0 };
        Some(sign_bit | mag)
    } else {
        None
    }
}

/// Emit `v` into `buf` using the smallest LC encoding that fits.
pub fn pack_lc(buf: &mut Vec<u8>, v: i32) {
    if let Some(b) = lc0_try(v) {
        buf.push(b);
    } else if (-128..=127).contains(&v) {
        buf.push(0x81);
        buf.push(v as i8 as u8);
    } else if (-32768..=32767).contains(&v) {
        buf.push(0x82);
        buf.extend_from_slice(&(v as i16).to_le_bytes());
    } else {
        buf.push(0x83);
        buf.extend_from_slice(&v.to_le_bytes());
    }
}

// ── Global variable addressing (GV = "global variable", i.e. reply buffer) ──

/// Single-byte GV0 for addresses 0..=31. Bit layout: `0b 011 AAAAA`.
pub fn gv0(addr: u8) -> u8 {
    0x60 | (addr & 0x1F)
}

// ── Frame ────────────────────────────────────────

/// Build the 2-byte Direct Command header that declares reply-buffer and
/// scratch-buffer sizes.
///
///   - `global_bytes`: up to 1023 bytes of reply data (low 10 bits).
///   - `local_bytes`: up to 63 bytes of scratch (top 6 bits).
pub fn direct_header(global_bytes: u16, local_bytes: u8) -> u16 {
    (global_bytes & 0x03FF) | ((local_bytes as u16 & 0x3F) << 10)
}

/// A complete, unframed command. Use `encode()` to produce the on-wire bytes.
#[derive(Debug, Clone)]
pub struct Frame {
    pub counter: u16,
    pub message_type: MessageType,
    /// Direct Command header (reply + scratch buffer sizes).
    /// System Commands ignore this field — set to 0.
    pub header: u16,
    pub body: Vec<u8>,
}

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let body_prefix_len = match self.message_type {
            MessageType::DirectCmdReply | MessageType::DirectCmdNoReply => 2, // header bytes
            MessageType::SystemCmdReply | MessageType::SystemCmdNoReply => 0,
        };
        // length field = counter(2) + type(1) + header(0 or 2) + body
        let length = (2 + 1 + body_prefix_len + self.body.len()) as u16;
        let mut out = Vec::with_capacity(2 + length as usize);
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&self.counter.to_le_bytes());
        out.push(self.message_type as u8);
        if matches!(
            self.message_type,
            MessageType::DirectCmdReply | MessageType::DirectCmdNoReply
        ) {
            out.extend_from_slice(&self.header.to_le_bytes());
        }
        out.extend_from_slice(&self.body);
        out
    }
}

// ── High-level command builders ──────────────────
//
// Every builder takes a `counter` and emits a ready-to-send `Frame`. The
// ports_mask uses EV3's 4-bit bitmask for outputs (A=0x01, B=0x02, C=0x04,
// D=0x08). Input (sensor) ports are 0..3.

fn with_body(counter: u16, reply: bool, global_bytes: u16, body: Vec<u8>) -> Frame {
    Frame {
        counter,
        message_type: if reply {
            MessageType::DirectCmdReply
        } else {
            MessageType::DirectCmdNoReply
        },
        header: direct_header(global_bytes, 0),
        body,
    }
}

/// Set motor power (no reply). Motors will not actually run until `start`
/// is sent; this sets the PWM duty cycle the next `start` will use.
pub fn cmd_output_power(counter: u16, ports_mask: u8, power: i8) -> Frame {
    let mut body = Vec::with_capacity(6);
    body.push(OP_OUTPUT_POWER);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, ports_mask as i32);
    pack_lc(&mut body, power as i32);
    with_body(counter, false, 0, body)
}

/// Begin running motors with the previously-set power.
pub fn cmd_output_start(counter: u16, ports_mask: u8) -> Frame {
    let mut body = Vec::with_capacity(4);
    body.push(OP_OUTPUT_START);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, ports_mask as i32);
    with_body(counter, false, 0, body)
}

/// Stop motors. `brake=true` uses electromagnetic braking; `false` coasts.
pub fn cmd_output_stop(counter: u16, ports_mask: u8, brake: bool) -> Frame {
    let mut body = Vec::with_capacity(5);
    body.push(OP_OUTPUT_STOP);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, ports_mask as i32);
    pack_lc(&mut body, if brake { 1 } else { 0 });
    with_body(counter, false, 0, body)
}

/// Run motors for a number of degrees at the given power, with optional ramp.
/// `ramp_up_steps` + `constant_steps` + `ramp_down_steps` = total motion
/// (each in degrees). For a plain "rotate N degrees with no ramp" pass
/// `(0, N, 0)`.
pub fn cmd_output_step_power(
    counter: u16,
    ports_mask: u8,
    power: i8,
    ramp_up_steps: i32,
    constant_steps: i32,
    ramp_down_steps: i32,
    brake: bool,
) -> Frame {
    let mut body = Vec::with_capacity(16);
    body.push(OP_OUTPUT_STEP_POWER);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, ports_mask as i32);
    pack_lc(&mut body, power as i32);
    pack_lc(&mut body, ramp_up_steps);
    pack_lc(&mut body, constant_steps);
    pack_lc(&mut body, ramp_down_steps);
    pack_lc(&mut body, if brake { 1 } else { 0 });
    with_body(counter, false, 0, body)
}

/// Run motors for a duration (milliseconds) at the given power, with ramp.
pub fn cmd_output_time_power(
    counter: u16,
    ports_mask: u8,
    power: i8,
    ramp_up_ms: i32,
    constant_ms: i32,
    ramp_down_ms: i32,
    brake: bool,
) -> Frame {
    let mut body = Vec::with_capacity(16);
    body.push(OP_OUTPUT_TIME_POWER);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, ports_mask as i32);
    pack_lc(&mut body, power as i32);
    pack_lc(&mut body, ramp_up_ms);
    pack_lc(&mut body, constant_ms);
    pack_lc(&mut body, ramp_down_ms);
    pack_lc(&mut body, if brake { 1 } else { 0 });
    with_body(counter, false, 0, body)
}

/// Query whether any of the given ports are still busy with a STEP/TIME op.
/// Reply: 1 byte, non-zero if busy.
pub fn cmd_output_test_busy(counter: u16, ports_mask: u8) -> Frame {
    let mut body = Vec::with_capacity(5);
    body.push(OP_OUTPUT_TEST);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, ports_mask as i32);
    body.push(gv0(0)); // write busy-flag to reply[0]
    with_body(counter, true, 1, body)
}

/// Zero the position counter for the given ports.
pub fn cmd_output_clr_count(counter: u16, ports_mask: u8) -> Frame {
    let mut body = Vec::with_capacity(4);
    body.push(OP_OUTPUT_CLR_COUNT);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, ports_mask as i32);
    with_body(counter, false, 0, body)
}

/// Read the accumulated position counter (int32 degrees) for one port
/// (**not** a mask — the port index 0..=3). Reply: 4 bytes little-endian i32.
pub fn cmd_output_get_count(counter: u16, port_index: u8) -> Frame {
    let mut body = Vec::with_capacity(5);
    body.push(OP_OUTPUT_GET_COUNT);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, port_index as i32);
    body.push(gv0(0)); // i32 → reply[0..4]
    with_body(counter, true, 4, body)
}

/// Read a sensor value as a percent (0..=100). `port` is 0..=3.
pub fn cmd_input_read_pct(counter: u16, port: u8, sensor_type: u8, mode: u8) -> Frame {
    let mut body = Vec::with_capacity(8);
    body.push(OP_INPUT_DEVICE);
    pack_lc(&mut body, SUBCMD_READY_PCT as i32);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, port as i32);
    pack_lc(&mut body, sensor_type as i32);
    pack_lc(&mut body, mode as i32);
    pack_lc(&mut body, 1); // values wanted
    body.push(gv0(0));
    with_body(counter, true, 1, body)
}

/// Read a sensor value in SI units (f32). `port` is 0..=3.
pub fn cmd_input_read_si(counter: u16, port: u8, sensor_type: u8, mode: u8) -> Frame {
    let mut body = Vec::with_capacity(8);
    body.push(OP_INPUT_DEVICE);
    pack_lc(&mut body, SUBCMD_READY_SI as i32);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, port as i32);
    pack_lc(&mut body, sensor_type as i32);
    pack_lc(&mut body, mode as i32);
    pack_lc(&mut body, 1); // values wanted
    body.push(gv0(0));
    with_body(counter, true, 4, body)
}

/// Read the (type, mode) currently configured on a sensor port.
pub fn cmd_input_get_typemode(counter: u16, port: u8) -> Frame {
    let mut body = Vec::with_capacity(6);
    body.push(OP_INPUT_DEVICE);
    pack_lc(&mut body, SUBCMD_GET_TYPEMODE as i32);
    pack_lc(&mut body, LAYER_MASTER as i32);
    pack_lc(&mut body, port as i32);
    body.push(gv0(0)); // type → reply[0]
    body.push(gv0(1)); // mode → reply[1]
    with_body(counter, true, 2, body)
}

// ── Tests ────────────────────────────────────────

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
