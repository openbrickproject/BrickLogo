use super::*;

#[test]
fn test_encode_command_adds_id_and_newline() {
    let body = motor_run("a", 500);
    let bytes = encode_command(42, body);
    assert_eq!(*bytes.last().unwrap(), b'\n');
    let v: Value = serde_json::from_slice(&bytes[..bytes.len() - 1]).unwrap();
    assert_eq!(v["id"], 42);
    assert_eq!(v["op"], "motor_run");
    assert_eq!(v["port"], "a");
    assert_eq!(v["velocity"], 500);
}

#[test]
fn test_motor_stop() {
    let v = motor_stop("b");
    assert_eq!(v["op"], "motor_stop");
    assert_eq!(v["port"], "b");
}

#[test]
fn test_motor_reset() {
    let v = motor_reset("c", 0);
    assert_eq!(v["op"], "motor_reset");
    assert_eq!(v["port"], "c");
    assert_eq!(v["offset"], 0);
}

#[test]
fn test_motor_run_for_time() {
    let v = motor_run_for_time("a", 1000, 500);
    assert_eq!(v["op"], "motor_run_for_time");
    assert_eq!(v["ms"], 1000);
    assert_eq!(v["velocity"], 500);
}

#[test]
fn test_motor_run_for_degrees() {
    let v = motor_run_for_degrees("c", 360, 750);
    assert_eq!(v["op"], "motor_run_for_degrees");
    assert_eq!(v["degrees"], 360);
    assert_eq!(v["velocity"], 750);
}

#[test]
fn test_motor_run_to_abs() {
    let v = motor_run_to_abs("e", 90, 500, 1);
    assert_eq!(v["op"], "motor_run_to_abs");
    assert_eq!(v["position"], 90);
    assert_eq!(v["velocity"], 500);
    assert_eq!(v["direction"], 1);
}

#[test]
fn test_parallel_run_for_degrees() {
    let v = parallel_run_for_degrees(&[("a", 360, 500), ("b", 360, -500)]);
    assert_eq!(v["op"], "parallel_run_for_degrees");
    let entries = v["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["port"], "a");
    assert_eq!(entries[1]["velocity"], -500);
}

#[test]
fn test_parallel_run_for_time() {
    let v = parallel_run_for_time(&[("a", 500), ("b", -500)], 2000);
    assert_eq!(v["op"], "parallel_run_for_time");
    assert_eq!(v["ms"], 2000);
    assert_eq!(v["entries"].as_array().unwrap().len(), 2);
}

#[test]
fn test_parallel_run_to_abs() {
    let v = parallel_run_to_abs(&[("a", 90, 500, 0), ("b", -45, 500, 1)]);
    let entries = v["entries"].as_array().unwrap();
    assert_eq!(entries[0]["direction"], 0);
    assert_eq!(entries[1]["direction"], 1);
}

#[test]
fn test_read_sensor() {
    let v = read_sensor("a", "rotation");
    assert_eq!(v["op"], "read");
    assert_eq!(v["port"], "a");
    assert_eq!(v["mode"], "rotation");
}

#[test]
fn test_read_hub() {
    let v = read_hub("tilt");
    assert_eq!(v["op"], "read");
    assert_eq!(v["mode"], "tilt");
    assert!(v.get("port").is_none());
}

#[test]
fn test_ping() {
    assert_eq!(ping()["op"], "ping");
}

#[test]
fn test_parse_reply_success_void() {
    let bytes = br#"{"id":5,"ok":true}"#;
    assert_eq!(parse_reply(bytes).unwrap(), Value::Null);
}

#[test]
fn test_parse_reply_success_value() {
    let bytes = br#"{"id":5,"value":180}"#;
    assert_eq!(parse_reply(bytes).unwrap(), serde_json::json!(180));
}

#[test]
fn test_parse_reply_error() {
    let bytes = br#"{"id":5,"error":"motor offline"}"#;
    let err = parse_reply(bytes).unwrap_err();
    assert!(err.contains("motor offline"));
}

#[test]
fn test_reply_id() {
    assert_eq!(reply_id(br#"{"id":42,"ok":true}"#), Some(42));
    assert_eq!(reply_id(br#"{"op":"ready"}"#), None);
    assert_eq!(reply_id(b"not json"), None);
}

#[test]
fn test_is_ready() {
    assert!(is_ready(br#"{"op":"ready"}"#));
    assert!(!is_ready(br#"{"id":1,"ok":true}"#));
    assert!(!is_ready(b"not json"));
}
