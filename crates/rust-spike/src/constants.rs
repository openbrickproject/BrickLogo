/// Number of ports on the SPIKE Prime / Robot Inventor large hub.
pub const PORT_COUNT: usize = 6;

// ── LPF2 device type IDs ───────────────────────
// Same IDs as Build HAT / Powered UP — these are the same physical devices.
pub const DEVICE_PASSIVE_MOTOR: u16 = 1;
pub const DEVICE_TRAIN_MOTOR: u16 = 2;
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

// ── Stop modes ─────────────────────────────────
pub const STOP_COAST: u8 = 0;
pub const STOP_BRAKE: u8 = 1;
pub const STOP_HOLD: u8 = 2;

// ── Default acceleration / deceleration ────────
pub const DEFAULT_ACCEL: u16 = 100;
pub const DEFAULT_DECEL: u16 = 100;

pub fn is_motor(type_id: u16) -> bool {
    matches!(
        type_id,
        DEVICE_PASSIVE_MOTOR
            | DEVICE_TRAIN_MOTOR
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

pub fn is_absolute_motor(type_id: u16) -> bool {
    matches!(
        type_id,
        DEVICE_MEDIUM_ANGULAR_MOTOR
            | DEVICE_LARGE_ANGULAR_MOTOR
            | DEVICE_SMALL_ANGULAR_MOTOR
            | DEVICE_MEDIUM_ANGULAR_MOTOR_GREY
            | DEVICE_LARGE_ANGULAR_MOTOR_GREY
    )
}

pub fn port_index(letter: &str) -> Option<usize> {
    match letter.to_lowercase().as_str() {
        "a" => Some(0),
        "b" => Some(1),
        "c" => Some(2),
        "d" => Some(3),
        "e" => Some(4),
        "f" => Some(5),
        _ => None,
    }
}

pub fn port_letter(index: usize) -> &'static str {
    match index {
        0 => "a",
        1 => "b",
        2 => "c",
        3 => "d",
        4 => "e",
        5 => "f",
        _ => "?",
    }
}

#[cfg(test)]
#[path = "tests/constants.rs"]
mod tests;
