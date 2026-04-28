use super::*;

#[test]
fn test_port_index_and_letter() {
    for (letter, index) in [("a", 0), ("b", 1), ("c", 2), ("d", 3), ("e", 4), ("f", 5)] {
        assert_eq!(port_index(letter), Some(index));
        assert_eq!(port_letter(index), letter);
    }
    assert_eq!(port_index("g"), None);
    assert_eq!(port_index("A"), Some(0));
}

#[test]
fn test_device_classification() {
    assert!(is_motor(DEVICE_LARGE_ANGULAR_MOTOR));
    assert!(is_motor(DEVICE_MEDIUM_ANGULAR_MOTOR_GREY));
    assert!(is_motor(DEVICE_TRAIN_MOTOR));
    assert!(!is_motor(DEVICE_COLOR_SENSOR));

    assert!(is_led(DEVICE_LIGHT));
    assert!(!is_led(DEVICE_PASSIVE_MOTOR));
    assert!(!is_led(DEVICE_COLOR_SENSOR));

    assert!(is_sensor(DEVICE_COLOR_SENSOR));
    assert!(is_sensor(DEVICE_DISTANCE_SENSOR));
    assert!(is_sensor(DEVICE_FORCE_SENSOR));
    assert!(!is_sensor(DEVICE_LARGE_ANGULAR_MOTOR));

    assert!(is_tacho_motor(DEVICE_LARGE_ANGULAR_MOTOR));
    assert!(is_tacho_motor(DEVICE_MEDIUM_LINEAR_MOTOR));
    assert!(!is_tacho_motor(DEVICE_TRAIN_MOTOR));
    assert!(!is_tacho_motor(DEVICE_PASSIVE_MOTOR));

    assert!(is_absolute_motor(DEVICE_LARGE_ANGULAR_MOTOR));
    assert!(is_absolute_motor(DEVICE_SMALL_ANGULAR_MOTOR));
    assert!(!is_absolute_motor(DEVICE_MEDIUM_LINEAR_MOTOR));
    assert!(!is_absolute_motor(DEVICE_LARGE_MOTOR));
}
