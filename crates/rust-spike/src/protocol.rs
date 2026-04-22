//! JSON command builders for the BrickLogo SPIKE Prime agent protocol.
//!
//! Each command is a single JSON object, one newline-delimited line per
//! `TunnelMessage` payload. The agent replies with another JSON object keyed
//! by the same `id`.

use serde_json::{json, Value};

/// Assign an id and return a byte payload (JSON + trailing newline) ready to
/// hand to the slot. The slot wraps the bytes in a `TunnelMessage` and
/// correlates the reply id back to the caller.
pub fn encode_command(id: u64, body: Value) -> Vec<u8> {
    let mut obj = body;
    if let Value::Object(ref mut map) = obj {
        map.insert("id".to_string(), Value::from(id));
    }
    let mut bytes = serde_json::to_vec(&obj).expect("json serialization");
    bytes.push(b'\n');
    bytes
}

// ── Motor commands ──────────────────────────────

pub fn motor_run(port: &str, velocity: i32) -> Value {
    json!({"op": "motor_run", "port": port, "velocity": velocity})
}

pub fn motor_stop(port: &str) -> Value {
    json!({"op": "motor_stop", "port": port})
}

pub fn motor_reset(port: &str, offset: i32) -> Value {
    json!({"op": "motor_reset", "port": port, "offset": offset})
}

pub fn motor_run_for_time(port: &str, ms: u32, velocity: i32) -> Value {
    json!({"op": "motor_run_for_time", "port": port, "ms": ms, "velocity": velocity})
}

pub fn motor_run_for_degrees(port: &str, degrees: i32, velocity: i32) -> Value {
    json!({"op": "motor_run_for_degrees", "port": port, "degrees": degrees, "velocity": velocity})
}

pub fn motor_run_to_abs(port: &str, position: i32, velocity: i32, direction: u8) -> Value {
    json!({
        "op": "motor_run_to_abs",
        "port": port,
        "position": position,
        "velocity": velocity,
        "direction": direction,
    })
}

// ── Parallel motor commands ─────────────────────

pub fn parallel_run_for_time(entries: &[(&str, i32)], ms: u32) -> Value {
    let arr: Vec<Value> = entries
        .iter()
        .map(|(p, v)| json!({"port": p, "velocity": v}))
        .collect();
    json!({"op": "parallel_run_for_time", "ms": ms, "entries": arr})
}

pub fn parallel_run_for_degrees(entries: &[(&str, i32, i32)]) -> Value {
    let arr: Vec<Value> = entries
        .iter()
        .map(|(p, d, v)| json!({"port": p, "degrees": d, "velocity": v}))
        .collect();
    json!({"op": "parallel_run_for_degrees", "entries": arr})
}

pub fn parallel_run_to_abs(entries: &[(&str, i32, i32, u8)]) -> Value {
    let arr: Vec<Value> = entries
        .iter()
        .map(|(p, pos, v, dir)| {
            json!({"port": p, "position": pos, "velocity": v, "direction": dir})
        })
        .collect();
    json!({"op": "parallel_run_to_abs", "entries": arr})
}

// ── Sensor read commands ────────────────────────

pub fn read_sensor(port: &str, mode: &str) -> Value {
    json!({"op": "read", "port": port, "mode": mode})
}

pub fn read_hub(mode: &str) -> Value {
    json!({"op": "read", "mode": mode})
}

// ── Misc ────────────────────────────────────────

pub fn ping() -> Value {
    json!({"op": "ping"})
}

/// Parse an agent response payload. Returns `Ok(value)` for success or
/// `Err(msg)` for an error reply. Read commands return the `value` field;
/// write commands return `Value::Null`.
pub fn parse_reply(bytes: &[u8]) -> Result<Value, String> {
    let v: Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("agent reply not JSON: {}", e))?;
    if let Some(err) = v.get("error") {
        return Err(err.as_str().unwrap_or("unknown error").to_string());
    }
    if let Some(val) = v.get("value") {
        return Ok(val.clone());
    }
    Ok(Value::Null)
}

/// Return the `id` field of a reply, if any. `None` on startup messages
/// like `{"op": "ready"}`.
pub fn reply_id(bytes: &[u8]) -> Option<u64> {
    serde_json::from_slice::<Value>(bytes)
        .ok()?
        .get("id")
        .and_then(|v| v.as_u64())
}

/// Return `true` if this is the agent's startup ready signal.
pub fn is_ready(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|v| v.get("op").and_then(|v| v.as_str()).map(|s| s.to_string()))
        == Some("ready".to_string())
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
