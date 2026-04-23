use super::*;

#[test]
fn test_process_touch() {
    let mut sensor_types = [SensorType::Unknown; INPUT_PORT_COUNT];
    sensor_types[0] = SensorType::Touch;
    let mut rotation_values = [0i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    let mut msg = vec![0x00u8; SENSOR_MESSAGE_LENGTH];
    let offset = SENSOR_MESSAGE_OFFSETS[0];
    msg[offset] = 125;
    msg[offset + 1] = 0;
    let sum: u16 = msg.iter().map(|&b| b as u16).sum();
    let needed = (0xFF - (sum & 0xFF)) & 0xFF;
    msg[SENSOR_MESSAGE_LENGTH - 1] = msg[SENSOR_MESSAGE_LENGTH - 1].wrapping_add(needed as u8);

    let mut buffer = msg;
    process_sensor_data(
        &mut buffer,
        &sensor_types,
        &mut rotation_values,
        &mut payloads,
    );

    let payload = payloads.get("touch:1");
    assert!(payload.is_some());
    if let Some(ControlLabSensorPayload::Touch(t)) = payload {
        assert!(t.pressed);
    }
}

#[test]
fn test_rotation_accumulates() {
    let mut rotation_values = [0i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    for _ in 0..5 {
        let notification = SensorNotification {
            samples: vec![SensorSample {
                input_port: 1,
                raw_value: 512,
                state: 5,
                rotation_delta: 1,
            }],
        };
        for sample in &notification.samples {
            let idx = sample.input_port - 1;
            rotation_values[idx] += sample.rotation_delta as i32;
            let rotations = rotation_values[idx];
            payloads.insert(
                format!("rotation:{}", sample.input_port),
                ControlLabSensorPayload::Rotation(RotationSensorPayload {
                    input_port: sample.input_port,
                    raw_value: sample.raw_value,
                    rotations,
                    delta: sample.rotation_delta,
                }),
            );
        }
    }

    if let Some(ControlLabSensorPayload::Rotation(r)) = payloads.get("rotation:1") {
        assert_eq!(r.rotations, 5);
    } else {
        panic!("Expected rotation");
    }
}

/// Build a well-formed sensor frame with a single sample on port 1.
///
/// The wire encoding packs a 10-bit raw reading plus a 6-bit state nibble
/// into two bytes: byte0 is the high 8 bits of raw, byte1's top 2 bits
/// are raw's low 2 bits, and byte1's bottom 6 bits carry the state.
fn make_frame(port_index_1based: usize, raw_value: u16, state: u8) -> Vec<u8> {
    let mut msg = vec![0x00u8; SENSOR_MESSAGE_LENGTH];
    let offset = SENSOR_MESSAGE_OFFSETS[port_index_1based - 1];
    let raw_10 = raw_value & 0x3FF;
    msg[offset] = (raw_10 >> 2) as u8;
    msg[offset + 1] = (((raw_10 & 0x03) << 6) | (state as u16 & 0x3F)) as u8;
    // Pad the checksum byte so the frame validates.
    let sum: u16 = msg.iter().map(|&b| b as u16).sum();
    let needed = (0xFF - (sum & 0xFF)) & 0xFF;
    msg[SENSOR_MESSAGE_LENGTH - 1] = msg[SENSOR_MESSAGE_LENGTH - 1].wrapping_add(needed as u8);
    msg
}

#[test]
fn test_touch_released_when_raw_above_threshold() {
    let mut sensor_types = [SensorType::Unknown; INPUT_PORT_COUNT];
    sensor_types[0] = SensorType::Touch;
    let mut rotation = [0i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    // raw > 1000 → touch released. Cap at 1023 because raw is 10-bit.
    let mut buffer = make_frame(1, 1023, 0);
    process_sensor_data(&mut buffer, &sensor_types, &mut rotation, &mut payloads);

    match payloads.get("touch:1").unwrap() {
        ControlLabSensorPayload::Touch(t) => {
            assert!(!t.pressed, "raw > 1000 should be treated as released");
        }
        _ => panic!("expected Touch"),
    }
}

#[test]
fn test_light_sensor_intensity_mapping() {
    let mut sensor_types = [SensorType::Unknown; INPUT_PORT_COUNT];
    sensor_types[0] = SensorType::Light;
    let mut rotation = [0i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    // raw=1000 → intensity = floor(146 - 1000/7) ≈ 146 - 142 = 4.
    let mut buffer = make_frame(1, 1000, 0);
    process_sensor_data(&mut buffer, &sensor_types, &mut rotation, &mut payloads);

    match payloads.get("light:1").unwrap() {
        ControlLabSensorPayload::Light(l) => {
            assert!(l.intensity <= 10, "expected low intensity for raw=1000, got {}", l.intensity);
            assert_eq!(l.raw_value, 1000);
        }
        _ => panic!("expected Light"),
    }
}

#[test]
fn test_temperature_celsius_and_fahrenheit() {
    let mut sensor_types = [SensorType::Unknown; INPUT_PORT_COUNT];
    sensor_types[0] = SensorType::Temperature;
    let mut rotation = [0i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    // raw=760 is the zero point by formula — celsius ≈ 0, fahrenheit ≈ 32.
    let mut buffer = make_frame(1, 760, 0);
    process_sensor_data(&mut buffer, &sensor_types, &mut rotation, &mut payloads);

    match payloads.get("temperature:1").unwrap() {
        ControlLabSensorPayload::Temperature(t) => {
            assert!(
                t.celsius.abs() < 1.0,
                "celsius near 0 expected, got {}",
                t.celsius
            );
            assert!(
                (t.fahrenheit - 32.0).abs() < 1.0,
                "fahrenheit near 32 expected, got {}",
                t.fahrenheit
            );
        }
        _ => panic!("expected Temperature"),
    }
}

#[test]
fn test_misaligned_buffer_drops_leading_garbage() {
    // If the buffer starts mid-frame (non-zero leading byte), the
    // processor should skip bytes one at a time looking for the 0x00
    // frame marker, not drop the whole buffer.
    let sensor_types = [SensorType::Unknown; INPUT_PORT_COUNT];
    let mut rotation = [0i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    let mut buffer = vec![0xFF, 0xFF, 0xFE]; // garbage, no frame yet
    process_sensor_data(&mut buffer, &sensor_types, &mut rotation, &mut payloads);
    // Three bytes isn't enough for a full frame; processor returns
    // without progress and leaves no payloads.
    assert!(payloads.is_empty());
}

#[test]
fn test_invalid_checksum_clears_buffer() {
    // A frame whose checksum doesn't validate should cause the processor
    // to bail out and clear the buffer rather than emit garbage.
    let sensor_types = [SensorType::Unknown; INPUT_PORT_COUNT];
    let mut rotation = [0i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    let mut buffer = vec![0x00u8; SENSOR_MESSAGE_LENGTH];
    // Deliberately break the checksum by setting a non-zero byte.
    buffer[SENSOR_MESSAGE_OFFSETS[0]] = 123;
    // Do NOT fix the checksum.
    process_sensor_data(&mut buffer, &sensor_types, &mut rotation, &mut payloads);
    assert!(payloads.is_empty(), "bad checksum should produce no payloads");
    assert!(buffer.is_empty(), "bad frame should clear the buffer");
}

#[test]
fn test_rotation_delta_accumulates_negative() {
    let mut sensor_types = [SensorType::Unknown; INPUT_PORT_COUNT];
    sensor_types[0] = SensorType::Rotation;
    let mut rotation = [10i32; INPUT_PORT_COUNT];
    let mut payloads = HashMap::new();

    // State encoding: bits 0..=1 are magnitude, bit 2 is direction (set
    // = positive). `state = 0b000001 = 1` decodes to delta = -1.
    let mut buffer = make_frame(1, 0, 1);
    process_sensor_data(&mut buffer, &sensor_types, &mut rotation, &mut payloads);

    // Was 10, delta -1 → 9.
    assert_eq!(rotation[0], 9);
}
