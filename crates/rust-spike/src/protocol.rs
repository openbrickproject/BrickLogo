/// Raw REPL protocol for SPIKE Prime (firmware v3.x / MicroPython).
///
/// The hub's raw REPL mode accepts Python code blocks:
///   - Send `\x01` to enter raw REPL → hub responds `raw REPL; ...\r\n>`
///   - Send Python code
///   - Send `\x04` to execute → hub responds `OK<stdout>\x04<stderr>\x04`
///
/// Imports persist across executions in the global namespace, so we
/// import `motor`, `motor_pair`, `runloop`, and `port` once at init.

/// Ctrl+C — interrupt running program.
pub const CTRL_C: u8 = 0x03;
/// Ctrl+A — enter raw REPL mode.
pub const CTRL_A: u8 = 0x01;
/// Ctrl+B — enter friendly REPL mode.
pub const CTRL_B: u8 = 0x02;
/// Ctrl+D — execute code block in raw REPL / soft-reset in friendly REPL.
pub const CTRL_D: u8 = 0x04;

/// Imports needed in the raw REPL session. Sent once after entering raw REPL.
pub fn cmd_init_imports() -> Vec<u8> {
    let code = "import motor, motor_pair, runloop\nfrom hub import port\n";
    let mut buf = code.as_bytes().to_vec();
    buf.push(CTRL_D);
    buf
}

// ── Motor command builders ──────────────────────
// Each returns bytes ready to send: Python code + Ctrl+D.

fn port_ref(port_letter: &str) -> String {
    format!("port.{}", port_letter.to_uppercase())
}

/// Start motor running continuously. Non-blocking.
/// velocity is in degrees/second (roughly: power * 10).
pub fn cmd_motor_run(port: &str, velocity: i32) -> Vec<u8> {
    let code = format!("motor.run({}, {})\n", port_ref(port), velocity);
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

/// Stop motor.
pub fn cmd_motor_stop(port: &str) -> Vec<u8> {
    let code = format!("motor.stop({})\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

/// Run motor for time (blocking, returns awaitable).
/// Wrapped in runloop.run() so it blocks the REPL until done.
pub fn cmd_motor_run_for_time(port: &str, ms: u32, velocity: i32) -> Vec<u8> {
    let code = format!(
        "runloop.run(motor.run_for_time({}, {}, {}))\n",
        port_ref(port), ms, velocity
    );
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

/// Run motor for degrees (blocking).
pub fn cmd_motor_run_for_degrees(port: &str, degrees: i32, velocity: i32) -> Vec<u8> {
    let code = format!(
        "runloop.run(motor.run_for_degrees({}, {}, {}))\n",
        port_ref(port), degrees, velocity
    );
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

/// Run motor to absolute position (blocking).
/// direction: 0=shortest, 1=clockwise, 2=counterclockwise
pub fn cmd_motor_run_to_absolute_position(
    port: &str,
    position: i32,
    velocity: i32,
    direction: u8,
) -> Vec<u8> {
    let code = format!(
        "runloop.run(motor.run_to_absolute_position({}, {}, {}, direction={}))\n",
        port_ref(port), position, velocity, direction
    );
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

/// Reset relative position to offset.
pub fn cmd_motor_reset_relative_position(port: &str, offset: i32) -> Vec<u8> {
    let code = format!(
        "motor.reset_relative_position({}, {})\n",
        port_ref(port), offset
    );
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

// ── Parallel motor commands ─────────────────────
// Use runloop.gather() to run multiple awaitables concurrently.

/// Run multiple motors for degrees in parallel (blocking).
/// Each entry: (port_letter, degrees, velocity).
pub fn cmd_parallel_run_for_degrees(entries: &[(&str, i32, i32)]) -> Vec<u8> {
    let tasks: Vec<String> = entries
        .iter()
        .map(|(p, deg, vel)| {
            format!("motor.run_for_degrees({}, {}, {})", port_ref(p), deg, vel)
        })
        .collect();
    let code = format!("runloop.run({})\n", tasks.join(", "));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

/// Run multiple motors for time in parallel (blocking).
pub fn cmd_parallel_run_for_time(entries: &[(&str, i32)], ms: u32) -> Vec<u8> {
    let tasks: Vec<String> = entries
        .iter()
        .map(|(p, vel)| {
            format!("motor.run_for_time({}, {}, {})", port_ref(p), ms, vel)
        })
        .collect();
    let code = format!("runloop.run({})\n", tasks.join(", "));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

/// Run multiple motors to absolute positions in parallel (blocking).
/// Each entry: (port_letter, position, velocity, direction).
pub fn cmd_parallel_run_to_absolute(entries: &[(&str, i32, i32, u8)]) -> Vec<u8> {
    let tasks: Vec<String> = entries
        .iter()
        .map(|(p, pos, vel, dir)| {
            format!(
                "motor.run_to_absolute_position({}, {}, {}, direction={})",
                port_ref(p), pos, vel, dir
            )
        })
        .collect();
    let code = format!("runloop.run({})\n", tasks.join(", "));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

// ── Sensor read commands ────────────────────────
// Each prints a value; the adapter parses stdout.

pub fn cmd_read_relative_position(port: &str) -> Vec<u8> {
    let code = format!("print(motor.relative_position({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

pub fn cmd_read_absolute_position(port: &str) -> Vec<u8> {
    let code = format!("print(motor.absolute_position({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

pub fn cmd_read_velocity(port: &str) -> Vec<u8> {
    let code = format!("print(motor.velocity({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

pub fn cmd_read_color(port: &str) -> Vec<u8> {
    let code = format!("import color_sensor\nprint(color_sensor.color({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

pub fn cmd_read_reflection(port: &str) -> Vec<u8> {
    let code = format!("import color_sensor\nprint(color_sensor.reflection({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

pub fn cmd_read_distance(port: &str) -> Vec<u8> {
    let code = format!("import distance_sensor\nprint(distance_sensor.distance({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

pub fn cmd_read_force(port: &str) -> Vec<u8> {
    let code = format!("import force_sensor\nprint(force_sensor.force({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

pub fn cmd_read_force_touched(port: &str) -> Vec<u8> {
    let code = format!("import force_sensor\nprint(force_sensor.pressed({}))\n", port_ref(port));
    let mut buf = code.into_bytes();
    buf.push(CTRL_D);
    buf
}

// ── Response parsing ────────────────────────────

/// Parse a raw REPL response: `OK<stdout>\x04<stderr>\x04`
/// Returns Ok(stdout) or Err(stderr).
pub fn parse_raw_repl_response(data: &[u8]) -> Result<String, String> {
    // Strip leading "OK" if present
    let data = if data.starts_with(b"OK") {
        &data[2..]
    } else {
        data
    };

    // Split on \x04 — first part is stdout, second is stderr
    let mut parts = data.splitn(3, |&b| b == CTRL_D);
    let stdout = parts
        .next()
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
        .unwrap_or_default();
    let stderr = parts
        .next()
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
        .unwrap_or_default();

    if stderr.is_empty() {
        Ok(stdout)
    } else {
        Err(stderr)
    }
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
