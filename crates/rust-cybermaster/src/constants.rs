// LEGO Technic CyberMaster (set 8482, 1998) protocol constants.
//
// The CyberMaster is an RF-controlled programmable brick with fixed firmware
// in ROM. The host PC drives an RF tower (LEGO part 71846) over a serial COM
// port; the tower relays commands wirelessly to the brick. The brick shares
// the RCX direct-command opcode framework, but the on-the-wire framing and a
// few opcodes (unlock, tacho) are CyberMaster-specific.
//
// Protocol facts are drawn from the NQC source (BrickBot/nqc — `RCX_Constants.h`,
// `RCX_Cmd.cpp`, `compiler/rcx1.nqh`) and LEGO's SPIRIT.OCX reference. The RF
// framing (header + line control) is corroborated by the documented example
// packet `FE 00 00 FF 59 A6 05 FA 5E A1` (= PlaySound(5) with the toggle bit
// set), but the radio physical layer is otherwise undocumented — treat the
// transport as needing hardware verification.

// ── Serial / RF tower settings ───────────────────
// 2400 baud, 8 data bits, odd parity, 1 stop bit, with DTR asserted and RTS
// de-asserted (NQC's kCyberMasterMode). The DTR/RTS line control differs from
// the RCX tower.
pub const DEFAULT_BAUD_RATE: u32 = 2400;

pub const COMMAND_TIMEOUT_MS: u64 = 500;
pub const COMMAND_RETRIES: usize = 3;

// ── Protocol framing ─────────────────────────────
// CyberMaster packets are headed by `FE 00 00 FF` (the RCX uses `55 FF 00`).
// The tower inspects the leading `FE` to verify the 2400 bps baud rate before
// enabling the radio. Each payload byte is followed by its one's complement,
// then a checksum (sum of payload bytes) followed by its complement — the same
// encoding as the RCX, validated against the documented example packet above.
pub const HEADER: [u8; 4] = [0xFE, 0x00, 0x00, 0xFF];

// ── Unlock handshake ─────────────────────────────
// Before it accepts commands the CyberMaster requires an unlock: opcode 0xA5
// followed by the literal ASCII "Do you byte, when I knock?" (NQC's
// MakeUnlockCM). Sent once at connect.
pub const OP_UNLOCK: u8 = 0xA5;
pub const UNLOCK_MESSAGE: &[u8] = b"Do you byte, when I knock?";

// ── Motor bitmask ────────────────────────────────
// OUT_A = left internal motor, OUT_B = right internal motor,
// OUT_C = the external motor output (NQC's OUT_X).
pub const MOTOR_A: u8 = 0x01;
pub const MOTOR_B: u8 = 0x02;
pub const MOTOR_C: u8 = 0x04;

// ── Motor direction bits ─────────────────────────
pub const DIR_FORWARD: u8 = 0x80;
pub const DIR_REVERSE: u8 = 0x00;
pub const DIR_FLIP: u8 = 0x40;

// ── Motor on/off bits ────────────────────────────
pub const MOTOR_ON: u8 = 0x80;
pub const MOTOR_OFF: u8 = 0x40;
pub const MOTOR_FLOAT: u8 = 0x00;

// ── Toggle bit ───────────────────────────────────
// Bit 3 of every opcode is the message-numbering toggle: the brick treats a
// command whose opcode byte (toggle included) matches the previous one as a
// retransmission and re-acks without re-executing. NQC masks `data[0] & 0xf7`
// for all targets, the CyberMaster included. Senders flip the bit between
// logical commands and hold it across deliberate retransmissions.
pub const TOGGLE_BIT: u8 = 0x08;

// ── Opcodes (shared RCX direct-command framework) ─
pub const OP_ALIVE: u8 = 0x10;
pub const OP_GET_VALUE: u8 = 0x12;
pub const OP_SET_MOTOR_POWER: u8 = 0x13;
pub const OP_SET_MOTOR_DIRECTION: u8 = 0xE1;
pub const OP_SET_MOTOR_ON_OFF: u8 = 0x21;
pub const OP_SET_SENSOR_MODE: u8 = 0x42;
pub const OP_PLAY_SOUND: u8 = 0x51;
/// CyberMaster `ClearTachoCounter` — clears the tachometers for the given
/// motor mask. Reserved for future rotation support.
pub const OP_CLEAR_TACHO: u8 = 0x11;

// ── Sensor modes ─────────────────────────────────
pub const SENSOR_MODE_RAW: u8 = 0x00;
pub const SENSOR_MODE_BOOLEAN: u8 = 0x20;

// ── Value sources (for get_value) ────────────────
// The CyberMaster reads an input through Input(n) = source 9 (InputValue);
// unlike the RCX it has no separate "raw sensor" source. The tacho sources
// (5 = count, 6 = speed) are reserved for future rotation support.
pub const SOURCE_INPUT_VALUE: u8 = 9;
pub const SOURCE_TACHO_COUNT: u8 = 5;
pub const SOURCE_TACHO_SPEED: u8 = 6;

// ── Touch threshold ──────────────────────────────
// A touch sensor on the passive analog input reads near 1023 when released
// and near 0 when pressed; threshold at the midpoint (matches the RCX
// adapter's convention and sits inside NQC's documented dead-band).
pub const TOUCH_PRESSED_BELOW: i16 = 500;

// ── Port names ───────────────────────────────────
pub const OUTPUT_PORTS: [&str; 3] = ["A", "B", "C"];
pub const INPUT_PORTS: [&str; 3] = ["1", "2", "3"];

/// Map an output port letter to its motor bitmask.
pub fn motor_mask(port: &str) -> Option<u8> {
    match port.to_uppercase().as_str() {
        "A" => Some(MOTOR_A),
        "B" => Some(MOTOR_B),
        // "X" is the LEGO/NQC name for the external motor output (= OUT_C).
        "C" | "X" => Some(MOTOR_C),
        _ => None,
    }
}

/// Map an input port number string to its sensor index (0-2).
pub fn sensor_index(port: &str) -> Option<u8> {
    match port {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        _ => None,
    }
}

/// Map a motor port with an embedded tachometer to its tacho argument index.
/// Only the two internal motors (A, B) have tachometers; the external motor
/// output (C/X) has none. The NQC `TachoCount(OUT_x)` source argument is
/// `mask - 1`, i.e. A → 0, B → 1.
pub fn tacho_index(port: &str) -> Option<u8> {
    match port.to_uppercase().as_str() {
        "A" => Some(0),
        "B" => Some(1),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tests/constants.rs"]
mod tests;
