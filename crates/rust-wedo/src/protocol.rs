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
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}

/// Normalize a power value (-100..100) to the motor range (-127..127).
pub fn normalize_power(power: i32) -> i8 {
    if power.abs() <= 100 {
        ((power as f64 / 100.0) * MAX_MOTOR_POWER as f64).round().clamp(-127.0, 127.0) as i8
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
mod tests {
    use super::*;

    #[test]
    fn test_encode_motor_command() {
        let cmd = encode_motor_command(0x00, 100, -50);
        assert_eq!(cmd[0], 0x00); // report ID
        assert_eq!(cmd[1], 0x00); // output bits
        assert_eq!(cmd[2], 100u8); // motor A
        assert_eq!(cmd[3], (-50i8) as u8); // motor B
    }

    #[test]
    fn test_encode_high_power() {
        let cmd = encode_motor_command(HUB_CTL_BIT_HIGH_POWER, 127, 0);
        assert_eq!(cmd[1], 0x40);
    }

    #[test]
    fn test_normalize_power() {
        assert_eq!(normalize_power(100), 127);
        assert_eq!(normalize_power(-100), -127);
        assert_eq!(normalize_power(0), 0);
        assert_eq!(normalize_power(50), 64); // round(50/100 * 127) = 64
    }

    #[test]
    fn test_decode_sensor_notification() {
        // Simulated 8-byte message: header, header, portA_val, portA_type, portB_val, portB_type, pad, pad
        let data = [0x00, 0x00, 150, 180, 100, 35, 0x00, 0x00];
        let notif = decode_sensor_notification(&data).unwrap();
        assert_eq!(notif.samples.len(), 2);
        assert_eq!(notif.samples[0].port, "A");
        assert_eq!(notif.samples[0].raw_value, 150);
        assert_eq!(notif.samples[0].sensor_type, SensorType::Distance); // type_id 180 -> Distance
        assert_eq!(notif.samples[1].port, "B");
        assert_eq!(notif.samples[1].raw_value, 100);
        assert_eq!(notif.samples[1].sensor_type, SensorType::Tilt); // type_id 35 -> Tilt
    }

    #[test]
    fn test_decode_short_message() {
        assert!(decode_sensor_notification(&[0x00, 0x00, 0x00]).is_none());
    }
}
