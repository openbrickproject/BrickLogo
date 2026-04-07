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
