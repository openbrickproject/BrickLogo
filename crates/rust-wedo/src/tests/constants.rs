use super::*;

#[test]
fn test_sensor_type_detection() {
    assert_eq!(get_sensor_type(0), SensorType::Unknown);
    assert_eq!(get_sensor_type(35), SensorType::Tilt);
    assert_eq!(get_sensor_type(180), SensorType::Distance);
    assert_eq!(get_sensor_type(250), SensorType::Unknown);
}

#[test]
fn test_tilt_events() {
    assert_eq!(get_tilt_event(0), TiltEvent::Unknown);
    assert_eq!(get_tilt_event(25), TiltEvent::Back);
    assert_eq!(get_tilt_event(75), TiltEvent::Right);
    assert_eq!(get_tilt_event(120), TiltEvent::Level);
    assert_eq!(get_tilt_event(180), TiltEvent::Front);
    assert_eq!(get_tilt_event(230), TiltEvent::Left);
}

#[test]
fn test_distance_conversion() {
    assert_eq!(get_distance(71), 0);
    assert_eq!(get_distance(219), 100);
    assert_eq!(get_distance(145), 50);
    assert_eq!(get_distance(0), 0);
    assert_eq!(get_distance(255), 100);
}
