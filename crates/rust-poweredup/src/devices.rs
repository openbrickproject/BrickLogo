use crate::constants::DeviceType;
use std::collections::HashMap;

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
        DeviceType::MediumLinearMotor
        | DeviceType::MoveHubMediumLinearMotor
        | DeviceType::TechnicLargeLinearMotor
        | DeviceType::TechnicXLargeLinearMotor => vec![ModeMapEntry {
            event: "rotate",
            mode: 0x02,
        }],

        // ── Absolute Motors (rotation + absolute position) ─
        DeviceType::TechnicMediumAngularMotor
        | DeviceType::TechnicLargeAngularMotor
        | DeviceType::TechnicSmallAngularMotor
        | DeviceType::TechnicMediumAngularMotorGrey
        | DeviceType::TechnicLargeAngularMotorGrey => vec![
            ModeMapEntry {
                event: "rotate",
                mode: 0x02,
            },
            ModeMapEntry {
                event: "absolute",
                mode: 0x03,
            },
        ],

        // ── Technic Color Sensor ─────────────────
        DeviceType::TechnicColorSensor => vec![
            ModeMapEntry {
                event: "color",
                mode: 0x00,
            },
            ModeMapEntry {
                event: "reflect",
                mode: 0x01,
            },
            ModeMapEntry {
                event: "ambient",
                mode: 0x02,
            },
            ModeMapEntry {
                event: "rgbIntensity",
                mode: 0x05,
            },
            ModeMapEntry {
                event: "hsvIntensity",
                mode: 0x06,
            },
            ModeMapEntry {
                event: "hsvAmbient",
                mode: 0x07,
            },
        ],

        // ── Color Distance Sensor ────────────────
        DeviceType::ColorDistanceSensor => vec![
            ModeMapEntry {
                event: "color",
                mode: 0x00,
            },
            ModeMapEntry {
                event: "distance",
                mode: 0x01,
            },
            ModeMapEntry {
                event: "distanceCount",
                mode: 0x02,
            },
            ModeMapEntry {
                event: "reflect",
                mode: 0x03,
            },
            ModeMapEntry {
                event: "ambient",
                mode: 0x04,
            },
            ModeMapEntry {
                event: "rgbIntensity",
                mode: 0x06,
            },
            ModeMapEntry {
                event: "colorAndDistance",
                mode: 0x08,
            },
        ],

        // ── Technic Distance Sensor ──────────────
        DeviceType::TechnicDistanceSensor => vec![
            ModeMapEntry {
                event: "distance",
                mode: 0x00,
            },
            ModeMapEntry {
                event: "fastDistance",
                mode: 0x01,
            },
        ],

        // ── Technic Force Sensor ─────────────────
        DeviceType::TechnicForceSensor => vec![
            ModeMapEntry {
                event: "force",
                mode: 0x00,
            },
            ModeMapEntry {
                event: "touched",
                mode: 0x01,
            },
            ModeMapEntry {
                event: "tapped",
                mode: 0x02,
            },
        ],

        // ── Tilt Sensor (WeDo 2.0 / Hub) ────────
        DeviceType::TiltSensor => vec![
            ModeMapEntry {
                event: "tilt",
                mode: 0x00,
            },
            ModeMapEntry {
                event: "direction",
                mode: 0x01,
            },
            ModeMapEntry {
                event: "impactCount",
                mode: 0x02,
            },
            ModeMapEntry {
                event: "accel",
                mode: 0x03,
            },
        ],

        // ── Move Hub Tilt Sensor ─────────────────
        DeviceType::MoveHubTiltSensor => vec![ModeMapEntry {
            event: "tilt",
            mode: 0x00,
        }],

        // ── Technic Medium Hub Tilt Sensor ───────
        DeviceType::TechnicMediumHubTiltSensor => vec![
            ModeMapEntry {
                event: "tilt",
                mode: 0x00,
            },
            ModeMapEntry {
                event: "impactCount",
                mode: 0x01,
            },
        ],

        // ── Technic Medium Hub Accelerometer ─────
        DeviceType::TechnicMediumHubAccelerometer => vec![ModeMapEntry {
            event: "accel",
            mode: 0x00,
        }],

        // ── Technic Medium Hub Gyro ──────────────
        DeviceType::TechnicMediumHubGyroSensor => vec![ModeMapEntry {
            event: "gyro",
            mode: 0x00,
        }],

        // ── Motion Sensor ────────────────────────
        DeviceType::MotionSensor => vec![ModeMapEntry {
            event: "distance",
            mode: 0x00,
        }],

        // ── Remote Control Button ────────────────
        DeviceType::RemoteControlButton => vec![ModeMapEntry {
            event: "remoteButton",
            mode: 0x00,
        }],

        // ── Voltage Sensor ───────────────────────
        DeviceType::VoltageSensor => vec![ModeMapEntry {
            event: "voltage",
            mode: 0x00,
        }],

        // ── Current Sensor ───────────────────────
        DeviceType::CurrentSensor => vec![ModeMapEntry {
            event: "current",
            mode: 0x00,
        }],

        // ── Duplo Train Base Color Sensor ────────
        DeviceType::DuploTrainBaseColorSensor => vec![
            ModeMapEntry {
                event: "intensity",
                mode: 0x00,
            },
            ModeMapEntry {
                event: "color",
                mode: 0x01,
            },
            ModeMapEntry {
                event: "reflect",
                mode: 0x02,
            },
            ModeMapEntry {
                event: "rgb",
                mode: 0x03,
            },
        ],

        // ── Duplo Train Base Speedometer ─────────
        DeviceType::DuploTrainBaseSpeedometer => vec![ModeMapEntry {
            event: "speed",
            mode: 0x00,
        }],

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
    mode_map_for_device(device_type).first().map(|e| e.event)
}

/// Parse sensor data from a PORT_VALUE_SINGLE payload for a specific device and mode.
/// `data` is the raw bytes AFTER the port ID (offset 4+ in LWP3, 2+ in WeDo2).
pub fn parse_sensor_data(
    device_type: DeviceType,
    mode: u8,
    data: &[u8],
    is_wedo2: bool,
) -> Option<SensorReading> {
    match device_type {
        // ── Tacho/Absolute Motors ────────────────
        DeviceType::MediumLinearMotor
        | DeviceType::MoveHubMediumLinearMotor
        | DeviceType::TechnicLargeLinearMotor
        | DeviceType::TechnicXLargeLinearMotor
        | DeviceType::TechnicMediumAngularMotor
        | DeviceType::TechnicLargeAngularMotor
        | DeviceType::TechnicSmallAngularMotor
        | DeviceType::TechnicMediumAngularMotorGrey
        | DeviceType::TechnicLargeAngularMotorGrey => {
            match mode {
                0x02 => {
                    // rotate
                    if data.len() >= 4 {
                        let degrees = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        Some(SensorReading::Number(degrees as f64))
                    } else {
                        None
                    }
                }
                0x03 => {
                    // absolute
                    if data.len() >= 2 {
                        let angle = i16::from_le_bytes([data[0], data[1]]);
                        Some(SensorReading::Number(angle as f64))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Technic Color Sensor ─────────────────
        DeviceType::TechnicColorSensor => {
            match mode {
                0x00 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(255) as f64
                )), // color
                0x01 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(0) as f64
                )), // reflect
                0x02 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(0) as f64
                )), // ambient
                0x05 => {
                    // rgbIntensity
                    if data.len() >= 8 {
                        let r = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let g = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let b = u16::from_le_bytes([data[4], data[5]]) as f64;
                        let i = u16::from_le_bytes([data[6], data[7]]) as f64;
                        Some(SensorReading::Quad(r, g, b, i))
                    } else {
                        None
                    }
                }
                0x06 => {
                    // hsvIntensity
                    if data.len() >= 6 {
                        let h = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let s = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let v = u16::from_le_bytes([data[4], data[5]]) as f64;
                        Some(SensorReading::Triple(h, s, v))
                    } else {
                        None
                    }
                }
                0x07 => {
                    // hsvAmbient (SHSV)
                    if data.len() >= 8 {
                        let h = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let s = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let v = u16::from_le_bytes([data[4], data[5]]) as f64;
                        let i = u16::from_le_bytes([data[6], data[7]]) as f64;
                        Some(SensorReading::Quad(h, s, v, i))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Color Distance Sensor ────────────────
        DeviceType::ColorDistanceSensor => {
            match mode {
                0x00 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(255) as f64
                )), // color
                0x01 => {
                    // distance
                    let raw = data.first().copied().unwrap_or(0) as f64;
                    Some(SensorReading::Number(raw * 25.4 - 20.0))
                }
                0x02 => {
                    // distanceCount
                    if data.len() >= 4 {
                        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        Some(SensorReading::Number(count as f64))
                    } else {
                        None
                    }
                }
                0x03 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(0) as f64
                )), // reflect
                0x04 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(0) as f64
                )), // ambient
                0x06 => {
                    // rgbIntensity
                    if data.len() >= 6 {
                        let r = u16::from_le_bytes([data[0], data[1]]) as f64;
                        let g = u16::from_le_bytes([data[2], data[3]]) as f64;
                        let b = u16::from_le_bytes([data[4], data[5]]) as f64;
                        Some(SensorReading::Triple(r, g, b))
                    } else {
                        None
                    }
                }
                0x08 => {
                    // colorAndDistance
                    if data.len() >= 4 {
                        let color = data[0] as f64;
                        let mut distance = data[1] as f64;
                        if data.len() > 3 {
                            let partial = data[3] as f64;
                            distance += partial / 10.0;
                        }
                        Some(SensorReading::Pair(color, distance))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Technic Distance Sensor ──────────────
        DeviceType::TechnicDistanceSensor => {
            match mode {
                0x00 | 0x01 => {
                    // distance / fastDistance
                    if data.len() >= 2 {
                        let d = u16::from_le_bytes([data[0], data[1]]);
                        Some(SensorReading::Number(d as f64))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Technic Force Sensor ─────────────────
        DeviceType::TechnicForceSensor => {
            match mode {
                0x00 => {
                    // force (0-100, divided by 10 for Newtons)
                    let raw = data.first().copied().unwrap_or(0) as f64;
                    Some(SensorReading::Number(raw / 10.0))
                }
                0x01 => {
                    // touched
                    Some(SensorReading::Bool(data.first().copied().unwrap_or(0) != 0))
                }
                0x02 => {
                    // tapped
                    Some(SensorReading::Number(
                        data.first().copied().unwrap_or(0) as f64
                    ))
                }
                _ => None,
            }
        }

        // ── Tilt Sensor (WeDo 2.0 / Hub) ────────
        DeviceType::TiltSensor => {
            match mode {
                0x00 => {
                    // tilt
                    if data.len() >= 2 {
                        Some(SensorReading::Pair(
                            data[0] as i8 as f64,
                            data[1] as i8 as f64,
                        ))
                    } else {
                        None
                    }
                }
                0x01 => {
                    // direction
                    Some(SensorReading::Number(
                        data.first().copied().unwrap_or(0) as i8 as f64,
                    ))
                }
                0x02 => {
                    // impactCount (crash)
                    if data.len() >= 3 {
                        Some(SensorReading::Triple(
                            data[0] as f64,
                            data[1] as f64,
                            data[2] as f64,
                        ))
                    } else {
                        None
                    }
                }
                0x03 => {
                    // accel (cal)
                    if data.len() >= 3 {
                        let factor = 1000.0 / (45.0 * std::f64::consts::SQRT_2);
                        let x = data[0] as i8 as f64 * factor;
                        let y = data[1] as i8 as f64 * factor;
                        let z = data[2] as i8 as f64 * factor;
                        Some(SensorReading::Triple(x.round(), y.round(), z.round()))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Move Hub Tilt Sensor ─────────────────
        DeviceType::MoveHubTiltSensor => {
            match mode {
                0x00 => {
                    // tilt (x is negated)
                    if data.len() >= 2 {
                        Some(SensorReading::Pair(
                            -(data[0] as i8 as f64),
                            data[1] as i8 as f64,
                        ))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Technic Medium Hub Tilt Sensor ───────
        DeviceType::TechnicMediumHubTiltSensor => {
            match mode {
                0x00 => {
                    // tilt (z negated at offset 0, y at 2, x at 4)
                    if data.len() >= 6 {
                        let z = -(i16::from_le_bytes([data[0], data[1]]) as f64);
                        let y = i16::from_le_bytes([data[2], data[3]]) as f64;
                        let x = i16::from_le_bytes([data[4], data[5]]) as f64;
                        Some(SensorReading::Triple(x, y, z))
                    } else {
                        None
                    }
                }
                0x01 => {
                    // impactCount
                    if data.len() >= 4 {
                        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                        Some(SensorReading::Number(count as f64))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Technic Medium Hub Accelerometer ─────
        DeviceType::TechnicMediumHubAccelerometer => {
            match mode {
                0x00 => {
                    // accel (mG, divided by 4.096)
                    if data.len() >= 6 {
                        let x = (i16::from_le_bytes([data[0], data[1]]) as f64 / 4.096).round();
                        let y = (i16::from_le_bytes([data[2], data[3]]) as f64 / 4.096).round();
                        let z = (i16::from_le_bytes([data[4], data[5]]) as f64 / 4.096).round();
                        Some(SensorReading::Triple(x, y, z))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Technic Medium Hub Gyro ──────────────
        DeviceType::TechnicMediumHubGyroSensor => {
            match mode {
                0x00 => {
                    // gyro (DPS, scaled by 7/400)
                    if data.len() >= 6 {
                        let x =
                            (i16::from_le_bytes([data[0], data[1]]) as f64 * 7.0 / 400.0).round();
                        let y =
                            (i16::from_le_bytes([data[2], data[3]]) as f64 * 7.0 / 400.0).round();
                        let z =
                            (i16::from_le_bytes([data[4], data[5]]) as f64 * 7.0 / 400.0).round();
                        Some(SensorReading::Triple(x, y, z))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Motion Sensor ────────────────────────
        DeviceType::MotionSensor => {
            match mode {
                0x00 => {
                    // distance
                    if data.len() >= 2 {
                        let mut d = data[0] as f64;
                        if data[1] == 1 {
                            d += 255.0;
                        }
                        Some(SensorReading::Number(d * 10.0))
                    } else if data.len() >= 1 {
                        Some(SensorReading::Number(data[0] as f64 * 10.0))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Remote Control Button ────────────────
        DeviceType::RemoteControlButton => {
            match mode {
                0x00 => {
                    // remoteButton
                    Some(SensorReading::Number(
                        data.first().copied().unwrap_or(0) as f64
                    ))
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
                        } else {
                            None
                        }
                    } else {
                        if data.len() >= 2 {
                            let raw = u16::from_le_bytes([data[0], data[1]]);
                            // max voltage raw = 3893, max voltage value = 9.615
                            Some(SensorReading::Number(raw as f64 * 9.615 / 3893.0))
                        } else {
                            None
                        }
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
                        } else {
                            None
                        }
                    } else {
                        if data.len() >= 2 {
                            let raw = u16::from_le_bytes([data[0], data[1]]);
                            // max current raw = 4095, max current value = 2444
                            Some(SensorReading::Number(raw as f64 * 2444.0 / 4095.0))
                        } else {
                            None
                        }
                    }
                }
                _ => None,
            }
        }

        // ── Duplo Train Base Color Sensor ────────
        DeviceType::DuploTrainBaseColorSensor => {
            match mode {
                0x00 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(0) as f64
                )), // intensity
                0x01 => {
                    // color
                    let c = data.first().copied().unwrap_or(255);
                    if c <= 10 {
                        Some(SensorReading::Number(c as f64))
                    } else {
                        Some(SensorReading::Number(255.0)) // NONE
                    }
                }
                0x02 => Some(SensorReading::Number(
                    data.first().copied().unwrap_or(0) as f64
                )), // reflect
                0x03 => {
                    // rgb
                    if data.len() >= 6 {
                        let r = u16::from_le_bytes([data[0], data[1]]) as f64 / 4.0;
                        let g = u16::from_le_bytes([data[2], data[3]]) as f64 / 4.0;
                        let b = u16::from_le_bytes([data[4], data[5]]) as f64 / 4.0;
                        Some(SensorReading::Triple(r, g, b))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // ── Duplo Train Base Speedometer ─────────
        DeviceType::DuploTrainBaseSpeedometer => match mode {
            0x00 => {
                if data.len() >= 2 {
                    let speed = i16::from_le_bytes([data[0], data[1]]);
                    Some(SensorReading::Number(speed as f64))
                } else {
                    None
                }
            }
            _ => None,
        },

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
#[path = "tests/devices.rs"]
mod tests;
