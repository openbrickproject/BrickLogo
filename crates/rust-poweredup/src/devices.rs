use std::collections::HashMap;
use crate::constants::DeviceType;

/// A sensor reading value from a device.
#[derive(Debug, Clone, PartialEq)]
pub enum SensorReading {
    /// Single numeric value (color ID, distance, degrees, etc.)
    Number(f64),
    /// Boolean value (touched, pressed)
    Bool(bool),
    /// Two values (tilt x/y, etc.)
    Pair(f64, f64),
    /// Three values (RGB, gyro, accel, tilt xyz, etc.)
    Triple(f64, f64, f64),
    /// Four values (RGBI, HSVI, etc.)
    Quad(f64, f64, f64, f64),
}

/// Mode map entry: event name → mode number.
#[derive(Debug, Clone)]
pub struct ModeMapEntry {
    pub event: &'static str,
    pub mode: u8,
}

/// Get the mode map for a given device type.
/// Returns a list of (event_name, mode_number) pairs.
pub fn mode_map_for_device(device_type: DeviceType) -> Vec<ModeMapEntry> {
    match device_type {
        // ── Tacho Motors (rotation feedback) ─────
        DeviceType::MediumLinearMotor |
        DeviceType::MoveHubMediumLinearMotor |
        DeviceType::TechnicLargeLinearMotor |
        DeviceType::TechnicXLargeLinearMotor => vec![
            ModeMapEntry { event: "rotate", mode: 0x02 },
        ],

        // ── Absolute Motors (rotation + absolute position) ─
        DeviceType::TechnicMediumAngularMotor |
        DeviceType::TechnicLargeAngularMotor |
        DeviceType::TechnicSmallAngularMotor |
        DeviceType::TechnicMediumAngularMotorGrey |
        DeviceType::TechnicLargeAngularMotorGrey => vec![
            ModeMapEntry { event: "rotate", mode: 0x02 },
            ModeMapEntry { event: "absolute", mode: 0x03 },
        ],

        // ── Technic Color Sensor ─────────────────
        DeviceType::TechnicColorSensor => vec![
            ModeMapEntry { event: "color", mode: 0x00 },
            ModeMapEntry { event: "reflect", mode: 0x01 },
            ModeMapEntry { event: "ambient", mode: 0x02 },
            ModeMapEntry { event: "rgbIntensity", mode: 0x05 },
            ModeMapEntry { event: "hsvIntensity", mode: 0x06 },
            ModeMapEntry { event: "hsvAmbient", mode: 0x07 },
        ],

        // ── Color Distance Sensor ────────────────
        DeviceType::ColorDistanceSensor => vec![
            ModeMapEntry { event: "color", mode: 0x00 },
            ModeMapEntry { event: "distance", mode: 0x01 },
            ModeMapEntry { event: "distanceCount", mode: 0x02 },
            ModeMapEntry { event: "reflect", mode: 0x03 },
            ModeMapEntry { event: "ambient", mode: 0x04 },
            ModeMapEntry { event: "rgbIntensity", mode: 0x06 },
            ModeMapEntry { event: "colorAndDistance", mode: 0x08 },
        ],

        // ── Technic Distance Sensor ──────────────
        DeviceType::TechnicDistanceSensor => vec![
            ModeMapEntry { event: "distance", mode: 0x00 },
            ModeMapEntry { event: "fastDistance", mode: 0x01 },
        ],

        // ── Technic Force Sensor ─────────────────
        DeviceType::TechnicForceSensor => vec![
            ModeMapEntry { event: "force", mode: 0x00 },
            ModeMapEntry { event: "touched", mode: 0x01 },
            ModeMapEntry { event: "tapped", mode: 0x02 },
        ],

        // ── Tilt Sensor (WeDo 2.0 / Hub) ────────
        DeviceType::TiltSensor => vec![
            ModeMapEntry { event: "tilt", mode: 0x00 },
            ModeMapEntry { event: "direction", mode: 0x01 },
            ModeMapEntry { event: "impactCount", mode: 0x02 },
            ModeMapEntry { event: "accel", mode: 0x03 },
        ],

        // ── Move Hub Tilt Sensor ─────────────────
        DeviceType::MoveHubTiltSensor => vec![
            ModeMapEntry { event: "tilt", mode: 0x00 },
        ],

        // ── Technic Medium Hub Tilt Sensor ───────
        DeviceType::TechnicMediumHubTiltSensor => vec![
            ModeMapEntry { event: "tilt", mode: 0x00 },
            ModeMapEntry { event: "impactCount", mode: 0x01 },
        ],

        // ── Technic Medium Hub Accelerometer ─────
        DeviceType::TechnicMediumHubAccelerometer => vec![
            ModeMapEntry { event: "accel", mode: 0x00 },
        ],

        // ── Technic Medium Hub Gyro ──────────────
        DeviceType::TechnicMediumHubGyroSensor => vec![
            ModeMapEntry { event: "gyro", mode: 0x00 },
        ],

        // ── Motion Sensor ────────────────────────
        DeviceType::MotionSensor => vec![
            ModeMapEntry { event: "distance", mode: 0x00 },
        ],

        // ── Remote Control Button ────────────────
        DeviceType::RemoteControlButton => vec![
            ModeMapEntry { event: "remoteButton", mode: 0x00 },
        ],

        // ── Voltage Sensor ───────────────────────
        DeviceType::VoltageSensor => vec![
            ModeMapEntry { event: "voltage", mode: 0x00 },
        ],

        // ── Current Sensor ───────────────────────
        DeviceType::CurrentSensor => vec![
            ModeMapEntry { event: "current", mode: 0x00 },
        ],

        // ── Duplo Train Base Color Sensor ────────
        DeviceType::DuploTrainBaseColorSensor => vec![
            ModeMapEntry { event: "intensity", mode: 0x00 },
            ModeMapEntry { event: "color", mode: 0x01 },
            ModeMapEntry { event: "reflect", mode: 0x02 },
            ModeMapEntry { event: "rgb", mode: 0x03 },
        ],

        // ── Duplo Train Base Speedometer ─────────
        DeviceType::DuploTrainBaseSpeedometer => vec![
            ModeMapEntry { event: "speed", mode: 0x00 },
        ],

        // ── Devices with no sensor modes ─────────
        _ => vec![],
    }
}

/// Look up a mode number by event name for a device type.
pub fn mode_for_event(device_type: DeviceType, event: &str) -> Option<u8> {
    mode_map_for_device(device_type)
        .iter()
        .find(|e| e.event == event)
        .map(|e| e.mode)
}

/// Get the default event name for a device type (first in mode map).
pub fn default_event(device_type: DeviceType) -> Option<&'static str> {
    mode_map_for_device(device_type)
        .first()
        .map(|e| e.event)
}

/// Parse sensor data from a PORT_VALUE_SINGLE payload for a specific device and mode.
/// `data` is the raw bytes AFTER the port ID (offset 4+ in LWP3, 2+ in WeDo2).
pub fn parse_sensor_data(device_type: DeviceType, mode: u8, data: &[u8], is_wedo2: bool) -> Option<SensorReading> {
    match device_type {
        // ── Tacho/Absolute Motors ────────────────
        DeviceType::MediumLinearMotor |
        DeviceType::MoveHubMediumLinearMotor |
        DeviceType::TechnicLargeLinearMotor |
        DeviceType::TechnicXLargeLinearMotor |
        DeviceType::TechnicMediumAngularMotor |
        DeviceType::TechnicLargeAngularMotor |
        DeviceType::TechnicSmallAngularMotor |
        DeviceType::TechnicMediumAngularMotorGrey |
        DeviceType::TechnicLargeAngularMotorGrey => {
            match mode {
                0x02 => { // rotate
                    if data.len() >= 4 {
                        let degrees = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        Some(SensorReading::Number(degrees as f64))
                    } else { None }
                }
                0x03 => { // absolute
                    if data.len() >= 2 {
                        let angle = i16::from_le_bytes([data[0], data[1]]);
                        Some(SensorReading::Number(angle as f64))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Technic Color Sensor ─────────────────
        DeviceType::TechnicColorSensor => {
            match mode {
                0x00 => Some(SensorReading::Number(data.first().copied().unwrap_or(255) as f64)), // color
                0x01 => Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64)), // reflect
                0x02 => Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64)), // ambient
                0x05 => { // rgbIntensity
                    if data.len() >= 8 {
                        let r = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let g = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let b = u16::from_le_bytes([data[4], data[5]]) as f64;
                        let i = u16::from_le_bytes([data[6], data[7]]) as f64;
                        Some(SensorReading::Quad(r, g, b, i))
                    } else { None }
                }
                0x06 => { // hsvIntensity
                    if data.len() >= 6 {
                        let h = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let s = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let v = u16::from_le_bytes([data[4], data[5]]) as f64;
                        Some(SensorReading::Triple(h, s, v))
                    } else { None }
                }
                0x07 => { // hsvAmbient (SHSV)
                    if data.len() >= 8 {
                        let h = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let s = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let v = u16::from_le_bytes([data[4], data[5]]) as f64;
                        let i = u16::from_le_bytes([data[6], data[7]]) as f64;
                        Some(SensorReading::Quad(h, s, v, i))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Color Distance Sensor ────────────────
        DeviceType::ColorDistanceSensor => {
            match mode {
                0x00 => Some(SensorReading::Number(data.first().copied().unwrap_or(255) as f64)), // color
                0x01 => { // distance
                    let raw = data.first().copied().unwrap_or(0) as f64;
                    Some(SensorReading::Number(raw * 25.4 - 20.0))
                }
                0x02 => { // distanceCount
                    if data.len() >= 4 {
                        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        Some(SensorReading::Number(count as f64))
                    } else { None }
                }
                0x03 => Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64)), // reflect
                0x04 => Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64)), // ambient
                0x06 => { // rgbIntensity
                    if data.len() >= 6 {
                        let r = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let g = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let b = u16::from_le_bytes([data[4], data[5]]) as f64;
                        Some(SensorReading::Triple(r, g, b))
                    } else { None }
                }
                0x08 => { // colorAndDistance
                    if data.len() >= 4 {
                        let color = data[0] as f64;
                        let mut distance = data[1] as f64;
                        if data.len() > 3 {
                            let partial = data[3] as f64;
                            distance += partial / 10.0;
                        }
                        Some(SensorReading::Pair(color, distance))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Technic Distance Sensor ──────────────
        DeviceType::TechnicDistanceSensor => {
            match mode {
                0x00 | 0x01 => { // distance / fastDistance
                    if data.len() >= 2 {
                        let d = u16::from_le_bytes([data[0], data[1]]);
                        Some(SensorReading::Number(d as f64))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Technic Force Sensor ─────────────────
        DeviceType::TechnicForceSensor => {
            match mode {
                0x00 => { // force (0-100, divided by 10 for Newtons)
                    let raw = data.first().copied().unwrap_or(0) as f64;
                    Some(SensorReading::Number(raw / 10.0))
                }
                0x01 => { // touched
                    Some(SensorReading::Bool(data.first().copied().unwrap_or(0) != 0))
                }
                0x02 => { // tapped
                    Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64))
                }
                _ => None,
            }
        }

        // ── Tilt Sensor (WeDo 2.0 / Hub) ────────
        DeviceType::TiltSensor => {
            match mode {
                0x00 => { // tilt
                    if data.len() >= 2 {
                        Some(SensorReading::Pair(data[0] as i8 as f64, data[1] as i8 as f64))
                    } else { None }
                }
                0x01 => { // direction
                    Some(SensorReading::Number(data.first().copied().unwrap_or(0) as i8 as f64))
                }
                0x02 => { // impactCount (crash)
                    if data.len() >= 3 {
                        Some(SensorReading::Triple(data[0] as f64, data[1] as f64, data[2] as f64))
                    } else { None }
                }
                0x03 => { // accel (cal)
                    if data.len() >= 3 {
                        let factor = 1000.0 / (45.0 * std::f64::consts::SQRT_2);
                        let x = data[0] as i8 as f64 * factor;
                        let y = data[1] as i8 as f64 * factor;
                        let z = data[2] as i8 as f64 * factor;
                        Some(SensorReading::Triple(x.round(), y.round(), z.round()))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Move Hub Tilt Sensor ─────────────────
        DeviceType::MoveHubTiltSensor => {
            match mode {
                0x00 => { // tilt (x is negated)
                    if data.len() >= 2 {
                        Some(SensorReading::Pair(-(data[0] as i8 as f64), data[1] as i8 as f64))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Technic Medium Hub Tilt Sensor ───────
        DeviceType::TechnicMediumHubTiltSensor => {
            match mode {
                0x00 => { // tilt (z negated at offset 0, y at 2, x at 4)
                    if data.len() >= 6 {
                        let z = -(i16::from_le_bytes([data[0], data[1]]) as f64);
                        let y = i16::from_le_bytes([data[2], data[3]]) as f64;
                        let x = i16::from_le_bytes([data[4], data[5]]) as f64;
                        Some(SensorReading::Triple(x, y, z))
                    } else { None }
                }
                0x01 => { // impactCount
                    if data.len() >= 4 {
                        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        Some(SensorReading::Number(count as f64))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Technic Medium Hub Accelerometer ─────
        DeviceType::TechnicMediumHubAccelerometer => {
            match mode {
                0x00 => { // accel (mG, divided by 4.096)
                    if data.len() >= 6 {
                        let x = (i16::from_le_bytes([data[0], data[1]]) as f64 / 4.096).round();
                        let y = (i16::from_le_bytes([data[2], data[3]]) as f64 / 4.096).round();
                        let z = (i16::from_le_bytes([data[4], data[5]]) as f64 / 4.096).round();
                        Some(SensorReading::Triple(x, y, z))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Technic Medium Hub Gyro ──────────────
        DeviceType::TechnicMediumHubGyroSensor => {
            match mode {
                0x00 => { // gyro (DPS, scaled by 7/400)
                    if data.len() >= 6 {
                        let x = (i16::from_le_bytes([data[0], data[1]]) as f64 * 7.0 / 400.0).round();
                        let y = (i16::from_le_bytes([data[2], data[3]]) as f64 * 7.0 / 400.0).round();
                        let z = (i16::from_le_bytes([data[4], data[5]]) as f64 * 7.0 / 400.0).round();
                        Some(SensorReading::Triple(x, y, z))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Motion Sensor ────────────────────────
        DeviceType::MotionSensor => {
            match mode {
                0x00 => { // distance
                    if data.len() >= 2 {
                        let mut d = data[0] as f64;
                        if data[1] == 1 { d += 255.0; }
                        Some(SensorReading::Number(d * 10.0))
                    } else if data.len() >= 1 {
                        Some(SensorReading::Number(data[0] as f64 * 10.0))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Remote Control Button ────────────────
        DeviceType::RemoteControlButton => {
            match mode {
                0x00 => { // remoteButton
                    Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64))
                }
                _ => None,
            }
        }

        // ── Voltage Sensor ───────────────────────
        DeviceType::VoltageSensor => {
            match mode {
                0x00 => {
                    if is_wedo2 {
                        if data.len() >= 2 {
                            let raw = i16::from_le_bytes([data[0], data[1]]);
                            Some(SensorReading::Number(raw as f64 / 40.0))
                        } else { None }
                    } else {
                        if data.len() >= 2 {
                            let raw = u16::from_le_bytes([data[0], data[1]]);
                            // max voltage raw = 3893, max voltage value = 9.615
                            Some(SensorReading::Number(raw as f64 * 9.615 / 3893.0))
                        } else { None }
                    }
                }
                _ => None,
            }
        }

        // ── Current Sensor ───────────────────────
        DeviceType::CurrentSensor => {
            match mode {
                0x00 => {
                    if is_wedo2 {
                        if data.len() >= 2 {
                            let raw = i16::from_le_bytes([data[0], data[1]]);
                            Some(SensorReading::Number(raw as f64 / 1000.0))
                        } else { None }
                    } else {
                        if data.len() >= 2 {
                            let raw = u16::from_le_bytes([data[0], data[1]]);
                            // max current raw = 4095, max current value = 2444
                            Some(SensorReading::Number(raw as f64 * 2444.0 / 4095.0))
                        } else { None }
                    }
                }
                _ => None,
            }
        }

        // ── Duplo Train Base Color Sensor ────────
        DeviceType::DuploTrainBaseColorSensor => {
            match mode {
                0x00 => Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64)), // intensity
                0x01 => { // color
                    let c = data.first().copied().unwrap_or(255);
                    if c <= 10 {
                        Some(SensorReading::Number(c as f64))
                    } else {
                        Some(SensorReading::Number(255.0)) // NONE
                    }
                }
                0x02 => Some(SensorReading::Number(data.first().copied().unwrap_or(0) as f64)), // reflect
                0x03 => { // rgb
                    if data.len() >= 6 {
                        let r = u16::from_le_bytes([data[0], data[1]]) as f64 / 4.0;
                        let g = u16::from_le_bytes([data[2], data[3]]) as f64 / 4.0;
                        let b = u16::from_le_bytes([data[4], data[5]]) as f64 / 4.0;
                        Some(SensorReading::Triple(r, g, b))
                    } else { None }
                }
                _ => None,
            }
        }

        // ── Duplo Train Base Speedometer ─────────
        DeviceType::DuploTrainBaseSpeedometer => {
            match mode {
                0x00 => {
                    if data.len() >= 2 {
                        let speed = i16::from_le_bytes([data[0], data[1]]);
                        Some(SensorReading::Number(speed as f64))
                    } else { None }
                }
                _ => None,
            }
        }

        _ => None,
    }
}

/// Build a map of event name → mode number for quick lookups.
pub fn build_mode_lookup(device_type: DeviceType) -> HashMap<String, u8> {
    mode_map_for_device(device_type)
        .into_iter()
        .map(|e| (e.event.to_string(), e.mode))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_map_technic_color_sensor() {
        let map = mode_map_for_device(DeviceType::TechnicColorSensor);
        assert_eq!(map.len(), 6);
        assert_eq!(map[0].event, "color");
        assert_eq!(map[0].mode, 0x00);
        assert_eq!(map[1].event, "reflect");
        assert_eq!(map[1].mode, 0x01);
    }

    #[test]
    fn test_mode_for_event() {
        assert_eq!(mode_for_event(DeviceType::TechnicColorSensor, "color"), Some(0x00));
        assert_eq!(mode_for_event(DeviceType::TechnicColorSensor, "reflect"), Some(0x01));
        assert_eq!(mode_for_event(DeviceType::TechnicColorSensor, "nonexistent"), None);
    }

    #[test]
    fn test_default_event() {
        assert_eq!(default_event(DeviceType::TechnicColorSensor), Some("color"));
        assert_eq!(default_event(DeviceType::TechnicForceSensor), Some("force"));
        assert_eq!(default_event(DeviceType::TrainMotor), None);
    }

    #[test]
    fn test_mode_map_absolute_motor() {
        let map = mode_map_for_device(DeviceType::TechnicMediumAngularMotor);
        assert_eq!(map.len(), 2);
        assert_eq!(map[0].event, "rotate");
        assert_eq!(map[1].event, "absolute");
    }

    #[test]
    fn test_mode_map_tacho_motor() {
        let map = mode_map_for_device(DeviceType::MediumLinearMotor);
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].event, "rotate");
    }

    #[test]
    fn test_parse_motor_rotation() {
        // 360 degrees = 0x68010000 LE
        let data = 360_i32.to_le_bytes();
        let reading = parse_sensor_data(DeviceType::TechnicMediumAngularMotor, 0x02, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(360.0)));
    }

    #[test]
    fn test_parse_motor_rotation_negative() {
        let data = (-180_i32).to_le_bytes();
        let reading = parse_sensor_data(DeviceType::MediumLinearMotor, 0x02, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(-180.0)));
    }

    #[test]
    fn test_parse_motor_absolute() {
        let data = (-90_i16).to_le_bytes();
        let reading = parse_sensor_data(DeviceType::TechnicMediumAngularMotor, 0x03, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(-90.0)));
    }

    #[test]
    fn test_parse_technic_color_sensor_color() {
        let data = [3]; // blue
        let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(3.0)));
    }

    #[test]
    fn test_parse_technic_color_sensor_reflect() {
        let data = [75];
        let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x01, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(75.0)));
    }

    #[test]
    fn test_parse_technic_color_sensor_rgb() {
        // r=100, g=200, b=300, i=400
        let data = [100, 0, 200, 0, 0x2C, 0x01, 0x90, 0x01];
        let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x05, &data, false);
        assert_eq!(reading, Some(SensorReading::Quad(100.0, 200.0, 300.0, 400.0)));
    }

    #[test]
    fn test_parse_technic_color_sensor_hsv() {
        let data = [180, 0, 100, 0, 50, 0]; // h=180, s=100, v=50
        let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x06, &data, false);
        assert_eq!(reading, Some(SensorReading::Triple(180.0, 100.0, 50.0)));
    }

    #[test]
    fn test_parse_color_distance_color() {
        let data = [9]; // red
        let reading = parse_sensor_data(DeviceType::ColorDistanceSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(9.0)));
    }

    #[test]
    fn test_parse_color_distance_distance() {
        let data = [4]; // raw = 4, distance = 4*25.4-20 = 81.6
        let reading = parse_sensor_data(DeviceType::ColorDistanceSensor, 0x01, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(4.0 * 25.4 - 20.0)));
    }

    #[test]
    fn test_parse_color_distance_rgb() {
        let data = [0xFF, 0x00, 0x80, 0x00, 0x40, 0x00]; // r=255, g=128, b=64
        let reading = parse_sensor_data(DeviceType::ColorDistanceSensor, 0x06, &data, false);
        assert_eq!(reading, Some(SensorReading::Triple(255.0, 128.0, 64.0)));
    }

    #[test]
    fn test_parse_technic_distance() {
        let data = [0xE8, 0x03]; // 1000mm
        let reading = parse_sensor_data(DeviceType::TechnicDistanceSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(1000.0)));
    }

    #[test]
    fn test_parse_technic_force() {
        let data = [50]; // 50/10 = 5.0N
        let reading = parse_sensor_data(DeviceType::TechnicForceSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(5.0)));
    }

    #[test]
    fn test_parse_technic_force_touched() {
        let data = [1]; // touched
        let reading = parse_sensor_data(DeviceType::TechnicForceSensor, 0x01, &data, false);
        assert_eq!(reading, Some(SensorReading::Bool(true)));

        let data = [0]; // not touched
        let reading = parse_sensor_data(DeviceType::TechnicForceSensor, 0x01, &data, false);
        assert_eq!(reading, Some(SensorReading::Bool(false)));
    }

    #[test]
    fn test_parse_tilt_sensor() {
        let data = [10_i8 as u8, 245_u8]; // x=10, y=-11
        let reading = parse_sensor_data(DeviceType::TiltSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Pair(10.0, -11.0)));
    }

    #[test]
    fn test_parse_move_hub_tilt() {
        let data = [10_i8 as u8, 20]; // x negated → -10, y=20
        let reading = parse_sensor_data(DeviceType::MoveHubTiltSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Pair(-10.0, 20.0)));
    }

    #[test]
    fn test_parse_technic_tilt() {
        // z=100 (negated→-100), y=200, x=300 as i16 LE
        let data = [100, 0, 200, 0, 0x2C, 0x01];
        let reading = parse_sensor_data(DeviceType::TechnicMediumHubTiltSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Triple(300.0, 200.0, -100.0)));
    }

    #[test]
    fn test_parse_technic_accel() {
        // x=4096, y=-4096, z=0
        let data = [0x00, 0x10, 0x00, 0xF0, 0x00, 0x00];
        let reading = parse_sensor_data(DeviceType::TechnicMediumHubAccelerometer, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Triple(1000.0, -1000.0, 0.0)));
    }

    #[test]
    fn test_parse_technic_gyro() {
        // raw 400 → 400 * 7/400 = 7.0
        let data = [0x90, 0x01, 0x00, 0x00, 0x00, 0x00]; // 400 LE
        let reading = parse_sensor_data(DeviceType::TechnicMediumHubGyroSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Triple(7.0, 0.0, 0.0)));
    }

    #[test]
    fn test_parse_motion_sensor() {
        let data = [5, 0]; // distance = 5 * 10 = 50
        let reading = parse_sensor_data(DeviceType::MotionSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(50.0)));
    }

    #[test]
    fn test_parse_motion_sensor_extended() {
        let data = [5, 1]; // flag = 1: distance = (5+255) * 10 = 2600
        let reading = parse_sensor_data(DeviceType::MotionSensor, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(2600.0)));
    }

    #[test]
    fn test_parse_remote_button() {
        let data = [1]; // UP
        let reading = parse_sensor_data(DeviceType::RemoteControlButton, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(1.0)));
    }

    #[test]
    fn test_parse_voltage_lpf2() {
        let data = [0x35, 0x0F]; // 3893 raw → 9.615V
        let reading = parse_sensor_data(DeviceType::VoltageSensor, 0x00, &data, false);
        let expected = 3893.0 * 9.615 / 3893.0;
        assert_eq!(reading, Some(SensorReading::Number(expected)));
    }

    #[test]
    fn test_parse_voltage_wedo2() {
        let data = [0xA0, 0x0F]; // 4000 → 4000/40 = 100.0
        let reading = parse_sensor_data(DeviceType::VoltageSensor, 0x00, &data, true);
        assert_eq!(reading, Some(SensorReading::Number(100.0)));
    }

    #[test]
    fn test_parse_duplo_color_sensor() {
        let data = [6]; // green
        let reading = parse_sensor_data(DeviceType::DuploTrainBaseColorSensor, 0x01, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(6.0)));
    }

    #[test]
    fn test_parse_duplo_color_sensor_invalid() {
        let data = [50]; // > 10, should be NONE (255)
        let reading = parse_sensor_data(DeviceType::DuploTrainBaseColorSensor, 0x01, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(255.0)));
    }

    #[test]
    fn test_parse_duplo_speedometer() {
        let data = [0x20, 0x00]; // speed = 32
        let reading = parse_sensor_data(DeviceType::DuploTrainBaseSpeedometer, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(32.0)));
    }

    #[test]
    fn test_parse_duplo_speedometer_negative() {
        let data = (-15_i16).to_le_bytes();
        let reading = parse_sensor_data(DeviceType::DuploTrainBaseSpeedometer, 0x00, &data, false);
        assert_eq!(reading, Some(SensorReading::Number(-15.0)));
    }

    #[test]
    fn test_build_mode_lookup() {
        let lookup = build_mode_lookup(DeviceType::TechnicForceSensor);
        assert_eq!(lookup.get("force"), Some(&0x00));
        assert_eq!(lookup.get("touched"), Some(&0x01));
        assert_eq!(lookup.get("tapped"), Some(&0x02));
        assert_eq!(lookup.len(), 3);
    }

    #[test]
    fn test_unknown_device_has_no_modes() {
        let map = mode_map_for_device(DeviceType::Unknown);
        assert!(map.is_empty());
    }

    #[test]
    fn test_basic_motor_has_no_modes() {
        let map = mode_map_for_device(DeviceType::TrainMotor);
        assert!(map.is_empty());
    }
}
