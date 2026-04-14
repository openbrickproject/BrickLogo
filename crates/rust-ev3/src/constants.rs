//! Sensor type and mode tables for EV3 and NXT-compat sensors.
//!
//! Values come from LEGO's EV3 Firmware Developer Kit and the
//! `mindboards/ev3sources` lms2012 source. Mode indices are passed to
//! `opINPUT_DEVICE` subcommand `READY_*`; the sensor type gates which
//! mode indices are meaningful.

// ── Sensor type IDs ──────────────────────────────

pub const TYPE_NXT_TOUCH:      u8 = 1;
pub const TYPE_NXT_LIGHT:      u8 = 2;
pub const TYPE_NXT_SOUND:      u8 = 3;
pub const TYPE_NXT_COLOR:      u8 = 4;
pub const TYPE_NXT_ULTRASONIC: u8 = 5;
pub const TYPE_NXT_TEMPERATURE:u8 = 6;
pub const TYPE_EV3_TOUCH:      u8 = 16;
pub const TYPE_EV3_COLOR:      u8 = 29;
pub const TYPE_EV3_ULTRASONIC: u8 = 30;
pub const TYPE_EV3_GYRO:       u8 = 32;
pub const TYPE_EV3_INFRARED:   u8 = 33;

/// Kind of reply value an `opINPUT_DEVICE READY_*` call returns. Dictates
/// whether the adapter uses `READY_PCT` (→ u8) or `READY_SI` (→ f32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorKind {
    /// Integer in 0..=100, returned as u8. Works for color IDs, reflected
    /// light percent, ambient light percent, touch (0/1), etc.
    Pct,
    /// Physical SI-unit value, returned as f32. Distance cm, angle deg,
    /// gyro rate deg/s.
    Si,
}

/// Look up the EV3 type + mode + reply kind for a BrickLogo sensor-mode
/// name on a given sensor type. Returns `None` if the name isn't supported
/// on that sensor.
///
/// The name universe mirrors the naming used by other BrickLogo adapters
/// (Powered UP, Control Lab, WeDo) where a physical analog exists.
pub fn lookup_mode(sensor_type: u8, mode_name: &str) -> Option<(u8, SensorKind)> {
    match sensor_type {
        TYPE_EV3_COLOR => match mode_name {
            "light" | "reflect" => Some((0, SensorKind::Pct)),    // COL-REFLECT
            "ambient"           => Some((1, SensorKind::Pct)),    // COL-AMBIENT
            "color"             => Some((2, SensorKind::Pct)),    // COL-COLOR (0..7)
            "rgb"               => Some((4, SensorKind::Si)),     // RGB-RAW (returns 3× uint16 scaled; we surface as SI float)
            _ => None,
        },
        TYPE_EV3_TOUCH => match mode_name {
            "touch" => Some((0, SensorKind::Pct)),                // TOUCH (0/1)
            _ => None,
        },
        TYPE_EV3_ULTRASONIC => match mode_name {
            "distance" => Some((0, SensorKind::Si)),              // US-DIST-CM
            _ => None,
        },
        TYPE_EV3_GYRO => match mode_name {
            "angle" => Some((0, SensorKind::Si)),                 // GYRO-ANG
            "rate"  => Some((1, SensorKind::Si)),                 // GYRO-RATE
            _ => None,
        },
        TYPE_EV3_INFRARED => match mode_name {
            "distance" => Some((0, SensorKind::Si)),              // IR-PROX
            "seek"     => Some((1, SensorKind::Si)),              // IR-SEEK
            "remote"   => Some((2, SensorKind::Si)),              // IR-REMOTE
            _ => None,
        },
        TYPE_NXT_TOUCH => match mode_name {
            "touch" => Some((0, SensorKind::Pct)),
            _ => None,
        },
        TYPE_NXT_LIGHT => match mode_name {
            "light"   => Some((0, SensorKind::Pct)),              // LIGHT-REFLECT
            "ambient" => Some((1, SensorKind::Pct)),              // LIGHT-AMBIENT
            _ => None,
        },
        TYPE_NXT_SOUND => match mode_name {
            "sound" => Some((0, SensorKind::Pct)),                // SOUND-DB
            _ => None,
        },
        TYPE_NXT_ULTRASONIC => match mode_name {
            "distance" => Some((0, SensorKind::Si)),              // US-DIST-CM
            _ => None,
        },
        TYPE_NXT_TEMPERATURE => match mode_name {
            "temperature" => Some((0, SensorKind::Si)),
            _ => None,
        },
        _ => None,
    }
}

/// Return `true` if the sensor-type ID refers to a known EV3 or NXT
/// sensor we support. Used by the adapter's `validate_sensor_port`.
pub fn is_known_sensor(sensor_type: u8) -> bool {
    matches!(
        sensor_type,
        TYPE_NXT_TOUCH
            | TYPE_NXT_LIGHT
            | TYPE_NXT_SOUND
            | TYPE_NXT_COLOR
            | TYPE_NXT_ULTRASONIC
            | TYPE_NXT_TEMPERATURE
            | TYPE_EV3_TOUCH
            | TYPE_EV3_COLOR
            | TYPE_EV3_ULTRASONIC
            | TYPE_EV3_GYRO
            | TYPE_EV3_INFRARED
    )
}
