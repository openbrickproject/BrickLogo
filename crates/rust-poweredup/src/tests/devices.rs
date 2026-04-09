use super::*;

#[test]
fn test_mode_map_technic_color_sensor() {
    let map = mode_map_for_device(DeviceType::TechnicColorSensor);
    assert_eq!(map.len(), 6);
    assert_eq!(map[0].event, "color");
    assert_eq!(map[0].mode, 0x00);
    assert_eq!(map[1].event, "light");
    assert_eq!(map[1].mode, 0x01);
}

#[test]
fn test_mode_for_event() {
    assert_eq!(
        mode_for_event(DeviceType::TechnicColorSensor, "color"),
        Some(0x00)
    );
    assert_eq!(
        mode_for_event(DeviceType::TechnicColorSensor, "light"),
        Some(0x01)
    );
    assert_eq!(
        mode_for_event(DeviceType::TechnicColorSensor, "nonexistent"),
        None
    );
}

#[test]
fn test_default_event() {
    assert_eq!(default_event(DeviceType::TechnicColorSensor), Some("color"));
    assert_eq!(default_event(DeviceType::TechnicForceSensor), Some("force"));
    assert_eq!(default_event(DeviceType::TrainMotor), None);
}

#[test]
fn test_mode_map_absolute_motor() {
    let map = mode_map_for_device(DeviceType::TechnicMediumAngularMotor);
    assert_eq!(map.len(), 2);
    assert_eq!(map[0].event, "rotation");
    assert_eq!(map[1].event, "absolute");
}

#[test]
fn test_mode_map_tacho_motor() {
    let map = mode_map_for_device(DeviceType::MediumLinearMotor);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].event, "rotation");
}

#[test]
fn test_parse_motor_rotation() {
    let data = 360_i32.to_le_bytes();
    let reading = parse_sensor_data(DeviceType::TechnicMediumAngularMotor, 0x02, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(360.0)));
}

#[test]
fn test_parse_motor_rotation_negative() {
    let data = (-180_i32).to_le_bytes();
    let reading = parse_sensor_data(DeviceType::MediumLinearMotor, 0x02, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(-180.0)));
}

#[test]
fn test_parse_motor_absolute() {
    let data = (-90_i16).to_le_bytes();
    let reading = parse_sensor_data(DeviceType::TechnicMediumAngularMotor, 0x03, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(-90.0)));
}

#[test]
fn test_parse_technic_color_sensor_color() {
    let data = [3];
    let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(3.0)));
}

#[test]
fn test_parse_technic_color_sensor_reflect() {
    let data = [75];
    let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x01, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(75.0)));
}

#[test]
fn test_parse_technic_color_sensor_rgb() {
    let data = [100, 0, 200, 0, 0x2C, 0x01, 0x90, 0x01];
    let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x05, &data, false);
    assert_eq!(
        reading,
        Some(SensorReading::Quad(100.0, 200.0, 300.0, 400.0))
    );
}

#[test]
fn test_parse_technic_color_sensor_hsv() {
    let data = [180, 0, 100, 0, 50, 0];
    let reading = parse_sensor_data(DeviceType::TechnicColorSensor, 0x06, &data, false);
    assert_eq!(reading, Some(SensorReading::Triple(180.0, 100.0, 50.0)));
}

#[test]
fn test_parse_color_distance_color() {
    let data = [9];
    let reading = parse_sensor_data(DeviceType::ColorDistanceSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(9.0)));
}

#[test]
fn test_parse_color_distance_distance() {
    let data = [4];
    let reading = parse_sensor_data(DeviceType::ColorDistanceSensor, 0x01, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(4.0 * 25.4 - 20.0)));
}

#[test]
fn test_parse_color_distance_rgb() {
    let data = [0xFF, 0x00, 0x80, 0x00, 0x40, 0x00];
    let reading = parse_sensor_data(DeviceType::ColorDistanceSensor, 0x06, &data, false);
    assert_eq!(reading, Some(SensorReading::Triple(255.0, 128.0, 64.0)));
}

#[test]
fn test_parse_technic_distance() {
    let data = [0xE8, 0x03];
    let reading = parse_sensor_data(DeviceType::TechnicDistanceSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(1000.0)));
}

#[test]
fn test_parse_technic_force() {
    let data = [50];
    let reading = parse_sensor_data(DeviceType::TechnicForceSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(5.0)));
}

#[test]
fn test_parse_technic_force_touched() {
    let data = [1];
    let reading = parse_sensor_data(DeviceType::TechnicForceSensor, 0x01, &data, false);
    assert_eq!(reading, Some(SensorReading::Bool(true)));

    let data = [0];
    let reading = parse_sensor_data(DeviceType::TechnicForceSensor, 0x01, &data, false);
    assert_eq!(reading, Some(SensorReading::Bool(false)));
}

#[test]
fn test_parse_tilt_sensor() {
    let data = [10_i8 as u8, 245_u8];
    let reading = parse_sensor_data(DeviceType::TiltSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Pair(10.0, -11.0)));
}

#[test]
fn test_parse_move_hub_tilt() {
    let data = [10_i8 as u8, 20];
    let reading = parse_sensor_data(DeviceType::MoveHubTiltSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Pair(-10.0, 20.0)));
}

#[test]
fn test_parse_technic_tilt() {
    let data = [100, 0, 200, 0, 0x2C, 0x01];
    let reading = parse_sensor_data(DeviceType::TechnicMediumHubTiltSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Triple(300.0, 200.0, -100.0)));
}

#[test]
fn test_parse_technic_accel() {
    let data = [0x00, 0x10, 0x00, 0xF0, 0x00, 0x00];
    let reading = parse_sensor_data(
        DeviceType::TechnicMediumHubAccelerometer,
        0x00,
        &data,
        false,
    );
    assert_eq!(reading, Some(SensorReading::Triple(1000.0, -1000.0, 0.0)));
}

#[test]
fn test_parse_technic_gyro() {
    let data = [0x90, 0x01, 0x00, 0x00, 0x00, 0x00];
    let reading = parse_sensor_data(DeviceType::TechnicMediumHubGyroSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Triple(7.0, 0.0, 0.0)));
}

#[test]
fn test_parse_motion_sensor() {
    let data = [5, 0];
    let reading = parse_sensor_data(DeviceType::MotionSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(50.0)));
}

#[test]
fn test_parse_motion_sensor_extended() {
    let data = [5, 1];
    let reading = parse_sensor_data(DeviceType::MotionSensor, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(2600.0)));
}

#[test]
fn test_parse_remote_button() {
    let data = [1];
    let reading = parse_sensor_data(DeviceType::RemoteControlButton, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(1.0)));
}

#[test]
fn test_parse_voltage_lpf2() {
    let data = [0x35, 0x0F];
    let reading = parse_sensor_data(DeviceType::VoltageSensor, 0x00, &data, false);
    let expected = 3893.0 * 9.615 / 3893.0;
    assert_eq!(reading, Some(SensorReading::Number(expected)));
}

#[test]
fn test_parse_voltage_wedo2() {
    let data = [0xA0, 0x0F];
    let reading = parse_sensor_data(DeviceType::VoltageSensor, 0x00, &data, true);
    assert_eq!(reading, Some(SensorReading::Number(100.0)));
}

#[test]
fn test_parse_duplo_color_sensor() {
    let data = [6];
    let reading = parse_sensor_data(DeviceType::DuploTrainBaseColorSensor, 0x01, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(6.0)));
}

#[test]
fn test_parse_duplo_color_sensor_invalid() {
    let data = [50];
    let reading = parse_sensor_data(DeviceType::DuploTrainBaseColorSensor, 0x01, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(255.0)));
}

#[test]
fn test_parse_duplo_speedometer() {
    let data = [0x20, 0x00];
    let reading = parse_sensor_data(DeviceType::DuploTrainBaseSpeedometer, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(32.0)));
}

#[test]
fn test_parse_duplo_speedometer_negative() {
    let data = (-15_i16).to_le_bytes();
    let reading = parse_sensor_data(DeviceType::DuploTrainBaseSpeedometer, 0x00, &data, false);
    assert_eq!(reading, Some(SensorReading::Number(-15.0)));
}

#[test]
fn test_build_mode_lookup() {
    let lookup = build_mode_lookup(DeviceType::TechnicForceSensor);
    assert_eq!(lookup.get("force"), Some(&0x00));
    assert_eq!(lookup.get("touched"), Some(&0x01));
    assert_eq!(lookup.get("tapped"), Some(&0x02));
    assert_eq!(lookup.len(), 3);
}

#[test]
fn test_unknown_device_has_no_modes() {
    let map = mode_map_for_device(DeviceType::Unknown);
    assert!(map.is_empty());
}

#[test]
fn test_basic_motor_has_no_modes() {
    let map = mode_map_for_device(DeviceType::TrainMotor);
    assert!(map.is_empty());
}
