//! NXT sensor-type and sensor-mode enums, and the user-facing mode-name
//! lookup table. These match the `SET_INPUT_MODE` opcode's byte values
//! from the NXT Bluetooth Developer Kit, Appendix 2.

// ── Sensor types (SetInputMode byte 1) ───────────

pub const NXT_NO_SENSOR:      u8 = 0x00;
pub const NXT_SWITCH:         u8 = 0x01; // Touch sensor
pub const NXT_TEMPERATURE:    u8 = 0x02; // legacy RCX
pub const NXT_REFLECTION:     u8 = 0x03; // legacy RCX light
pub const NXT_ANGLE:          u8 = 0x04; // legacy RCX rotation
pub const NXT_LIGHT_ACTIVE:   u8 = 0x05; // 9844 light sensor, LED on
pub const NXT_LIGHT_INACTIVE: u8 = 0x06; // 9844 light sensor, LED off
pub const NXT_SOUND_DB:       u8 = 0x07;
pub const NXT_SOUND_DBA:      u8 = 0x08; // A-weighted
pub const NXT_CUSTOM:         u8 = 0x09;
pub const NXT_LOWSPEED:       u8 = 0x0A; // I2C, no +9V
pub const NXT_LOWSPEED_9V:    u8 = 0x0B; // I2C, +9V (ultrasonic)

// ── Sensor modes (SetInputMode byte 2) ───────────

pub const MODE_RAW:             u8 = 0x00;
pub const MODE_BOOLEAN:         u8 = 0x20;
pub const MODE_TRANSITION_CNT:  u8 = 0x40;
pub const MODE_PERIOD_COUNTER:  u8 = 0x60;
pub const MODE_PCT_FULL_SCALE:  u8 = 0x80;
pub const MODE_CELSIUS:         u8 = 0xA0;
pub const MODE_FAHRENHEIT:      u8 = 0xC0;
pub const MODE_ANGLE_STEPS:     u8 = 0xE0;

/// How the adapter should post-process the raw reply before returning a
/// `LogoValue` to the language layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorKind {
    /// Reported as 0 / 1; adapter returns the `scaled` field.
    Bool,
    /// 0..100; adapter returns the `scaled` field.
    Pct,
    /// 0..1023 ADC; adapter returns the `raw_ad` field.
    Raw,
}

/// Translate a BrickLogo user-facing mode name into the wire values the
/// NXT `SET_INPUT_MODE` opcode expects, plus the post-processing the
/// adapter should apply when reading the sensor.
///
/// Returns `None` for unknown modes so the adapter can surface a clean
/// error instead of a wire-level failure.
pub fn lookup_mode(mode_name: &str) -> Option<(u8, u8, SensorKind)> {
    match mode_name {
        "touch"          => Some((NXT_SWITCH,       MODE_BOOLEAN,        SensorKind::Bool)),
        "light"
        | "light_active" => Some((NXT_LIGHT_ACTIVE, MODE_PCT_FULL_SCALE, SensorKind::Pct)),
        "light_inactive"
        | "ambient"      => Some((NXT_LIGHT_INACTIVE, MODE_PCT_FULL_SCALE, SensorKind::Pct)),
        "sound"
        | "sound_dba"    => Some((NXT_SOUND_DBA,    MODE_PCT_FULL_SCALE, SensorKind::Pct)),
        "sound_db"       => Some((NXT_SOUND_DB,     MODE_PCT_FULL_SCALE, SensorKind::Pct)),
        "pct"            => Some((NXT_CUSTOM,       MODE_PCT_FULL_SCALE, SensorKind::Pct)),
        "raw"            => Some((NXT_CUSTOM,       MODE_RAW,            SensorKind::Raw)),
        _ => None,
    }
}
