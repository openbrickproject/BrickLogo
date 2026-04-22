//! LEGO SPIKE Prime COBS framing.
//!
//! This is NOT standard COBS. LEGO's variant:
//!
//! - Any byte value ≤ `DELIMITER` (0x02) is a "delimiter" that ends a block.
//!   Output never contains 0x00, 0x01, or 0x02.
//! - Block size limit is `MAX_BLOCK_SIZE` (84 bytes incl. the code word),
//!   not 254.
//! - Code word encodes both the block length and which delimiter value
//!   terminated the block: `code = delimiter_value * MAX_BLOCK_SIZE + block
//!   + COBS_CODE_OFFSET`. A full-size block with no delimiter uses the
//!   sentinel code `NO_DELIMITER` (0xFF).
//! - After encoding, every byte is XOR'd with `XOR` (3). This shifts the
//!   encoded range [0x03, 0xFF] so it cannot overlap with the frame
//!   delimiter 0x02 or the high-priority marker 0x01.
//! - Each frame ends with a single 0x02 delimiter byte.
//!
//! Ported from LEGO's Python reference
//! (<https://github.com/LEGO/spike-prime-docs/blob/main/examples/python/cobs.py>).

pub const DELIMITER: u8 = 0x02;
pub const NO_DELIMITER: u8 = 0xFF;
pub const COBS_CODE_OFFSET: u8 = DELIMITER;
pub const MAX_BLOCK_SIZE: usize = 84;
pub const XOR: u8 = 3;

/// COBS encode. Output never contains bytes `0x00`, `0x01`, or `0x02`.
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(data.len() + data.len() / MAX_BLOCK_SIZE + 2);
    let mut code_index: usize = 0;
    let mut block: usize = 0;

    fn begin_block(buf: &mut Vec<u8>, code_index: &mut usize, block: &mut usize) {
        *code_index = buf.len();
        buf.push(NO_DELIMITER);
        *block = 1;
    }

    begin_block(&mut buf, &mut code_index, &mut block);
    for &byte in data {
        if byte > DELIMITER {
            buf.push(byte);
            block += 1;
        }
        if byte <= DELIMITER || block > MAX_BLOCK_SIZE {
            if byte <= DELIMITER {
                let delimiter_base = (byte as usize) * MAX_BLOCK_SIZE;
                let block_offset = block + COBS_CODE_OFFSET as usize;
                buf[code_index] = (delimiter_base + block_offset) as u8;
            }
            begin_block(&mut buf, &mut code_index, &mut block);
        }
    }
    buf[code_index] = (block + COBS_CODE_OFFSET as usize) as u8;
    buf
}

/// COBS decode. Input must be an already-decoded (XOR removed, delimiter
/// stripped) COBS stream — use `unpack` for a full frame.
pub fn decode(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let mut buf = Vec::with_capacity(data.len());

    fn unescape(code: u8) -> (Option<u8>, i32) {
        if code == NO_DELIMITER {
            return (None, (MAX_BLOCK_SIZE as i32) + 1);
        }
        let raw = (code - COBS_CODE_OFFSET) as i32;
        let mut value = raw / MAX_BLOCK_SIZE as i32;
        let mut block = raw % MAX_BLOCK_SIZE as i32;
        if block == 0 {
            block = MAX_BLOCK_SIZE as i32;
            value -= 1;
        }
        (Some(value as u8), block)
    }

    let (mut value, mut block) = unescape(data[0]);
    for &byte in &data[1..] {
        block -= 1;
        if block > 0 {
            buf.push(byte);
            continue;
        }
        if let Some(v) = value {
            buf.push(v);
        }
        let (nv, nb) = unescape(byte);
        value = nv;
        block = nb;
    }
    Ok(buf)
}

/// Encode `data` and wrap in a transmission frame: XOR each byte with `XOR`,
/// then append the `DELIMITER` byte.
pub fn pack(data: &[u8]) -> Vec<u8> {
    let mut buf = encode(data);
    for b in buf.iter_mut() {
        *b ^= XOR;
    }
    buf.push(DELIMITER);
    buf
}

/// Inverse of `pack`. `frame` is expected to be the bytes between delimiters
/// with the trailing delimiter still attached (exactly one `0x02` at the end).
/// An optional leading `0x01` "priority" byte is tolerated and stripped.
pub fn unpack(frame: &[u8]) -> Result<Vec<u8>, String> {
    if frame.is_empty() {
        return Err("unpack: empty frame".to_string());
    }
    let mut start = 0;
    if frame[0] == 0x01 {
        start += 1;
    }
    let end = if frame.last() == Some(&DELIMITER) {
        frame.len() - 1
    } else {
        frame.len()
    };
    if start >= end {
        return Ok(Vec::new());
    }
    let unframed: Vec<u8> = frame[start..end].iter().map(|b| b ^ XOR).collect();
    decode(&unframed)
}

/// Framer constants so the delimiter is accessible by its canonical name.
pub const END_FRAME: u8 = DELIMITER;
pub const HIGH_PRIORITY_FRAME: u8 = 0x01;

#[cfg(test)]
#[path = "tests/cobs.rs"]
mod tests;
