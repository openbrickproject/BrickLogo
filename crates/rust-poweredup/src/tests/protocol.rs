use super::*;

#[test]
fn test_frame_message() {
    let msg = frame_message(&[0x81, 0x00, 0x11, 0x51, 0x00, 50]);
    assert_eq!(msg[0], 8);
    assert_eq!(msg[1], 0x00);
    assert_eq!(msg[2], 0x81);
    assert_eq!(msg[7], 50);
}

#[test]
fn test_extract_messages_single() {
    let buf = vec![5, 0x00, 0x45, 0x00, 0x10];
    let (msgs, remaining) = extract_messages(&buf);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0], vec![5, 0x00, 0x45, 0x00, 0x10]);
    assert!(remaining.is_empty());
}

#[test]
fn test_extract_messages_multiple() {
    let buf = vec![5, 0x00, 0x45, 0x00, 0x10, 5, 0x00, 0x45, 0x01, 0x20];
    let (msgs, remaining) = extract_messages(&buf);
    assert_eq!(msgs.len(), 2);
    assert!(remaining.is_empty());
}

#[test]
fn test_extract_messages_partial() {
    let buf = vec![5, 0x00, 0x45, 0x00, 0x10, 5, 0x00];
    let (msgs, remaining) = extract_messages(&buf);
    assert_eq!(msgs.len(), 1);
    assert_eq!(remaining, vec![5, 0x00]);
}

#[test]
fn test_message_type() {
    let msg = vec![5, 0x00, 0x45, 0x00, 0x10];
    assert_eq!(message_type(&msg), Some(MessageType::PortValueSingle));
}

#[test]
fn test_parse_attached_io_attached() {
    let msg = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let event = parse_attached_io(&msg).unwrap();
    assert_eq!(
        event,
        AttachedIoEvent::Attached {
            port_id: 0,
            device_type: DeviceType::TechnicColorSensor,
        }
    );
}

#[test]
fn test_parse_attached_io_detached() {
    let msg = vec![5, 0x00, 0x04, 0x02, 0x00];
    let event = parse_attached_io(&msg).unwrap();
    assert_eq!(event, AttachedIoEvent::Detached { port_id: 2 });
}

#[test]
fn test_parse_attached_io_virtual() {
    let msg = vec![9, 0x00, 0x04, 0x10, 0x02, 0x26, 0x00, 0x00, 0x01];
    let event = parse_attached_io(&msg).unwrap();
    assert_eq!(
        event,
        AttachedIoEvent::AttachedVirtual {
            port_id: 0x10,
            device_type: DeviceType::MediumLinearMotor,
            first_port: 0x00,
            second_port: 0x01,
        }
    );
}

#[test]
fn test_parse_port_value() {
    let msg = vec![7, 0x00, 0x45, 0x02, 0xe8, 0x03, 0x00];
    let (port_id, data) = parse_port_value(&msg).unwrap();
    assert_eq!(port_id, 0x02);
    assert_eq!(data, &[0xe8, 0x03, 0x00]);
}

#[test]
fn test_parse_port_feedback() {
    let msg = vec![7, 0x00, 0x82, 0x00, 0x0a, 0x01, 0x02];
    let feedbacks = parse_port_feedback(&msg);
    assert_eq!(feedbacks.len(), 2);
    assert_eq!(feedbacks[0].port_id, 0x00);
    assert_eq!(feedbacks[0].feedback, 0x0a);
    assert!(feedbacks[0].is_completed());
    assert_eq!(feedbacks[1].port_id, 0x01);
    assert_eq!(feedbacks[1].feedback, 0x02);
    assert!(feedbacks[1].is_completed());
}

#[test]
fn test_cmd_set_power() {
    let msg = cmd_set_power(0x00, 50, false);
    assert_eq!(msg[0], 8);
    assert_eq!(msg[2], 0x81);
    assert_eq!(msg[3], 0x00);
    assert_eq!(msg[4], 0x01);
    assert_eq!(msg[5], 0x51);
    assert_eq!(msg[6], 0x00);
    assert_eq!(msg[7], 50);
}

#[test]
fn test_cmd_set_power_interrupt() {
    let msg = cmd_set_power(0x01, -75, true);
    assert_eq!(msg[4], 0x11);
    assert_eq!(msg[7], (-75_i8) as u8);
}

#[test]
fn test_cmd_set_power_clamp() {
    let msg = cmd_set_power(0x00, 127, false);
    assert_eq!(msg[7], 100);
}

#[test]
fn test_cmd_start_speed() {
    let msg = cmd_start_speed(0x00, 75, 100, false);
    assert_eq!(msg[2], 0x81);
    assert_eq!(msg[5], SUBCMD_START_SPEED);
    assert_eq!(msg[6], 75);
    assert_eq!(msg[7], 100);
}

#[test]
fn test_cmd_start_speed_for_time() {
    let msg = cmd_start_speed_for_time(0x00, 1000, 50, 100, BrakingStyle::Hold, false);
    assert_eq!(msg[5], SUBCMD_START_SPEED_FOR_TIME);
    assert_eq!(msg[6], 0xE8);
    assert_eq!(msg[7], 0x03);
    assert_eq!(msg[8], 50);
    assert_eq!(msg[9], 100);
    assert_eq!(msg[10], BrakingStyle::Hold as u8);
}

#[test]
fn test_cmd_start_speed_for_degrees() {
    let msg = cmd_start_speed_for_degrees(0x01, 360, 80, 100, BrakingStyle::Brake, true);
    assert_eq!(msg[4], 0x11);
    assert_eq!(msg[5], SUBCMD_START_SPEED_FOR_DEGREES);
    assert_eq!(msg[6], 0x68);
    assert_eq!(msg[7], 0x01);
    assert_eq!(msg[8], 0x00);
    assert_eq!(msg[9], 0x00);
    assert_eq!(msg[10], 80);
    assert_eq!(msg[11], 100);
    assert_eq!(msg[12], BrakingStyle::Brake as u8);
}

#[test]
fn test_cmd_goto_absolute() {
    let msg = cmd_goto_absolute(0x00, -180, 50, 100, BrakingStyle::Hold, false);
    assert_eq!(msg[5], SUBCMD_GOTO_ABSOLUTE);
    let pos_bytes = (-180_i32).to_le_bytes();
    assert_eq!(msg[6], pos_bytes[0]);
    assert_eq!(msg[7], pos_bytes[1]);
    assert_eq!(msg[8], pos_bytes[2]);
    assert_eq!(msg[9], pos_bytes[3]);
    assert_eq!(msg[10], 50);
}

#[test]
fn test_cmd_reset_zero() {
    let msg = cmd_reset_zero(0x02, false);
    assert_eq!(msg[2], 0x81);
    assert_eq!(msg[5], SUBCMD_WRITE_DIRECT_MODE);
    assert_eq!(msg[6], 0x02);
    assert_eq!(msg[7..11], [0, 0, 0, 0]);
}

#[test]
fn test_cmd_subscribe() {
    let msg = cmd_subscribe(0x00, 0x02);
    assert_eq!(msg[2], MessageType::PortInputFormatSetupSingle as u8);
    assert_eq!(msg[3], 0x00);
    assert_eq!(msg[4], 0x02);
    assert_eq!(msg[9], 0x01);
}

#[test]
fn test_cmd_unsubscribe() {
    let msg = cmd_unsubscribe(0x00, 0x02);
    assert_eq!(msg[9], 0x00);
}

#[test]
fn test_cmd_disconnect() {
    let msg = cmd_disconnect();
    assert_eq!(msg[2], MessageType::HubActions as u8);
    assert_eq!(msg[3], 0x02);
}

#[test]
fn test_wedo2_motor() {
    let msg = wedo2_cmd_motor(1, 75);
    assert_eq!(msg, vec![1, 0x01, 0x02, 75]);
}

#[test]
fn test_wedo2_subscribe() {
    let msg = wedo2_cmd_subscribe(1, 34, 0);
    assert_eq!(msg[0], 0x01);
    assert_eq!(msg[1], 0x02);
    assert_eq!(msg[2], 1);
    assert_eq!(msg[3], 34);
    assert_eq!(msg[4], 0);
    assert_eq!(msg[10], 0x01);
}

#[test]
fn test_format_version() {
    assert_eq!(format_version(0x17_00_00_00_u32 as i32), "1.7.00.0000");
}

#[test]
fn test_parse_hub_property_battery() {
    let msg = vec![6, 0x00, 0x01, 0x06, 0x06, 85];
    let val = parse_hub_property(&msg).unwrap();
    assert_eq!(val, HubPropertyValue::BatteryVoltage(85));
}

#[test]
fn test_parse_hub_property_rssi() {
    let msg = vec![6, 0x00, 0x01, 0x05, 0x06, 0xD0];
    let val = parse_hub_property(&msg).unwrap();
    assert_eq!(val, HubPropertyValue::Rssi(-48));
}

#[test]
fn test_parse_hub_property_button() {
    let msg = vec![6, 0x00, 0x01, 0x02, 0x06, 0x02];
    let val = parse_hub_property(&msg).unwrap();
    assert_eq!(val, HubPropertyValue::Button(true));
}

#[test]
fn test_feedback_flags() {
    let fb = PortFeedback {
        port_id: 0,
        feedback: 0x0a,
    };
    assert!(fb.is_completed());
    assert!(fb.is_buffer_empty());
    assert!(!fb.is_discarded());
}
