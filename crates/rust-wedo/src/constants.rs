pub const WEDO_VENDOR_ID: u16 = 0x0694;
pub const WEDO_PRODUCT_ID: u16 = 0x0003;

pub const SENSOR_MESSAGE_LENGTH: usize = 8;
pub const SENSOR_VALUE_OFFSETS: [usize; 2] = [2, 4];
pub const SENSOR_TYPE_OFFSETS: [usize; 2] = [3, 5];

pub const PORTS: [&str; 2] = ["A", "B"];

pub const MAX_MOTOR_POWER: i8 = 127;

/// Control bit for high power motor output (requires 500mA USB)
pub const HUB_CTL_BIT_HIGH_POWER: u8 = 0x40;

pub const DISTANCE_SENSOR_RAW_MIN: u8 = 71;
pub const DISTANCE_SENSOR_RAW_MAX: u8 = 219;
pub const DISTANCE_SENSOR_MAPPED_MAX: u8 = 100;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeDoState {
    NotReady = 0,
    Ready = 1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SensorType {
    Unknown,
    Tilt,
    Distance,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TiltEvent {
    Level = 0,
    Front = 1,
    Back = 2,
    Left = 3,
    Right = 4,
    Unknown = 5,
}

pub const fn get_sensor_type(type_id: u8) -> SensorType {
    // Ranges from the Linux WeDo driver
    if type_id <= 9 {
        return SensorType::Unknown;
    }
    if type_id <= 27 {
        return SensorType::Unknown;
    }
    if type_id <= 47 {
        return SensorType::Tilt;
    }
    if type_id <= 67 {
        return SensorType::Unknown;
    }
    if type_id <= 87 {
        return SensorType::Unknown;
    }
    if type_id <= 100 {
        return SensorType::Unknown;
    }
    if type_id <= 109 {
        return SensorType::Unknown;
    }
    if type_id <= 131 {
        return SensorType::Unknown;
    }
    if type_id <= 152 {
        return SensorType::Unknown;
    }
    if type_id <= 169 {
        return SensorType::Unknown;
    }
    if type_id <= 190 {
        return SensorType::Distance;
    }
    SensorType::Unknown
}

pub const fn get_tilt_event(raw_value: u8) -> TiltEvent {
    if raw_value == 0 {
        return TiltEvent::Unknown;
    }
    if raw_value <= 48 {
        return TiltEvent::Back;
    }
    if raw_value <= 99 {
        return TiltEvent::Right;
    }
    if raw_value <= 153 {
        return TiltEvent::Level;
    }
    if raw_value <= 204 {
        return TiltEvent::Front;
    }
    TiltEvent::Left
}

pub fn get_distance(raw_value: u8) -> u8 {
    let raw_min = DISTANCE_SENSOR_RAW_MIN as f64;
    let raw_max = DISTANCE_SENSOR_RAW_MAX as f64;
    let span = raw_max - raw_min;
    if span <= 0.0 {
        return 0;
    }
    let clamped = (raw_value as f64).clamp(raw_min, raw_max);
    let normalized = (clamped - raw_min) / span;
    (normalized * DISTANCE_SENSOR_MAPPED_MAX as f64).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensor_type_detection() {
        assert_eq!(get_sensor_type(0), SensorType::Unknown);
        assert_eq!(get_sensor_type(35), SensorType::Tilt);
        assert_eq!(get_sensor_type(180), SensorType::Distance);
        assert_eq!(get_sensor_type(250), SensorType::Unknown);
    }

    #[test]
    fn test_tilt_events() {
        assert_eq!(get_tilt_event(0), TiltEvent::Unknown);
        assert_eq!(get_tilt_event(25), TiltEvent::Back);
        assert_eq!(get_tilt_event(75), TiltEvent::Right);
        assert_eq!(get_tilt_event(120), TiltEvent::Level);
        assert_eq!(get_tilt_event(180), TiltEvent::Front);
        assert_eq!(get_tilt_event(230), TiltEvent::Left);
    }

    #[test]
    fn test_distance_conversion() {
        assert_eq!(get_distance(71), 0); // min
        assert_eq!(get_distance(219), 100); // max
        assert_eq!(get_distance(145), 50); // midpoint
        assert_eq!(get_distance(0), 0); // below min clamps
        assert_eq!(get_distance(255), 100); // above max clamps
    }
}
