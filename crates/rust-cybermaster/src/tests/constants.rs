use super::*;

#[test]
fn test_motor_mask() {
    assert_eq!(motor_mask("a"), Some(MOTOR_A));
    assert_eq!(motor_mask("B"), Some(MOTOR_B));
    assert_eq!(motor_mask("c"), Some(MOTOR_C));
    // "x" is the LEGO/NQC name for the external motor (= OUT_C).
    assert_eq!(motor_mask("x"), Some(MOTOR_C));
    assert_eq!(motor_mask("d"), None);
}

#[test]
fn test_tacho_index() {
    // Only the two internal motors have tachometers; the external motor (C/X)
    // and the passive sensor ports have none.
    assert_eq!(tacho_index("a"), Some(0));
    assert_eq!(tacho_index("B"), Some(1));
    assert_eq!(tacho_index("c"), None);
    assert_eq!(tacho_index("x"), None);
    assert_eq!(tacho_index("1"), None);
}

#[test]
fn test_sensor_index() {
    assert_eq!(sensor_index("1"), Some(0));
    assert_eq!(sensor_index("2"), Some(1));
    assert_eq!(sensor_index("3"), Some(2));
    assert_eq!(sensor_index("4"), None);
    assert_eq!(sensor_index("a"), None);
}
