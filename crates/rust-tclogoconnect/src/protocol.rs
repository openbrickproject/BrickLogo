use crate::constants::*;

/// A parsed line from the device.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    /// An unsolicited input report (`Ixx`) — the sensor bitmask (bit
    /// `INPUT_BIT_PORT6`/`INPUT_BIT_PORT7` = pressed).
    Input(u8),
    /// Any other line: a command reply (`CF`, a version string, a `T`
    /// stats line, `BOOT`) or something unrecognized.
    Reply(String),
}

/// Encode a `D` frame setting all six outputs from one bitmask. Only the
/// low six bits are wired; any others are masked off.
pub fn encode_set_outputs(mask: u8) -> String {
    format!("D{:02X}\n", mask & OUTPUT_MASK)
}

/// Encode a batch of `D` frames as one string — one frame per desired 1ms
/// replay slot. The device's 64-entry queue replays them at exactly 1kHz,
/// so concatenating frames here and writing them in one serial write lets
/// the caller do host-side PWM.
pub fn encode_batch(masks: &[u8]) -> String {
    masks.iter().map(|&m| encode_set_outputs(m)).collect()
}

/// Parse one line, already stripped of its terminator. Lines are matched by
/// content, not by which terminator they arrived with (replies end `\r\n`,
/// input reports end with a bare `\n`, but a trailing `\r` is trimmed
/// either way so both shapes parse identically).
pub fn parse_line(line: &str) -> Line {
    let trimmed = line.trim_end_matches('\r');
    if let Some(hex) = trimmed.strip_prefix('I') {
        if hex.len() == 2 {
            if let Ok(mask) = u8::from_str_radix(hex, 16) {
                return Line::Input(mask);
            }
        }
    }
    Line::Reply(trimmed.to_string())
}

/// Pull every complete (`\n`-terminated) line out of `buffer`, leaving any
/// trailing partial line in place for the next call. Empty lines are
/// dropped.
pub fn drain_lines(buffer: &mut Vec<u8>) -> Vec<Line> {
    let mut lines = Vec::new();
    loop {
        let Some(pos) = buffer.iter().position(|&b| b == b'\n') else {
            break;
        };
        let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
        let text = String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1]).into_owned();
        if !text.is_empty() {
            lines.push(parse_line(&text));
        }
    }
    lines
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
