use serde_json::{Value, json};

// ── Task ID generator ───────────────────────────

pub struct TaskIdGen {
    counter: u16,
}

impl TaskIdGen {
    pub fn new() -> Self {
        TaskIdGen { counter: 0 }
    }

    pub fn next(&mut self) -> String {
        let id = format!("{:04x}", self.counter);
        self.counter = self.counter.wrapping_add(1);
        id
    }
}

// ── Outbound command builders ───────────────────
// All commands return a String with trailing \r ready to write to serial.

/// Switch the hub into play mode (required before motor/sensor commands).
pub fn cmd_program_modechange() -> String {
    let msg = json!({"m": "program_modechange", "p": {"mode": "play"}});
    format!("{}\r", msg)
}

/// Start motor running continuously at the given speed (-100..100).
pub fn cmd_motor_start(task_id: &str, port: &str, speed: i32, stall: bool) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.motor_start",
        "p": {
            "port": port,
            "speed": speed,
            "stall": stall,
            "acceleration": crate::constants::DEFAULT_ACCEL,
        }
    });
    format!("{}\r", msg)
}

/// Stop motor.
pub fn cmd_motor_stop(task_id: &str, port: &str, stop: u8, decel: u16) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.motor_stop",
        "p": {
            "port": port,
            "stop": stop,
            "deceleration": decel,
        }
    });
    format!("{}\r", msg)
}

/// Run motor for a number of milliseconds.
pub fn cmd_motor_run_timed(
    task_id: &str,
    port: &str,
    speed: i32,
    time_ms: u32,
    stall: bool,
    stop: u8,
) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.motor_run_timed",
        "p": {
            "port": port,
            "speed": speed,
            "time": time_ms,
            "stall": stall,
            "stop": stop,
            "acceleration": crate::constants::DEFAULT_ACCEL,
            "deceleration": crate::constants::DEFAULT_DECEL,
        }
    });
    format!("{}\r", msg)
}

/// Run motor for a number of degrees.
pub fn cmd_motor_run_for_degrees(
    task_id: &str,
    port: &str,
    speed: i32,
    degrees: i32,
    stall: bool,
    stop: u8,
) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.motor_run_for_degrees",
        "p": {
            "port": port,
            "speed": speed,
            "degrees": degrees,
            "stall": stall,
            "stop": stop,
            "acceleration": crate::constants::DEFAULT_ACCEL,
            "deceleration": crate::constants::DEFAULT_DECEL,
        }
    });
    format!("{}\r", msg)
}

/// Go to an absolute position with direction hint.
/// `direction` is "shortest", "clockwise", or "counterclockwise".
pub fn cmd_motor_go_direction_to_position(
    task_id: &str,
    port: &str,
    position: i32,
    speed: i32,
    direction: &str,
    stall: bool,
    stop: u8,
) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.motor_go_direction_to_position",
        "p": {
            "port": port,
            "position": position,
            "speed": speed,
            "direction": direction,
            "stall": stall,
            "stop": stop,
            "acceleration": crate::constants::DEFAULT_ACCEL,
            "deceleration": crate::constants::DEFAULT_DECEL,
        }
    });
    format!("{}\r", msg)
}

/// Reset the encoder zero point.
pub fn cmd_motor_set_position(task_id: &str, port: &str, offset: i32) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.motor_set_position",
        "p": {
            "port": port,
            "offset": offset,
        }
    });
    format!("{}\r", msg)
}

/// Start two motors simultaneously at independent speeds.
pub fn cmd_move_start_speeds(
    task_id: &str,
    lmotor: &str,
    rmotor: &str,
    lspeed: i32,
    rspeed: i32,
) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.move_start_speeds",
        "p": {
            "lmotor": lmotor,
            "rmotor": rmotor,
            "lspeed": lspeed,
            "rspeed": rspeed,
            "acceleration": crate::constants::DEFAULT_ACCEL,
        }
    });
    format!("{}\r", msg)
}

/// Stop two motors simultaneously.
pub fn cmd_move_stop(task_id: &str, lmotor: &str, rmotor: &str, stop: u8) -> String {
    let msg = json!({
        "i": task_id,
        "m": "scratch.move_stop",
        "p": {
            "lmotor": lmotor,
            "rmotor": rmotor,
            "stop": stop,
        }
    });
    format!("{}\r", msg)
}

// ── Inbound message parsing ─────────────────────

/// Per-port telemetry from the hub.
#[derive(Debug, Clone, Default)]
pub struct PortTelemetry {
    /// LPF2 device type ID (0 = no device).
    pub device_type: u16,
    /// Up to 4 data values. Meaning depends on device type:
    ///   Motors: [speed, relative_pos, absolute_pos, power]
    ///   Color sensor: [reflection, color, r, g] (b in overflow slot or needs mode switch)
    ///   Distance sensor: [distance_cm, ?, ?, ?]
    ///   Force sensor: [force_newtons, ?, ?, ?]
    pub data: [f64; 4],
}

/// IMU data from the hub's built-in sensors.
#[derive(Debug, Clone, Default)]
pub struct ImuData {
    pub accel: [f64; 3],
    pub gyro: [f64; 3],
    pub yaw_pitch_roll: [f64; 3],
}

/// Telemetry snapshot from an m=0 message.
#[derive(Debug, Clone)]
pub struct TelemetryData {
    pub ports: [PortTelemetry; crate::constants::PORT_COUNT],
    pub imu: ImuData,
}

/// Parsed inbound message from the hub.
#[derive(Debug, Clone)]
pub enum SpikeMessage {
    /// Task completion: hub finished executing a command.
    TaskComplete { task_id: String, result: Value },
    /// Periodic telemetry (m=0): port devices, sensor data, IMU.
    Telemetry(TelemetryData),
    /// Battery status (m=2).
    Battery { voltage: f64, percentage: f64 },
    /// Message we don't need to handle.
    Unknown,
}

/// Parse a single JSON line from the hub.
pub fn parse_message(line: &str) -> SpikeMessage {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return SpikeMessage::Unknown;
    };

    // Task completion: {"i":"xxxx","r":<result>}
    if let Some(task_id) = v.get("i").and_then(|v| v.as_str()) {
        if let Some(result) = v.get("r") {
            return SpikeMessage::TaskComplete {
                task_id: task_id.to_string(),
                result: result.clone(),
            };
        }
    }

    // Event message: {"m":<code>,"p":<data>}
    if let Some(m) = v.get("m").and_then(|v| v.as_u64()) {
        match m {
            0 => return parse_telemetry(&v),
            2 => return parse_battery(&v),
            _ => {}
        }
    }

    SpikeMessage::Unknown
}

fn parse_telemetry(v: &Value) -> SpikeMessage {
    let p = match v.get("p").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return SpikeMessage::Unknown,
    };

    let mut data = TelemetryData {
        ports: Default::default(),
        imu: ImuData::default(),
    };

    // The telemetry array layout from the SPIKE Scratch VM:
    // p[0]: array of 6 port sub-arrays, each [device_type, ...data...]
    // p[6]: accelerometer [x, y, z]
    // p[7]: gyroscope [x, y, z]
    // p[8]: orientation [yaw, pitch, roll]
    //
    // Port sub-array for motors: [type, [speed, rel_pos, abs_pos, power]]
    // Port sub-array for sensors: [type, [val0, val1, val2, val3]]
    // Port sub-array for empty: [0, []]
    if let Some(port_data) = p.first().and_then(|v| v.as_array()) {
        for (i, port_entry) in port_data.iter().enumerate().take(crate::constants::PORT_COUNT) {
            if let Some(arr) = port_entry.as_array() {
                if arr.len() >= 2 {
                    data.ports[i].device_type = arr[0].as_f64().unwrap_or(0.0) as u16;
                    if let Some(vals) = arr[1].as_array() {
                        for (j, val) in vals.iter().enumerate().take(4) {
                            data.ports[i].data[j] = val.as_f64().unwrap_or(0.0);
                        }
                    }
                }
            }
        }
    }

    // IMU data — indices 6, 7, 8 in the top-level p array
    fn read_xyz(p: &[Value], index: usize) -> [f64; 3] {
        p.get(index)
            .and_then(|v| v.as_array())
            .map(|a| {
                [
                    a.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                    a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                    a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
                ]
            })
            .unwrap_or([0.0; 3])
    }

    data.imu.accel = read_xyz(p, 6);
    data.imu.gyro = read_xyz(p, 7);
    data.imu.yaw_pitch_roll = read_xyz(p, 8);

    SpikeMessage::Telemetry(data)
}

fn parse_battery(v: &Value) -> SpikeMessage {
    let p = match v.get("p").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return SpikeMessage::Unknown,
    };
    let voltage = p.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let percentage = p.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
    SpikeMessage::Battery { voltage, percentage }
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
