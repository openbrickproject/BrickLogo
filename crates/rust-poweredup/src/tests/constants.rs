use super::*;

#[test]
fn test_hub_type_from_manufacturer_byte() {
    assert_eq!(hub_type_from_manufacturer_byte(64), HubType::MoveHub);
    assert_eq!(hub_type_from_manufacturer_byte(65), HubType::Hub);
    assert_eq!(hub_type_from_manufacturer_byte(66), HubType::RemoteControl);
    assert_eq!(
        hub_type_from_manufacturer_byte(128),
        HubType::TechnicMediumHub
    );
    assert_eq!(
        hub_type_from_manufacturer_byte(131),
        HubType::TechnicSmallHub
    );
    assert_eq!(hub_type_from_manufacturer_byte(32), HubType::DuploTrainBase);
    assert_eq!(hub_type_from_manufacturer_byte(99), HubType::Unknown);
}

#[test]
fn test_device_type_from_u16() {
    assert_eq!(DeviceType::from_u16(1), DeviceType::SimpleMediumLinearMotor);
    assert_eq!(DeviceType::from_u16(2), DeviceType::TrainMotor);
    assert_eq!(DeviceType::from_u16(61), DeviceType::TechnicColorSensor);
    assert_eq!(DeviceType::from_u16(63), DeviceType::TechnicForceSensor);
    assert_eq!(DeviceType::from_u16(9999), DeviceType::Unknown);
}

#[test]
fn test_device_type_motor_classification() {
    assert!(DeviceType::TrainMotor.is_motor());
    assert!(DeviceType::TechnicMediumAngularMotor.is_motor());
    assert!(!DeviceType::TechnicColorSensor.is_motor());

    assert!(DeviceType::TechnicMediumAngularMotor.is_tacho_motor());
    assert!(!DeviceType::TrainMotor.is_tacho_motor());

    assert!(DeviceType::TechnicMediumAngularMotor.is_absolute_motor());
    assert!(!DeviceType::MediumLinearMotor.is_absolute_motor());
}

#[test]
fn test_message_type_from_u8() {
    assert_eq!(MessageType::from_u8(0x01), Some(MessageType::HubProperties));
    assert_eq!(MessageType::from_u8(0x04), Some(MessageType::HubAttachedIo));
    assert_eq!(
        MessageType::from_u8(0x45),
        Some(MessageType::PortValueSingle)
    );
    assert_eq!(
        MessageType::from_u8(0x81),
        Some(MessageType::PortOutputCommand)
    );
    assert_eq!(
        MessageType::from_u8(0x82),
        Some(MessageType::PortOutputCommandFeedback)
    );
    assert_eq!(MessageType::from_u8(0xFF), None);
}

#[test]
fn test_hub_type_wedo2() {
    assert!(HubType::WeDo2SmartHub.is_wedo2());
    assert!(!HubType::Hub.is_wedo2());
    assert!(!HubType::TechnicMediumHub.is_wedo2());
}
