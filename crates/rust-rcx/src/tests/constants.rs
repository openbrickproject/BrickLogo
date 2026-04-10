use super::*;

#[test]
fn test_motor_mask() {
    assert_eq!(motor_mask("A"), Some(0x01));
    assert_eq!(motor_mask("b"), Some(0x02));
    assert_eq!(motor_mask("C"), Some(0x04));
    assert_eq!(motor_mask("D"), None);
}

#[test]
fn test_sensor_index() {
    assert_eq!(sensor_index("1"), Some(0));
    assert_eq!(sensor_index("2"), Some(1));
    assert_eq!(sensor_index("3"), Some(2));
    assert_eq!(sensor_index("4"), None);
}
