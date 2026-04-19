use super::*;

#[test]
fn test_task_id_id_gen() {
    let mut id_gen = TaskIdGen::new();
    assert_eq!(id_gen.next(), "0000");
    assert_eq!(id_gen.next(), "0001");
    assert_eq!(id_gen.next(), "0002");
}

#[test]
fn test_task_id_wraps() {
    let mut id_gen = TaskIdGen { counter: 0xFFFE };
    assert_eq!(id_gen.next(), "fffe");
    assert_eq!(id_gen.next(), "ffff");
    assert_eq!(id_gen.next(), "0000");
}

#[test]
fn test_cmd_program_modechange() {
    let cmd = cmd_program_modechange();
    assert!(cmd.ends_with('\r'));
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "program_modechange");
    assert_eq!(v["p"]["mode"], "play");
}

#[test]
fn test_cmd_motor_start() {
    let cmd = cmd_motor_start("0001", "A", 75, true);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["i"], "0001");
    assert_eq!(v["m"], "scratch.motor_start");
    assert_eq!(v["p"]["port"], "A");
    assert_eq!(v["p"]["speed"], 75);
    assert_eq!(v["p"]["stall"], true);
}

#[test]
fn test_cmd_motor_stop() {
    let cmd = cmd_motor_stop("0002", "B", 1, 100);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "scratch.motor_stop");
    assert_eq!(v["p"]["stop"], 1);
}

#[test]
fn test_cmd_motor_run_timed() {
    let cmd = cmd_motor_run_timed("0003", "C", 50, 2000, true, 1);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "scratch.motor_run_timed");
    assert_eq!(v["p"]["time"], 2000);
    assert_eq!(v["p"]["speed"], 50);
}

#[test]
fn test_cmd_motor_run_for_degrees() {
    let cmd = cmd_motor_run_for_degrees("0004", "D", -80, 360, false, 2);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "scratch.motor_run_for_degrees");
    assert_eq!(v["p"]["degrees"], 360);
    assert_eq!(v["p"]["speed"], -80);
    assert_eq!(v["p"]["stop"], 2);
}

#[test]
fn test_cmd_motor_go_direction_to_position() {
    let cmd = cmd_motor_go_direction_to_position("0005", "E", 90, 50, "clockwise", true, 1);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "scratch.motor_go_direction_to_position");
    assert_eq!(v["p"]["position"], 90);
    assert_eq!(v["p"]["direction"], "clockwise");
}

#[test]
fn test_cmd_motor_set_position() {
    let cmd = cmd_motor_set_position("0006", "F", 0);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "scratch.motor_set_position");
    assert_eq!(v["p"]["offset"], 0);
}

#[test]
fn test_cmd_move_start_speeds() {
    let cmd = cmd_move_start_speeds("0007", "A", "B", 50, -50);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "scratch.move_start_speeds");
    assert_eq!(v["p"]["lmotor"], "A");
    assert_eq!(v["p"]["rmotor"], "B");
    assert_eq!(v["p"]["lspeed"], 50);
    assert_eq!(v["p"]["rspeed"], -50);
}

#[test]
fn test_cmd_move_stop() {
    let cmd = cmd_move_stop("0008", "A", "B", 0);
    let v: serde_json::Value = serde_json::from_str(cmd.trim_end_matches('\r')).unwrap();
    assert_eq!(v["m"], "scratch.move_stop");
    assert_eq!(v["p"]["stop"], 0);
}

#[test]
fn test_parse_task_complete() {
    let line = r#"{"i":"0001","r":0}"#;
    match parse_message(line) {
        SpikeMessage::TaskComplete { task_id, result } => {
            assert_eq!(task_id, "0001");
            assert_eq!(result, 0);
        }
        other => panic!("Expected TaskComplete, got {:?}", other),
    }
}

#[test]
fn test_parse_battery() {
    let line = r#"{"m":2,"p":[7.8,95]}"#;
    match parse_message(line) {
        SpikeMessage::Battery { voltage, percentage } => {
            assert!((voltage - 7.8).abs() < 0.01);
            assert!((percentage - 95.0).abs() < 0.01);
        }
        other => panic!("Expected Battery, got {:?}", other),
    }
}

#[test]
fn test_parse_telemetry() {
    // Minimal telemetry with one motor on port 0 and empty remaining ports
    let motor_data = "[49,[10,180,45,50]]";
    let empty = "[0,[]]";
    let ports = format!("[{},{},{},{},{},{}]", motor_data, empty, empty, empty, empty, empty);
    let line = format!(
        r#"{{"m":0,"p":[{},null,null,null,null,null,[1.0,2.0,3.0],[4.0,5.0,6.0],[7.0,8.0,9.0]]}}"#,
        ports
    );
    match parse_message(&line) {
        SpikeMessage::Telemetry(data) => {
            assert_eq!(data.ports[0].device_type, 49);
            assert!((data.ports[0].data[0] - 10.0).abs() < 0.01); // speed
            assert!((data.ports[0].data[1] - 180.0).abs() < 0.01); // rel pos
            assert!((data.ports[0].data[2] - 45.0).abs() < 0.01); // abs pos
            assert!((data.ports[0].data[3] - 50.0).abs() < 0.01); // power
            assert_eq!(data.ports[1].device_type, 0);
            assert!((data.imu.accel[0] - 1.0).abs() < 0.01);
            assert!((data.imu.gyro[1] - 5.0).abs() < 0.01);
            assert!((data.imu.yaw_pitch_roll[2] - 9.0).abs() < 0.01);
        }
        other => panic!("Expected Telemetry, got {:?}", other),
    }
}

#[test]
fn test_parse_unknown() {
    assert!(matches!(parse_message("garbage"), SpikeMessage::Unknown));
    assert!(matches!(parse_message(r#"{"m":3,"p":[1]}"#), SpikeMessage::Unknown));
}
