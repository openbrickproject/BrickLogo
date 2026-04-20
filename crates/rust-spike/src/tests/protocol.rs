use super::*;

#[test]
fn test_cmd_init_imports() {
    let cmd = cmd_init_imports();
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("import motor"));
    assert!(s.contains("from hub import port"));
    assert_eq!(*cmd.last().unwrap(), CTRL_D);
}

#[test]
fn test_cmd_motor_run() {
    let cmd = cmd_motor_run("a", 500);
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("motor.run(port.A, 500)"));
    assert_eq!(*cmd.last().unwrap(), CTRL_D);
}

#[test]
fn test_cmd_motor_stop() {
    let cmd = cmd_motor_stop("b");
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("motor.stop(port.B)"));
}

#[test]
fn test_cmd_motor_run_for_degrees() {
    let cmd = cmd_motor_run_for_degrees("c", 360, 750);
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("runloop.run(motor.run_for_degrees(port.C, 360, 750))"));
}

#[test]
fn test_cmd_motor_run_for_time() {
    let cmd = cmd_motor_run_for_time("d", 2000, -500);
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("motor.run_for_time(port.D, 2000, -500)"));
    assert!(s.contains("runloop.run("));
}

#[test]
fn test_cmd_motor_run_to_absolute_position() {
    let cmd = cmd_motor_run_to_absolute_position("e", 90, 500, 1);
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("motor.run_to_absolute_position(port.E, 90, 500, direction=1)"));
}

#[test]
fn test_cmd_motor_reset_relative_position() {
    let cmd = cmd_motor_reset_relative_position("f", 0);
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("motor.reset_relative_position(port.F, 0)"));
    // Not wrapped in runloop — non-blocking
    assert!(!s.contains("runloop"));
}

#[test]
fn test_cmd_parallel_run_for_degrees() {
    let cmd = cmd_parallel_run_for_degrees(&[("a", 360, 500), ("b", 360, -500)]);
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("runloop.run("));
    assert!(s.contains("motor.run_for_degrees(port.A, 360, 500)"));
    assert!(s.contains("motor.run_for_degrees(port.B, 360, -500)"));
}

#[test]
fn test_cmd_parallel_run_for_time() {
    let cmd = cmd_parallel_run_for_time(&[("a", 500), ("b", -500)], 2000);
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("runloop.run("));
    assert!(s.contains("motor.run_for_time(port.A, 2000, 500)"));
    assert!(s.contains("motor.run_for_time(port.B, 2000, -500)"));
}

#[test]
fn test_cmd_read_relative_position() {
    let cmd = cmd_read_relative_position("a");
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("print(motor.relative_position(port.A))"));
}

#[test]
fn test_cmd_read_color() {
    let cmd = cmd_read_color("c");
    let s = String::from_utf8_lossy(&cmd[..cmd.len() - 1]);
    assert!(s.contains("import color_sensor"));
    assert!(s.contains("print(color_sensor.color(port.C))"));
}

#[test]
fn test_parse_raw_repl_response_success() {
    let data = b"OK42\x04\x04";
    assert_eq!(parse_raw_repl_response(data), Ok("42".to_string()));
}

#[test]
fn test_parse_raw_repl_response_empty_output() {
    let data = b"OK\x04\x04";
    assert_eq!(parse_raw_repl_response(data), Ok("".to_string()));
}

#[test]
fn test_parse_raw_repl_response_error() {
    let data = b"OK\x04Traceback: something went wrong\x04";
    assert!(parse_raw_repl_response(data).is_err());
    assert!(parse_raw_repl_response(data).unwrap_err().contains("Traceback"));
}

#[test]
fn test_parse_raw_repl_response_multiline_output() {
    let data = b"OK[1, 2, 3]\r\n\x04\x04";
    assert_eq!(parse_raw_repl_response(data), Ok("[1, 2, 3]".to_string()));
}

#[test]
fn test_port_ref_uppercase() {
    let cmd = cmd_motor_run("a", 100);
    let s = String::from_utf8_lossy(&cmd);
    assert!(s.contains("port.A"));

    let cmd = cmd_motor_run("F", 100);
    let s = String::from_utf8_lossy(&cmd);
    assert!(s.contains("port.F"));
}
