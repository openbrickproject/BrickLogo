// ── Serial settings ──────────────────────────────
pub const DEFAULT_BAUD_RATE: u32 = 115200;
pub const DEFAULT_SERIAL_PATH: &str = "/dev/serial0";

// ── Firmware upload markers ──────────────────────
pub const STX: u8 = 0x02;
pub const ETX: u8 = 0x03;

// ── Port count ───────────────────────────────────
pub const PORT_COUNT: usize = 4;

// ── Device type IDs (LPF2) ──────────────────────
// Same IDs as Powered UP — these are the same physical devices.
pub const DEVICE_PASSIVE_MOTOR: u16 = 1;
pub const DEVICE_LIGHT: u16 = 8;
pub const DEVICE_TILT_SENSOR: u16 = 34;
pub const DEVICE_MOTION_SENSOR: u16 = 35;
pub const DEVICE_COLOR_DISTANCE_SENSOR: u16 = 37;
pub const DEVICE_MEDIUM_LINEAR_MOTOR: u16 = 38;
pub const DEVICE_LARGE_MOTOR: u16 = 46;
pub const DEVICE_XL_MOTOR: u16 = 47;
pub const DEVICE_MEDIUM_ANGULAR_MOTOR: u16 = 48;
pub const DEVICE_LARGE_ANGULAR_MOTOR: u16 = 49;
pub const DEVICE_COLOR_SENSOR: u16 = 61;
pub const DEVICE_DISTANCE_SENSOR: u16 = 62;
pub const DEVICE_FORCE_SENSOR: u16 = 63;
pub const DEVICE_MATRIX: u16 = 64;
pub const DEVICE_SMALL_ANGULAR_MOTOR: u16 = 65;
pub const DEVICE_MEDIUM_ANGULAR_MOTOR_GREY: u16 = 75;
pub const DEVICE_LARGE_ANGULAR_MOTOR_GREY: u16 = 76;

/// Returns true if this device type is a motor.
pub fn is_motor(type_id: u16) -> bool {
    matches!(
        type_id,
        DEVICE_PASSIVE_MOTOR
            | DEVICE_MEDIUM_LINEAR_MOTOR
            | DEVICE_LARGE_MOTOR
            | DEVICE_XL_MOTOR
            | DEVICE_MEDIUM_ANGULAR_MOTOR
            | DEVICE_LARGE_ANGULAR_MOTOR
            | DEVICE_SMALL_ANGULAR_MOTOR
            | DEVICE_MEDIUM_ANGULAR_MOTOR_GREY
            | DEVICE_LARGE_ANGULAR_MOTOR_GREY
    )
}

/// Returns true if this device type is a sensor.
pub fn is_sensor(type_id: u16) -> bool {
    matches!(
        type_id,
        DEVICE_TILT_SENSOR
            | DEVICE_MOTION_SENSOR
            | DEVICE_COLOR_DISTANCE_SENSOR
            | DEVICE_COLOR_SENSOR
            | DEVICE_DISTANCE_SENSOR
            | DEVICE_FORCE_SENSOR
    )
}

/// Returns true if this device type has encoder feedback.
pub fn is_tacho_motor(type_id: u16) -> bool {
    matches!(
        type_id,
        DEVICE_MEDIUM_LINEAR_MOTOR
            | DEVICE_LARGE_MOTOR
            | DEVICE_XL_MOTOR
            | DEVICE_MEDIUM_ANGULAR_MOTOR
            | DEVICE_LARGE_ANGULAR_MOTOR
            | DEVICE_SMALL_ANGULAR_MOTOR
            | DEVICE_MEDIUM_ANGULAR_MOTOR_GREY
            | DEVICE_LARGE_ANGULAR_MOTOR_GREY
    )
}

/// Map port index (0-3) to port letter (a-d).
pub fn port_letter(index: usize) -> &'static str {
    match index {
        0 => "a",
        1 => "b",
        2 => "c",
        3 => "d",
        _ => "?",
    }
}

/// Map port letter to index (0-3).
pub fn port_index(letter: &str) -> Option<usize> {
    match letter.to_lowercase().as_str() {
        "a" => Some(0),
        "b" => Some(1),
        "c" => Some(2),
        "d" => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_motor() {
        assert!(is_motor(DEVICE_MEDIUM_ANGULAR_MOTOR));
        assert!(is_motor(DEVICE_PASSIVE_MOTOR));
        assert!(!is_motor(DEVICE_COLOR_SENSOR));
    }

    #[test]
    fn test_is_sensor() {
        assert!(is_sensor(DEVICE_COLOR_SENSOR));
        assert!(is_sensor(DEVICE_FORCE_SENSOR));
        assert!(!is_sensor(DEVICE_LARGE_MOTOR));
    }

    #[test]
    fn test_port_mapping() {
        assert_eq!(port_letter(0), "a");
        assert_eq!(port_letter(3), "d");
        assert_eq!(port_index("A"), Some(0));
        assert_eq!(port_index("d"), Some(3));
        assert_eq!(port_index("e"), None);
    }
}
