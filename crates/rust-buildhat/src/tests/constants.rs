use super::*;

#[test]
fn test_is_motor() {
    assert!(is_motor(DEVICE_MEDIUM_ANGULAR_MOTOR));
    assert!(is_motor(DEVICE_PASSIVE_MOTOR));
    assert!(!is_motor(DEVICE_COLOR_SENSOR));
}

#[test]
fn test_is_led() {
    assert!(is_led(DEVICE_LIGHT));
    assert!(!is_led(DEVICE_PASSIVE_MOTOR));
    assert!(!is_led(DEVICE_COLOR_SENSOR));
}

#[test]
fn test_is_sensor() {
    assert!(is_sensor(DEVICE_COLOR_SENSOR));
    assert!(is_sensor(DEVICE_FORCE_SENSOR));
    assert!(!is_sensor(DEVICE_LARGE_MOTOR));
}

#[test]
fn test_port_mapping() {
    assert_eq!(port_letter(0), "a");
    assert_eq!(port_letter(3), "d");
    assert_eq!(port_index("A"), Some(0));
    assert_eq!(port_index("d"), Some(3));
    assert_eq!(port_index("e"), None);
}
