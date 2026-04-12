use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use bricklogo_lang::value::LogoValue;

// ── Message type ────────────────────────────────

#[derive(Debug, Clone)]
pub enum NetMessage {
    Hello { password: Option<String>, binary_protocol: bool },
    Hi,
    Sync,
    Snapshot { vars: HashMap<String, LogoValue> },
    Set { vars: HashMap<String, LogoValue> },
}

// ── JSON encoding/decoding ──────────────────────

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum JsonMessage {
    #[serde(rename = "hello")]
    Hello {
        #[serde(skip_serializing_if = "Option::is_none")]
        password: Option<String>,
        #[serde(rename = "binaryProtocol", default, skip_serializing_if = "is_false")]
        binary_protocol: bool,
    },
    #[serde(rename = "hi")]
    Hi,
    #[serde(rename = "sync")]
    Sync,
    #[serde(rename = "snapshot")]
    Snapshot { vars: HashMap<String, LogoValue> },
    #[serde(rename = "set")]
    Set { vars: HashMap<String, LogoValue> },
}

fn is_false(b: &bool) -> bool { !b }

impl From<&NetMessage> for JsonMessage {
    fn from(msg: &NetMessage) -> Self {
        match msg {
            NetMessage::Hello { password, binary_protocol } => JsonMessage::Hello {
                password: password.clone(),
                binary_protocol: *binary_protocol,
            },
            NetMessage::Hi => JsonMessage::Hi,
            NetMessage::Sync => JsonMessage::Sync,
            NetMessage::Snapshot { vars } => JsonMessage::Snapshot { vars: vars.clone() },
            NetMessage::Set { vars } => JsonMessage::Set { vars: vars.clone() },
        }
    }
}

impl From<JsonMessage> for NetMessage {
    fn from(msg: JsonMessage) -> Self {
        match msg {
            JsonMessage::Hello { password, binary_protocol } => NetMessage::Hello { password, binary_protocol },
            JsonMessage::Hi => NetMessage::Hi,
            JsonMessage::Sync => NetMessage::Sync,
            JsonMessage::Snapshot { vars } => NetMessage::Snapshot { vars },
            JsonMessage::Set { vars } => NetMessage::Set { vars },
        }
    }
}

pub fn encode_json(msg: &NetMessage) -> String {
    let json_msg: JsonMessage = msg.into();
    serde_json::to_string(&json_msg).expect("JSON serialization failed")
}

pub fn decode_json(text: &str) -> Result<NetMessage, String> {
    let json_msg: JsonMessage = serde_json::from_str(text)
        .map_err(|e| format!("JSON decode error: {}", e))?;
    Ok(json_msg.into())
}

// ── Binary encoding/decoding ────────────────────

const OP_HELLO: u8 = 0x01;
const OP_HI: u8 = 0x02;
const OP_SYNC: u8 = 0x03;
const OP_SNAPSHOT: u8 = 0x04;
const OP_SET: u8 = 0x05;

const VAL_NUMBER: u8 = 0x01;
const VAL_WORD: u8 = 0x02;
const VAL_LIST: u8 = 0x03;

const FLAG_AUTH: u8 = 0x01;
const FLAG_BINARY: u8 = 0x02;

pub fn encode_binary(msg: &NetMessage) -> Vec<u8> {
    let mut payload = Vec::new();
    match msg {
        NetMessage::Hello { password, binary_protocol } => {
            payload.push(OP_HELLO);
            let mut flags: u8 = 0;
            if password.is_some() { flags |= FLAG_AUTH; }
            if *binary_protocol { flags |= FLAG_BINARY; }
            payload.push(flags);
            if let Some(pw) = password {
                write_string(&mut payload, pw);
            }
        }
        NetMessage::Hi => {
            payload.push(OP_HI);
        }
        NetMessage::Sync => {
            payload.push(OP_SYNC);
        }
        NetMessage::Snapshot { vars } => {
            payload.push(OP_SNAPSHOT);
            write_vars(&mut payload, vars);
        }
        NetMessage::Set { vars } => {
            payload.push(OP_SET);
            write_vars(&mut payload, vars);
        }
    }
    // Length prefix
    let len = payload.len() as u32;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&payload);
    buf
}

pub fn decode_binary(data: &[u8]) -> Result<NetMessage, String> {
    if data.is_empty() {
        return Err("Empty message".to_string());
    }

    // Skip length prefix if present (4 bytes)
    let payload = if data.len() >= 5 {
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if 4 + len == data.len() {
            &data[4..]
        } else {
            data
        }
    } else {
        data
    };

    if payload.is_empty() {
        return Err("Empty payload".to_string());
    }

    let mut pos = 0;
    let opcode = payload[pos];
    pos += 1;

    match opcode {
        OP_HELLO => {
            if pos >= payload.len() {
                return Err("Hello: missing flags".to_string());
            }
            let flags = payload[pos];
            pos += 1;
            let has_password = flags & FLAG_AUTH != 0;
            let binary_protocol = flags & FLAG_BINARY != 0;
            let password = if has_password {
                Some(read_string(payload, &mut pos)?)
            } else {
                None
            };
            Ok(NetMessage::Hello { password, binary_protocol })
        }
        OP_HI => Ok(NetMessage::Hi),
        OP_SYNC => Ok(NetMessage::Sync),
        OP_SNAPSHOT => {
            let vars = read_vars(payload, &mut pos)?;
            Ok(NetMessage::Snapshot { vars })
        }
        OP_SET => {
            let vars = read_vars(payload, &mut pos)?;
            Ok(NetMessage::Set { vars })
        }
        _ => Err(format!("Unknown opcode: {:#04x}", opcode)),
    }
}

// ── Binary helpers ──────────────────────────────

fn write_u16(buf: &mut Vec<u8>, val: u16) {
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

fn write_vars(buf: &mut Vec<u8>, vars: &HashMap<String, LogoValue>) {
    write_u16(buf, vars.len() as u16);
    for (name, value) in vars {
        write_string(buf, name);
        write_value(buf, value);
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

fn read_vars(data: &[u8], pos: &mut usize) -> Result<HashMap<String, LogoValue>, String> {
    let count = read_u16(data, pos)?;
    let mut vars = HashMap::new();
    for _ in 0..count {
        let name = read_string(data, pos)?;
        let value = read_value(data, pos)?;
        vars.insert(name, value);
    }
    Ok(vars)
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
