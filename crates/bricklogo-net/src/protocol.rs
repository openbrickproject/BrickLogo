use std::collections::HashMap;
use std::io::{self, Read, Write};
use bricklogo_lang::value::LogoValue;

const OP_SYNC: u8 = 0x01;
const OP_SNAPSHOT: u8 = 0x02;
const OP_SET: u8 = 0x03;

const VAL_NUMBER: u8 = 0x01;
const VAL_WORD: u8 = 0x02;
const VAL_LIST: u8 = 0x03;

#[derive(Debug)]
pub enum NetMessage {
    Sync,
    Snapshot { vars: HashMap<String, LogoValue> },
    Set { name: String, value: LogoValue },
}

// ── Encoding ────────────────────────────────────

pub fn encode(msg: &NetMessage) -> Vec<u8> {
    let mut payload = Vec::new();
    match msg {
        NetMessage::Sync => {
            payload.push(OP_SYNC);
        }
        NetMessage::Snapshot { vars } => {
            payload.push(OP_SNAPSHOT);
            write_u16(&mut payload, vars.len() as u16);
            for (name, value) in vars {
                write_string(&mut payload, name);
                write_value(&mut payload, value);
            }
        }
        NetMessage::Set { name, value } => {
            payload.push(OP_SET);
            write_string(&mut payload, name);
            write_value(&mut payload, value);
        }
    }
    let mut buf = Vec::with_capacity(4 + payload.len());
    write_u32(&mut buf, payload.len() as u32);
    buf.extend_from_slice(&payload);
    buf
}

fn write_u16(buf: &mut Vec<u8>, val: u16) {
    buf.extend_from_slice(&val.to_be_bytes());
}

fn write_u32(buf: &mut Vec<u8>, val: u32) {
    buf.extend_from_slice(&val.to_be_bytes());
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_u16(buf, bytes.len() as u16);
    buf.extend_from_slice(bytes);
}

fn write_value(buf: &mut Vec<u8>, val: &LogoValue) {
    match val {
        LogoValue::Number(n) => {
            buf.push(VAL_NUMBER);
            buf.extend_from_slice(&n.to_be_bytes());
        }
        LogoValue::Word(s) => {
            buf.push(VAL_WORD);
            write_string(buf, s);
        }
        LogoValue::List(items) => {
            buf.push(VAL_LIST);
            write_u16(buf, items.len() as u16);
            for item in items {
                write_value(buf, item);
            }
        }
    }
}

// ── Decoding ────────────────────────────────────

pub fn decode(data: &[u8]) -> Result<NetMessage, String> {
    let mut pos = 0;
    if data.is_empty() {
        return Err("Empty message".to_string());
    }
    let opcode = data[pos];
    pos += 1;

    match opcode {
        OP_SYNC => Ok(NetMessage::Sync),
        OP_SNAPSHOT => {
            let count = read_u16(data, &mut pos)?;
            let mut vars = HashMap::new();
            for _ in 0..count {
                let name = read_string(data, &mut pos)?;
                let value = read_value(data, &mut pos)?;
                vars.insert(name, value);
            }
            Ok(NetMessage::Snapshot { vars })
        }
        OP_SET => {
            let name = read_string(data, &mut pos)?;
            let value = read_value(data, &mut pos)?;
            Ok(NetMessage::Set { name, value })
        }
        _ => Err(format!("Unknown opcode: {:#04x}", opcode)),
    }
}

fn read_u16(data: &[u8], pos: &mut usize) -> Result<u16, String> {
    if *pos + 2 > data.len() {
        return Err("Unexpected end of message".to_string());
    }
    let val = u16::from_be_bytes([data[*pos], data[*pos + 1]]);
    *pos += 2;
    Ok(val)
}

fn read_string(data: &[u8], pos: &mut usize) -> Result<String, String> {
    let len = read_u16(data, pos)? as usize;
    if *pos + len > data.len() {
        return Err("Unexpected end of message".to_string());
    }
    let s = std::str::from_utf8(&data[*pos..*pos + len])
        .map_err(|e| format!("Invalid UTF-8: {}", e))?;
    *pos += len;
    Ok(s.to_string())
}

fn read_value(data: &[u8], pos: &mut usize) -> Result<LogoValue, String> {
    if *pos >= data.len() {
        return Err("Unexpected end of message".to_string());
    }
    let tag = data[*pos];
    *pos += 1;

    match tag {
        VAL_NUMBER => {
            if *pos + 8 > data.len() {
                return Err("Unexpected end of message".to_string());
            }
            let bytes: [u8; 8] = data[*pos..*pos + 8].try_into().unwrap();
            *pos += 8;
            Ok(LogoValue::Number(f64::from_be_bytes(bytes)))
        }
        VAL_WORD => {
            let s = read_string(data, pos)?;
            Ok(LogoValue::Word(s))
        }
        VAL_LIST => {
            let count = read_u16(data, pos)?;
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(read_value(data, pos)?);
            }
            Ok(LogoValue::List(items))
        }
        _ => Err(format!("Unknown value type: {:#04x}", tag)),
    }
}

// ── Stream I/O ──────────────────────────────────

pub fn write_message(stream: &mut dyn Write, msg: &NetMessage) -> io::Result<()> {
    let buf = encode(msg);
    stream.write_all(&buf)?;
    stream.flush()
}

const READ_BUF_SIZE: usize = 8192;

/// Buffered message reader. One per connection. Reads from a TCP stream into a
/// fixed buffer and parses complete frames without per-message allocation.
pub struct MessageReader {
    buf: Vec<u8>,
    len: usize, // bytes of valid data in buf
}

impl MessageReader {
    pub fn new() -> Self {
        MessageReader {
            buf: vec![0u8; READ_BUF_SIZE],
            len: 0,
        }
    }

    /// Read the next complete message from the stream. Blocks until a full
    /// frame is available or the connection closes.
    pub fn read(&mut self, stream: &mut dyn Read) -> io::Result<NetMessage> {
        loop {
            // Try to parse a complete frame from what we have
            if self.len >= 4 {
                let payload_len = u32::from_be_bytes([
                    self.buf[0], self.buf[1], self.buf[2], self.buf[3],
                ]) as usize;

                if payload_len > 16 * 1024 * 1024 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Message too large"));
                }

                let frame_len = 4 + payload_len;
                if self.len >= frame_len {
                    // Complete frame available — decode it
                    let msg = decode(&self.buf[4..frame_len])
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                    // Shift remaining data to the front
                    self.buf.copy_within(frame_len..self.len, 0);
                    self.len -= frame_len;

                    return Ok(msg);
                }

                // Ensure buffer is large enough for the full frame
                if self.buf.len() < frame_len {
                    self.buf.resize(frame_len, 0);
                }
            }

            // Read more data from the stream
            let n = stream.read(&mut self.buf[self.len..])?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Connection closed"));
            }
            self.len += n;
        }
    }
}

/// Simple read_message for one-off reads (initial sync handshake).
/// Uses read_exact — two syscalls per message but no state to manage.
pub fn read_message(stream: &mut dyn Read) -> io::Result<NetMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 16 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Message too large"));
    }

    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    decode(&payload).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
