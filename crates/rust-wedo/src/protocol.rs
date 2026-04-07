use crate::constants::*;

#[derive(Debug, Clone)]
pub struct SensorSample {
    pub port: String,
    pub raw_value: u8,
    pub sensor_type_id: u8,
    pub sensor_type: SensorType,
}

#[derive(Debug, Clone)]
pub struct SensorNotification {
    pub samples: Vec<SensorSample>,
}

/// Encode a motor command as a 9-byte HID report.
pub fn encode_motor_command(output_bits: u8, motor_a: i8, motor_b: i8) -> [u8; 9] {
    [
        0x00, // HID report ID
        output_bits,
        motor_a as u8,
        motor_b as u8,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
    ]
}

/// Normalize a power value (-100..100) to the motor range (-127..127).
pub fn normalize_power(power: i32) -> i8 {
    if power.abs() <= 100 {
        ((power as f64 / 100.0) * MAX_MOTOR_POWER as f64)
            .round()
            .clamp(-127.0, 127.0) as i8
    } else {
        power.clamp(-127, 127) as i8
    }
}

/// Decode an 8-byte sensor notification message.
pub fn decode_sensor_notification(data: &[u8]) -> Option<SensorNotification> {
    if data.len() < SENSOR_MESSAGE_LENGTH {
        return None;
    }

    let mut samples = Vec::new();
    for i in 0..PORTS.len() {
        let raw_value = data[SENSOR_VALUE_OFFSETS[i]];
        let sensor_type_id = data[SENSOR_TYPE_OFFSETS[i]];
        samples.push(SensorSample {
            port: PORTS[i].to_string(),
            raw_value,
            sensor_type_id,
            sensor_type: get_sensor_type(sensor_type_id),
        });
    }

    Some(SensorNotification { samples })
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
