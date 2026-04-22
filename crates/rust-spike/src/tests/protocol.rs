use super::*;

#[test]
fn test_port_index_all_letters() {
    assert_eq!(port_index("a").unwrap(), 0);
    assert_eq!(port_index("F").unwrap(), 5);
    assert!(port_index("g").is_err());
}

#[test]
fn test_sensor_mode_lookup() {
    assert_eq!(sensor_mode("rotation").unwrap(), MODE_ROTATION);
    assert_eq!(sensor_mode("raw").unwrap(), MODE_ROTATION);
    assert_eq!(sensor_mode("touched").unwrap(), MODE_TOUCHED);
    assert!(sensor_mode("bogus").is_err());
}

#[test]
fn test_motor_run_layout() {
    let bytes = motor_run(0x1234, "c", 500).unwrap();
    assert_eq!(bytes[0], OP_MOTOR_RUN);
    assert_eq!(&bytes[1..3], &0x1234u16.to_le_bytes());
    assert_eq!(bytes[3], 2); // c
    assert_eq!(&bytes[4..6], &500i16.to_le_bytes());
    assert_eq!(bytes.len(), 6);
}

#[test]
fn test_motor_stop_layout() {
    let bytes = motor_stop(7, "a").unwrap();
    assert_eq!(bytes, vec![OP_MOTOR_STOP, 7, 0, 0]);
}

#[test]
fn test_motor_reset_layout() {
    let bytes = motor_reset(1, "d", -17).unwrap();
    assert_eq!(bytes[0], OP_MOTOR_RESET);
    assert_eq!(bytes[3], 3);
    assert_eq!(&bytes[4..8], &(-17i32).to_le_bytes());
}

#[test]
fn test_motor_run_for_degrees_layout() {
    let bytes = motor_run_for_degrees(100, "c", 360, 500).unwrap();
    assert_eq!(bytes[0], OP_MOTOR_RUN_FOR_DEGREES);
    assert_eq!(&bytes[1..3], &100u16.to_le_bytes());
    assert_eq!(bytes[3], 2);
    assert_eq!(&bytes[4..8], &360i32.to_le_bytes());
    assert_eq!(&bytes[8..10], &500i16.to_le_bytes());
}

#[test]
fn test_motor_run_to_abs_layout() {
    let bytes = motor_run_to_abs(1, "e", 90, 500, 1).unwrap();
    assert_eq!(bytes[0], OP_MOTOR_RUN_TO_ABS);
    assert_eq!(bytes[3], 4);
    assert_eq!(&bytes[4..8], &90i32.to_le_bytes());
    assert_eq!(&bytes[8..10], &500i16.to_le_bytes());
    assert_eq!(bytes[10], 1);
}

#[test]
fn test_parallel_run_for_degrees_layout() {
    let bytes = parallel_run_for_degrees(0x42, &[("a", 360, 500), ("b", 180, -500)]).unwrap();
    assert_eq!(bytes[0], OP_PARALLEL_RUN_FOR_DEGREES);
    assert_eq!(&bytes[1..3], &0x42u16.to_le_bytes());
    assert_eq!(bytes[3], 2);
    // Entry 0: port=0, degrees=360, velocity=500
    assert_eq!(bytes[4], 0);
    assert_eq!(&bytes[5..9], &360i32.to_le_bytes());
    assert_eq!(&bytes[9..11], &500i16.to_le_bytes());
    // Entry 1: port=1, degrees=180, velocity=-500
    assert_eq!(bytes[11], 1);
    assert_eq!(&bytes[12..16], &180i32.to_le_bytes());
    assert_eq!(&bytes[16..18], &(-500i16).to_le_bytes());
}

#[test]
fn test_parallel_run_for_time_layout() {
    let bytes = parallel_run_for_time(1, 1000, &[("a", 500), ("b", -500)]).unwrap();
    assert_eq!(bytes[0], OP_PARALLEL_RUN_FOR_TIME);
    assert_eq!(&bytes[3..7], &1000u32.to_le_bytes());
    assert_eq!(bytes[7], 2);
    assert_eq!(bytes[8], 0);
    assert_eq!(&bytes[9..11], &500i16.to_le_bytes());
    assert_eq!(bytes[11], 1);
    assert_eq!(&bytes[12..14], &(-500i16).to_le_bytes());
}

#[test]
fn test_parallel_run_to_abs_layout() {
    let bytes = parallel_run_to_abs(1, &[("a", 90, 500, 0), ("b", -45, 500, 1)]).unwrap();
    assert_eq!(bytes[0], OP_PARALLEL_RUN_TO_ABS);
    assert_eq!(bytes[3], 2);
    // Entry 0
    assert_eq!(bytes[4], 0);
    assert_eq!(&bytes[5..9], &90i32.to_le_bytes());
    assert_eq!(&bytes[9..11], &500i16.to_le_bytes());
    assert_eq!(bytes[11], 0);
}

#[test]
fn test_read_sensor_layout() {
    let bytes = read_sensor(5, "a", "rotation").unwrap();
    assert_eq!(bytes, vec![OP_READ, 5, 0, 0, MODE_ROTATION]);
}

#[test]
fn test_read_hub_layout() {
    let bytes = read_hub(5, "tilt").unwrap();
    assert_eq!(bytes, vec![OP_READ_HUB, 5, 0, MODE_TILT]);
}

#[test]
fn test_ping_layout() {
    assert_eq!(ping(9), vec![OP_PING, 9, 0]);
}

#[test]
fn test_parse_event_ok() {
    let bytes = vec![REPLY_OK, 0x42, 0x00];
    assert_eq!(
        parse_event(&bytes).unwrap(),
        Event::Reply { rid: 0x42, reply: Reply::Ok }
    );
}

#[test]
fn test_parse_event_int() {
    let mut bytes = vec![REPLY_INT, 0x01, 0x00];
    bytes.extend_from_slice(&180i32.to_le_bytes());
    assert_eq!(
        parse_event(&bytes).unwrap(),
        Event::Reply { rid: 1, reply: Reply::Int(180) }
    );
}

#[test]
fn test_parse_event_list() {
    let mut bytes = vec![REPLY_LIST, 0x01, 0x00, 0x03];
    bytes.extend_from_slice(&10i32.to_le_bytes());
    bytes.extend_from_slice(&20i32.to_le_bytes());
    bytes.extend_from_slice(&(-5i32).to_le_bytes());
    assert_eq!(
        parse_event(&bytes).unwrap(),
        Event::Reply { rid: 1, reply: Reply::List(vec![10, 20, -5]) }
    );
}

#[test]
fn test_parse_event_bool() {
    assert_eq!(
        parse_event(&[REPLY_BOOL, 0x01, 0x00, 0x00]).unwrap(),
        Event::Reply { rid: 1, reply: Reply::Bool(false) }
    );
    assert_eq!(
        parse_event(&[REPLY_BOOL, 0x02, 0x00, 0x01]).unwrap(),
        Event::Reply { rid: 2, reply: Reply::Bool(true) }
    );
}

#[test]
fn test_parse_event_error() {
    let msg = b"motor offline";
    let mut bytes = vec![REPLY_ERROR, 0x05, 0x00, msg.len() as u8];
    bytes.extend_from_slice(msg);
    assert_eq!(
        parse_event(&bytes).unwrap(),
        Event::Reply { rid: 5, reply: Reply::Error("motor offline".to_string()) }
    );
}

#[test]
fn test_parse_event_ready_heartbeat() {
    assert_eq!(parse_event(&[REPLY_READY]).unwrap(), Event::Ready);
    assert_eq!(parse_event(&[REPLY_HEARTBEAT]).unwrap(), Event::Heartbeat);
}

#[test]
fn test_parse_event_empty() {
    assert!(parse_event(&[]).is_err());
}

#[test]
fn test_parse_event_unknown_kind() {
    assert!(parse_event(&[0xFF, 0x01, 0x00]).is_err());
}
