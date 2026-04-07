use crate::constants::*;

#[derive(Debug, Clone)]
pub struct SensorSample {
    pub input_port: usize,
    pub raw_value: u16,
    pub state: u8,
    pub rotation_delta: i8,
}

#[derive(Debug, Clone)]
pub struct SensorNotification {
    pub samples: Vec<SensorSample>,
}

/// Encode an output power command.
/// Returns a byte sequence to send over serial.
/// power: -8 to 8 (0 = off, positive = left direction, negative = right direction)
pub fn encode_output_power(output_mask: u8, power: i8) -> Vec<u8> {
    let clamped = power.clamp(-8, 8);
    if clamped == 0 {
        return vec![ControlLabCommand::PowerOff as u8, output_mask];
    }
    let direction = if clamped < 0 {
        ControlLabCommand::DirectionRight as u8
    } else {
        ControlLabCommand::DirectionLeft as u8
    };
    let absolute_power = (clamped.unsigned_abs() - 1) as u8;
    vec![
        direction,
        output_mask,
        ControlLabCommand::PowerLevel as u8 | absolute_power,
        output_mask,
        ControlLabCommand::PowerOn as u8,
        output_mask,
    ]
}

/// Encode a keep-alive message.
pub fn encode_keep_alive() -> Vec<u8> {
    vec![0x02]
}

/// Verify a sensor message checksum.
fn verify_sensor_message(data: &[u8]) -> bool {
    let checksum: u16 = data.iter().map(|&b| b as u16).sum();
    (checksum & 0xff) == 0xff
}

/// Extract rotation delta from state byte.
fn extract_rotation_delta(state: u8) -> i8 {
    let mut change = (state & 3) as i8;
    if (state & 4) == 0 {
        change = -change;
    }
    change
}

/// Decode a 19-byte sensor notification message.
pub fn decode_sensor_message(data: &[u8]) -> Option<SensorNotification> {
    if data.len() < SENSOR_MESSAGE_LENGTH {
        return None;
    }
    if !verify_sensor_message(&data[..SENSOR_MESSAGE_LENGTH]) {
        return None;
    }

    let mut samples = Vec::new();
    for (sensor_idx, &offset) in SENSOR_MESSAGE_OFFSETS.iter().enumerate() {
        if offset + 1 >= data.len() {
            continue;
        }
        let word = &data[offset..offset + 2];
        let raw_value = ((word[0] as u16) << 2) | (((word[1] >> 6) & 0x03) as u16);
        let state = word[1] & 0x3f;
        samples.push(SensorSample {
            input_port: sensor_idx + 1,
            raw_value,
            state,
            rotation_delta: extract_rotation_delta(state),
        });
    }

    Some(SensorNotification { samples })
}

/// Get the output port bitmask for a port letter (A-H).
pub fn get_output_port_mask(port: &str) -> Option<u8> {
    let normalized = port.to_uppercase();
    OUTPUT_PORTS
        .iter()
        .position(|&p| p == normalized)
        .map(|i| 1u8 << i)
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
