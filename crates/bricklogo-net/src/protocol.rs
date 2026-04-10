use std::collections::HashMap;
use bricklogo_lang::value::LogoValue;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetMessage {
    #[serde(rename = "sync")]
    Sync,
    #[serde(rename = "snapshot")]
    Snapshot {
        vars: HashMap<String, LogoValue>,
    },
    #[serde(rename = "set")]
    Set {
        name: String,
        value: LogoValue,
    },
}

pub fn encode(msg: &NetMessage) -> String {
    let mut s = serde_json::to_string(msg).expect("NetMessage serialization failed");
    s.push('\n');
    s
}

pub fn decode(line: &str) -> Result<NetMessage, String> {
    serde_json::from_str(line.trim()).map_err(|e| format!("Protocol decode error: {}", e))
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
