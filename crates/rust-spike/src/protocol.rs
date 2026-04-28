//! Binary command/reply protocol carried inside Atlantis `TunnelMessage`
//! payloads. One tunnel message = one command; one tunnel message = one
//! reply. All multi-byte fields are little-endian.
//!
//! ## Requests (host → agent)
//!
//! | op | name                     | payload                                                        |
//! |----|--------------------------|----------------------------------------------------------------|
//! | 01 | motor_run                | rid u16, port u8, velocity i16                                 |
//! | 02 | motor_stop               | rid u16, port u8                                               |
//! | 03 | motor_reset              | rid u16, port u8, offset i32                                   |
//! | 04 | motor_run_for_time       | rid u16, port u8, ms u32, velocity i16                         |
//! | 05 | motor_run_for_degrees    | rid u16, port u8, degrees i32, velocity i16                    |
//! | 06 | motor_run_to_abs         | rid u16, port u8, position i32, velocity i16, direction u8     |
//! | 07 | parallel_run_for_time    | rid u16, ms u32, count u8, (port u8, velocity i16) × count     |
//! | 08 | parallel_run_for_degrees | rid u16, count u8, (port u8, degrees i32, velocity i16) × count|
//! | 09 | parallel_run_to_abs      | rid u16, count u8, (port u8, pos i32, vel i16, dir u8) × count |
//! | 0A | read                     | rid u16, port u8, mode u8                                      |
//! | 0B | read_hub                 | rid u16, mode u8                                               |
//! | 0C | ping                     | rid u16                                                        |
//! | 0D | port_types               | rid u16                                                        |
//! | 0E | port_pwm                 | rid u16, port u8, value i8 (-100..100)                         |
//!
//! ## Replies (agent → host)
//!
//! | kind | name       | payload                                |
//! |------|------------|----------------------------------------|
//! | 00   | ok         | rid u16                                |
//! | 01   | int        | rid u16, value i32                     |
//! | 02   | list       | rid u16, count u8, i32 × count         |
//! | 03   | bool       | rid u16, value u8                      |
//! | 04   | error      | rid u16, len u8, utf8_bytes × len      |
//! | 05   | type_list  | rid u16, count u8, u16 × count         |
//! | 10   | ready       | (no payload)                                       |
//! | 11   | heartbeat   | (no payload)                                       |
//! | 12   | port_event  | port u8, type u16  (no rid; unsolicited on change) |

// ── Opcodes ─────────────────────────────────────

pub const OP_MOTOR_RUN: u8 = 0x01;
pub const OP_MOTOR_STOP: u8 = 0x02;
pub const OP_MOTOR_RESET: u8 = 0x03;
pub const OP_MOTOR_RUN_FOR_TIME: u8 = 0x04;
pub const OP_MOTOR_RUN_FOR_DEGREES: u8 = 0x05;
pub const OP_MOTOR_RUN_TO_ABS: u8 = 0x06;
pub const OP_PARALLEL_RUN_FOR_TIME: u8 = 0x07;
pub const OP_PARALLEL_RUN_FOR_DEGREES: u8 = 0x08;
pub const OP_PARALLEL_RUN_TO_ABS: u8 = 0x09;
pub const OP_READ: u8 = 0x0A;
pub const OP_READ_HUB: u8 = 0x0B;
pub const OP_PING: u8 = 0x0C;
pub const OP_PORT_TYPES: u8 = 0x0D;
pub const OP_PORT_PWM: u8 = 0x0E;

// ── Reply kinds ─────────────────────────────────

pub const REPLY_OK: u8 = 0x00;
pub const REPLY_INT: u8 = 0x01;
pub const REPLY_LIST: u8 = 0x02;
pub const REPLY_BOOL: u8 = 0x03;
pub const REPLY_ERROR: u8 = 0x04;
pub const REPLY_TYPE_LIST: u8 = 0x05;
pub const REPLY_READY: u8 = 0x10;
pub const REPLY_HEARTBEAT: u8 = 0x11;
pub const REPLY_PORT_EVENT: u8 = 0x12;

// ── Sensor modes ────────────────────────────────

pub const MODE_ROTATION: u8 = 0x00;
pub const MODE_ABSOLUTE: u8 = 0x01;
pub const MODE_SPEED: u8 = 0x02;
pub const MODE_COLOR: u8 = 0x03;
pub const MODE_LIGHT: u8 = 0x04;
pub const MODE_DISTANCE: u8 = 0x05;
pub const MODE_FORCE: u8 = 0x06;
pub const MODE_TOUCHED: u8 = 0x07;
pub const MODE_TILT: u8 = 0x08;
pub const MODE_GYRO: u8 = 0x09;
pub const MODE_ACCEL: u8 = 0x0A;

// ── Encoding helpers ────────────────────────────

pub fn port_index(letter: &str) -> Result<u8, String> {
    match letter.to_lowercase().as_str() {
        "a" => Ok(0),
        "b" => Ok(1),
        "c" => Ok(2),
        "d" => Ok(3),
        "e" => Ok(4),
        "f" => Ok(5),
        _ => Err(format!("Unknown port \"{}\"", letter)),
    }
}

pub fn sensor_mode(name: &str) -> Result<u8, String> {
    match name {
        "rotation" | "raw" => Ok(MODE_ROTATION),
        "absolute" => Ok(MODE_ABSOLUTE),
        "speed" => Ok(MODE_SPEED),
        "color" => Ok(MODE_COLOR),
        "light" => Ok(MODE_LIGHT),
        "distance" => Ok(MODE_DISTANCE),
        "force" => Ok(MODE_FORCE),
        "touched" => Ok(MODE_TOUCHED),
        "tilt" => Ok(MODE_TILT),
        "gyro" => Ok(MODE_GYRO),
        "accel" => Ok(MODE_ACCEL),
        _ => Err(format!("Unsupported sensor mode \"{}\"", name)),
    }
}

fn header(op: u8, rid: u16, cap: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(cap);
    buf.push(op);
    buf.extend_from_slice(&rid.to_le_bytes());
    buf
}

pub fn motor_run(rid: u16, port: &str, velocity: i16) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let mut buf = header(OP_MOTOR_RUN, rid, 6);
    buf.push(p);
    buf.extend_from_slice(&velocity.to_le_bytes());
    Ok(buf)
}

pub fn motor_stop(rid: u16, port: &str) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let mut buf = header(OP_MOTOR_STOP, rid, 4);
    buf.push(p);
    Ok(buf)
}

pub fn motor_reset(rid: u16, port: &str, offset: i32) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let mut buf = header(OP_MOTOR_RESET, rid, 8);
    buf.push(p);
    buf.extend_from_slice(&offset.to_le_bytes());
    Ok(buf)
}

pub fn motor_run_for_time(
    rid: u16,
    port: &str,
    ms: u32,
    velocity: i16,
) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let mut buf = header(OP_MOTOR_RUN_FOR_TIME, rid, 10);
    buf.push(p);
    buf.extend_from_slice(&ms.to_le_bytes());
    buf.extend_from_slice(&velocity.to_le_bytes());
    Ok(buf)
}

pub fn motor_run_for_degrees(
    rid: u16,
    port: &str,
    degrees: i32,
    velocity: i16,
) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let mut buf = header(OP_MOTOR_RUN_FOR_DEGREES, rid, 10);
    buf.push(p);
    buf.extend_from_slice(&degrees.to_le_bytes());
    buf.extend_from_slice(&velocity.to_le_bytes());
    Ok(buf)
}

pub fn motor_run_to_abs(
    rid: u16,
    port: &str,
    position: i32,
    velocity: i16,
    direction: u8,
) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let mut buf = header(OP_MOTOR_RUN_TO_ABS, rid, 11);
    buf.push(p);
    buf.extend_from_slice(&position.to_le_bytes());
    buf.extend_from_slice(&velocity.to_le_bytes());
    buf.push(direction);
    Ok(buf)
}

pub fn parallel_run_for_time(
    rid: u16,
    ms: u32,
    entries: &[(&str, i16)],
) -> Result<Vec<u8>, String> {
    if entries.len() > u8::MAX as usize {
        return Err("too many parallel entries".into());
    }
    let mut buf = header(OP_PARALLEL_RUN_FOR_TIME, rid, 8 + entries.len() * 3);
    buf.extend_from_slice(&ms.to_le_bytes());
    buf.push(entries.len() as u8);
    for (p, v) in entries {
        buf.push(port_index(p)?);
        buf.extend_from_slice(&v.to_le_bytes());
    }
    Ok(buf)
}

pub fn parallel_run_for_degrees(
    rid: u16,
    entries: &[(&str, i32, i16)],
) -> Result<Vec<u8>, String> {
    if entries.len() > u8::MAX as usize {
        return Err("too many parallel entries".into());
    }
    let mut buf = header(OP_PARALLEL_RUN_FOR_DEGREES, rid, 4 + entries.len() * 7);
    buf.push(entries.len() as u8);
    for (p, deg, vel) in entries {
        buf.push(port_index(p)?);
        buf.extend_from_slice(&deg.to_le_bytes());
        buf.extend_from_slice(&vel.to_le_bytes());
    }
    Ok(buf)
}

pub fn parallel_run_to_abs(
    rid: u16,
    entries: &[(&str, i32, i16, u8)],
) -> Result<Vec<u8>, String> {
    if entries.len() > u8::MAX as usize {
        return Err("too many parallel entries".into());
    }
    let mut buf = header(OP_PARALLEL_RUN_TO_ABS, rid, 4 + entries.len() * 8);
    buf.push(entries.len() as u8);
    for (p, pos, vel, dir) in entries {
        buf.push(port_index(p)?);
        buf.extend_from_slice(&pos.to_le_bytes());
        buf.extend_from_slice(&vel.to_le_bytes());
        buf.push(*dir);
    }
    Ok(buf)
}

pub fn read_sensor(rid: u16, port: &str, mode: &str) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let m = sensor_mode(mode)?;
    let mut buf = header(OP_READ, rid, 5);
    buf.push(p);
    buf.push(m);
    Ok(buf)
}

pub fn read_hub(rid: u16, mode: &str) -> Result<Vec<u8>, String> {
    let m = sensor_mode(mode)?;
    let mut buf = header(OP_READ_HUB, rid, 4);
    buf.push(m);
    Ok(buf)
}

pub fn ping(rid: u16) -> Vec<u8> {
    header(OP_PING, rid, 3)
}

pub fn port_types(rid: u16) -> Vec<u8> {
    header(OP_PORT_TYPES, rid, 3)
}

/// Direct PWM on a port. The agent dispatches at call time based on the
/// device attached: tacho motors get `motor.run`, passive motors and lights
/// get raw duty-cycle (signed for motors, unsigned for lights).
/// `value` is `-100..100`; the agent scales to the firmware's 0..10000 range.
pub fn port_pwm(rid: u16, port: &str, value: i8) -> Result<Vec<u8>, String> {
    let p = port_index(port)?;
    let mut buf = header(OP_PORT_PWM, rid, 5);
    buf.push(p);
    buf.push(value as u8);
    Ok(buf)
}

// ── Reply parsing ───────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Ok,
    Int(i32),
    List(Vec<i32>),
    Bool(bool),
    Error(String),
    /// Snapshot of LPF2 device type IDs for ports A–F. `0` means no device.
    TypeList(Vec<u16>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Request reply, keyed by `rid`.
    Reply { rid: u16, reply: Reply },
    /// Agent startup signal. No rid.
    Ready,
    /// Agent heartbeat. No rid.
    Heartbeat,
    /// Unsolicited port attach/detach push (agent debounces — only sent on
    /// type-id change). `type_id == 0` means detach.
    PortEvent { port: u8, type_id: u16 },
}

/// Parse one tunnel message payload as an `Event`. Returns `Err` on malformed
/// data so the caller can log / drop the frame.
pub fn parse_event(data: &[u8]) -> Result<Event, String> {
    if data.is_empty() {
        return Err("empty reply".into());
    }
    match data[0] {
        REPLY_READY => Ok(Event::Ready),
        REPLY_HEARTBEAT => Ok(Event::Heartbeat),
        REPLY_PORT_EVENT => {
            if data.len() < 4 {
                return Err("port_event too short".into());
            }
            Ok(Event::PortEvent {
                port: data[1],
                type_id: u16::from_le_bytes([data[2], data[3]]),
            })
        }
        kind => {
            if data.len() < 3 {
                return Err(format!("reply too short for kind {:#x}", kind));
            }
            let rid = u16::from_le_bytes([data[1], data[2]]);
            let reply = parse_reply_body(kind, &data[3..])?;
            Ok(Event::Reply { rid, reply })
        }
    }
}

fn parse_reply_body(kind: u8, body: &[u8]) -> Result<Reply, String> {
    match kind {
        REPLY_OK => Ok(Reply::Ok),
        REPLY_INT => {
            if body.len() < 4 {
                return Err("int reply too short".into());
            }
            let v = i32::from_le_bytes([body[0], body[1], body[2], body[3]]);
            Ok(Reply::Int(v))
        }
        REPLY_LIST => {
            if body.is_empty() {
                return Err("list reply missing count".into());
            }
            let count = body[0] as usize;
            if body.len() < 1 + count * 4 {
                return Err("list reply truncated".into());
            }
            let mut values = Vec::with_capacity(count);
            for i in 0..count {
                let off = 1 + i * 4;
                values.push(i32::from_le_bytes([
                    body[off], body[off + 1], body[off + 2], body[off + 3],
                ]));
            }
            Ok(Reply::List(values))
        }
        REPLY_BOOL => {
            if body.is_empty() {
                return Err("bool reply too short".into());
            }
            Ok(Reply::Bool(body[0] != 0))
        }
        REPLY_TYPE_LIST => {
            if body.is_empty() {
                return Err("type_list reply missing count".into());
            }
            let count = body[0] as usize;
            if body.len() < 1 + count * 2 {
                return Err("type_list reply truncated".into());
            }
            let mut values = Vec::with_capacity(count);
            for i in 0..count {
                let off = 1 + i * 2;
                values.push(u16::from_le_bytes([body[off], body[off + 1]]));
            }
            Ok(Reply::TypeList(values))
        }
        REPLY_ERROR => {
            if body.is_empty() {
                return Err("error reply missing len".into());
            }
            let len = body[0] as usize;
            if body.len() < 1 + len {
                return Err("error reply truncated".into());
            }
            let msg = String::from_utf8_lossy(&body[1..1 + len]).into_owned();
            Ok(Reply::Error(msg))
        }
        _ => Err(format!("unknown reply kind {:#x}", kind)),
    }
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
