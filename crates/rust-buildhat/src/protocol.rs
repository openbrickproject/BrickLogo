// ── Command builders ─────────────────────────────
// All commands are ASCII text terminated with \r.

pub fn cmd_version() -> String {
    "version\r".to_string()
}

pub fn cmd_clear() -> String {
    "clear\r".to_string()
}

pub fn cmd_load(size: usize, checksum: u32) -> String {
    format!("load {} {}\r", size, checksum)
}

pub fn cmd_signature(size: usize) -> String {
    format!("signature {}\r", size)
}

pub fn cmd_reboot() -> String {
    "reboot\r".to_string()
}

pub fn cmd_echo_off() -> String {
    "echo 0\r".to_string()
}

pub fn cmd_list() -> String {
    "list\r".to_string()
}

pub fn cmd_select_all_ports() -> String {
    "port 0 ; select ; port 1 ; select ; port 2 ; select ; port 3 ; select\r".to_string()
}

// ── Motor commands ───────────────────────────────

/// Set motor speed (continuous run). Speed: -100 to 100.
/// Enables PWM mode and sets the speed as a fraction (-1.0 to 1.0).
pub fn cmd_motor_set(port: u8, speed: i32) -> String {
    let value = speed.clamp(-100, 100) as f64 / 100.0;
    format!("port {} ; pwm ; set {}\r", port, value)
}

/// Coast motor (free spin).
pub fn cmd_motor_coast(port: u8) -> String {
    format!("port {} ; coast\r", port)
}

/// Hard stop motor (active brake via PWM 0).
pub fn cmd_motor_off(port: u8) -> String {
    format!("port {} ; pwm ; set 0\r", port)
}

/// PID-regulated speed control for tacho motors. Speed: -100 to 100.
pub fn cmd_motor_speed(port: u8, speed: i32) -> String {
    format!(
        "port {} ; pid {} 0 0 s1 1 0 0.15 0.06 0 100 0.01 ; set {}\r",
        port, port, speed
    )
}

/// Direct PWM control. Value: -1.0 to 1.0.
pub fn cmd_motor_pwm(port: u8, value: f64) -> String {
    format!("port {} ; pwm ; set {}\r", port, value)
}

/// Run motor for a number of seconds using PID control.
pub fn cmd_motor_pulse(port: u8, speed: i32, seconds: f64) -> String {
    format!(
        "port {} ; pid {} 0 0 s1 1 0 0.15 0.06 0 100 0.01 ; set pulse {} 0.0 {} 0\r",
        port, port, speed, seconds
    )
}

/// Ramp motor to a position using PID control.
/// Positions are in decimal rotations (degrees / 360).
pub fn cmd_motor_ramp(port: u8, from_pos: f64, to_pos: f64, duration: f64) -> String {
    format!(
        "port {} ; pid {} 0 1 s4 0.0027777778 0 5 0 .1 3 0.01 ; set ramp {} {} {} 0\r",
        port, port, from_pos, to_pos, duration
    )
}

/// Set power limit (0.0 to 1.0).
pub fn cmd_plimit(port: u8, limit: f64) -> String {
    format!("port {} ; port_plimit {}\r", port, limit)
}

// ── Sensor commands ──────────────────────────────

/// Select a sensor mode with update interval (ms).
pub fn cmd_select_mode(port: u8, mode: u8, interval_ms: u32) -> String {
    format!("port {} ; select {} ; selrate {}\r", port, mode, interval_ms)
}

/// Select combined sensor modes with reporting interval (ms).
pub fn cmd_select_combi(port: u8, combi_index: u8, modes: &[(u8, u8)], interval_ms: u32) -> String {
    let mode_str: Vec<String> = modes.iter().map(|(m, s)| format!("{} {}", m, s)).collect();
    format!(
        "port {} ; combi {} {} ; select {} ; selrate {}\r",
        port,
        combi_index,
        mode_str.join(" "),
        combi_index,
        interval_ms
    )
}

/// Deselect sensor mode on a port.
pub fn cmd_deselect(port: u8) -> String {
    format!("port {} ; select\r", port)
}

/// Set sensor LED mode (use -1 to enable default LED behavior for color/distance sensors).
pub fn cmd_set_value(port: u8, value: i32) -> String {
    format!("port {} ; set {}\r", port, value)
}

/// Preset a mode value (e.g. reset position counter to 0).
pub fn cmd_preset(port: u8, mode: u8, value: f64) -> String {
    format!("port {} ; preset {} {}\r", port, mode, value)
}

// ── Response parsing ─────────────────────────────

/// Build HAT state detected from version response.
#[derive(Debug, Clone, PartialEq)]
pub enum HatState {
    Firmware(String),
    Bootloader,
}

/// Parse a version response line.
pub fn parse_version(line: &str) -> Option<HatState> {
    if line.contains("Firmware version:") {
        let version = line.split("Firmware version:").nth(1)?.trim().to_string();
        Some(HatState::Firmware(version))
    } else if line.contains("BuildHAT bootloader") {
        Some(HatState::Bootloader)
    } else {
        None
    }
}

/// Device attachment info from list response.
#[derive(Debug, Clone)]
pub struct DeviceAttach {
    pub port: u8,
    pub type_id: u16,
    pub active: bool,
}

/// Parse a device attach/detach line.
pub fn parse_device_line(line: &str) -> Option<DeviceAttach> {
    // "P0: connected to active ID2e"
    // "P1: connected to passive ID01"
    // "P2: no device detected"
    if !line.starts_with('P') || !line.contains(':') {
        return None;
    }
    let port = line.as_bytes().get(1)?.checked_sub(b'0')? as u8;
    if port > 3 {
        return None;
    }

    if line.contains("no device") || line.contains("disconnected") || line.contains("timeout") {
        return None; // No device on this port
    }

    let active = line.contains("active");
    // Extract hex type ID after "ID"
    if let Some(id_pos) = line.find("ID") {
        let rest = line[id_pos + 2..].trim();
        let hex: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if !hex.is_empty() {
        if let Ok(type_id) = u16::from_str_radix(&hex, 16) {
            return Some(DeviceAttach {
                port,
                type_id,
                active,
            });
        }
        }
    }
    None
}

/// Sensor data from a mode response.
#[derive(Debug, Clone)]
pub struct SensorData {
    pub port: u8,
    pub mode: u8,
    pub values: Vec<f64>,
}

/// Parse a sensor data line: "P0M1: 45 67.5 89"
pub fn parse_sensor_data(line: &str) -> Option<SensorData> {
    if !line.starts_with('P') || (!line.contains('M') && !line.contains('C')) {
        return None;
    }

    let port = line.as_bytes().get(1)?.checked_sub(b'0')? as u8;
    if port > 3 {
        return None;
    }

    // Find mode character (M or C) and mode number
    let mode_char_pos = line.find('M').or_else(|| line.find('C'))?;
    let colon_pos = line.find(':')?;
    let mode_str = &line[mode_char_pos + 1..colon_pos];
    let mode: u8 = mode_str.parse().ok()?;

    let data_str = line[colon_pos + 1..].trim();
    let values: Vec<f64> = data_str
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();

    Some(SensorData {
        port,
        mode,
        values,
    })
}

/// Check if a line indicates command completion.
pub fn parse_completion(line: &str) -> Option<(u8, &str)> {
    // "P0: ramp done" or "P1: pulse done"
    if !line.starts_with('P') {
        return None;
    }
    let port = line.as_bytes().get(1)?.checked_sub(b'0')? as u8;
    if line.contains("ramp done") {
        Some((port, "ramp"))
    } else if line.contains("pulse done") {
        Some((port, "pulse"))
    } else {
        None
    }
}

/// Check if line indicates initialization complete.
pub fn is_init_done(line: &str) -> bool {
    line.contains("Done initialising ports")
}

/// Check if line is a bootloader prompt.
pub fn is_bootloader_prompt(line: &str) -> bool {
    line.contains("BHBL>")
}

// ── Checksum ─────────────────────────────────────

/// Compute the Build HAT firmware checksum.
pub fn firmware_checksum(data: &[u8]) -> u32 {
    let mut u: u32 = 1;
    for &byte in data {
        if (u & 0x80000000) != 0 {
            u = (u << 1) ^ 0x1d872b41;
        } else {
            u = u << 1;
        }
        u = (u ^ byte as u32) & 0xFFFFFFFF;
    }
    u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_firmware() {
        let line = "Firmware version: 1.4.3";
        assert_eq!(
            parse_version(line),
            Some(HatState::Firmware("1.4.3".to_string()))
        );
    }

    #[test]
    fn test_parse_version_bootloader() {
        let line = "BuildHAT bootloader version 1.0";
        assert_eq!(parse_version(line), Some(HatState::Bootloader));
    }

    #[test]
    fn test_parse_device_active() {
        let line = "P0: connected to active ID2e";
        let dev = parse_device_line(line).unwrap();
        assert_eq!(dev.port, 0);
        assert_eq!(dev.type_id, 0x2e); // 46 = Large Motor
        assert!(dev.active);
    }

    #[test]
    fn test_parse_device_passive() {
        let line = "P1: connected to passive ID01";
        let dev = parse_device_line(line).unwrap();
        assert_eq!(dev.port, 1);
        assert_eq!(dev.type_id, 1);
        assert!(!dev.active);
    }

    #[test]
    fn test_parse_device_none() {
        assert!(parse_device_line("P2: no device detected").is_none());
        assert!(parse_device_line("P3: disconnected").is_none());
    }

    #[test]
    fn test_parse_sensor_data() {
        let line = "P0M1: 45 67.5 89";
        let data = parse_sensor_data(line).unwrap();
        assert_eq!(data.port, 0);
        assert_eq!(data.mode, 1);
        assert_eq!(data.values, vec![45.0, 67.5, 89.0]);
    }

    #[test]
    fn test_parse_sensor_data_combined() {
        let line = "P1C0: 10 20 30";
        let data = parse_sensor_data(line).unwrap();
        assert_eq!(data.port, 1);
        assert_eq!(data.mode, 0);
        assert_eq!(data.values, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn test_parse_completion() {
        assert_eq!(parse_completion("P0: ramp done"), Some((0, "ramp")));
        assert_eq!(parse_completion("P1: pulse done"), Some((1, "pulse")));
        assert!(parse_completion("P0: some other message").is_none());
    }

    #[test]
    fn test_checksum() {
        // Simple test: checksum of empty data should be 1 (initial value, no bytes)
        assert_eq!(firmware_checksum(&[]), 1);
        // Checksum of single byte
        let c = firmware_checksum(&[0x42]);
        assert_ne!(c, 0); // Just verify it produces something
    }

    #[test]
    fn test_motor_commands() {
        assert_eq!(cmd_motor_set(0, 50), "port 0 ; pwm ; set 0.5\r");
        assert_eq!(cmd_motor_set(1, -100), "port 1 ; pwm ; set -1\r");
        assert_eq!(cmd_motor_coast(1), "port 1 ; coast\r");
        assert_eq!(cmd_motor_off(2), "port 2 ; pwm ; set 0\r");
        assert!(cmd_motor_speed(0, 75).contains("pid"));
        assert!(cmd_motor_speed(0, 75).contains("set 75"));
    }

    #[test]
    fn test_sensor_commands() {
        assert_eq!(cmd_select_mode(0, 1, 100), "port 0 ; select 1 ; selrate 100\r");
        assert_eq!(cmd_deselect(3), "port 3 ; select\r");
    }
}
