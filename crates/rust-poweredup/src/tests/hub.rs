use super::*;

#[test]
fn test_hub_new() {
    let hub = Hub::new(HubType::TechnicMediumHub);
    assert_eq!(hub.hub_type, HubType::TechnicMediumHub);
    assert!(!hub.is_connected());
    assert!(hub.get_attached_devices().is_empty());
}

#[test]
fn test_hub_connect_disconnect() {
    let mut hub = Hub::new(HubType::Hub);
    hub.on_connected();
    assert!(hub.is_connected());
    hub.on_disconnected();
    assert!(!hub.is_connected());
}

#[test]
fn test_process_attach() {
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();

    let msg = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let events = hub.process_message(&msg);

    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        HubEvent::DeviceAttached {
            port_id: 0,
            device_type: DeviceType::TechnicColorSensor,
        }
    );

    let device = hub.get_device(0).unwrap();
    assert_eq!(device.device_type, DeviceType::TechnicColorSensor);
    assert!(device.mode_lookup.contains_key("color"));
}

#[test]
fn test_process_detach() {
    let mut hub = Hub::new(HubType::Hub);
    hub.on_connected();

    let msg = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&msg);
    assert!(hub.get_device(0).is_some());

    let msg = vec![5, 0x00, 0x04, 0x00, 0x00];
    let events = hub.process_message(&msg);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        HubEvent::DeviceDetached { port_id: 0, .. }
    ));
    assert!(hub.get_device(0).is_none());
}

#[test]
fn test_process_sensor_value() {
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();

    let attach = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&attach);
    hub.set_subscribed_mode(0, 0x00);

    let msg = vec![6, 0x00, 0x45, 0x00, 0x03];
    let events = hub.process_message(&msg);

    assert_eq!(events.len(), 1);
    if let HubEvent::SensorValue {
        port_id, reading, ..
    } = &events[0]
    {
        assert_eq!(*port_id, 0);
        assert_eq!(*reading, SensorReading::Number(3.0));
    } else {
        panic!("Expected SensorValue event");
    }

    assert_eq!(hub.last_reading(0), Some(&SensorReading::Number(3.0)));
}

#[test]
fn test_set_subscribed_mode_clears_stale_reading_on_mode_change() {
    // When switching subscription modes, the cached last_reading was
    // captured under the old mode and is meaningless for the new one.
    // set_subscribed_mode must clear it so callers polling last_reading
    // wait for fresh data instead of returning the stale value.
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();
    // Attach a Technic motor (type 0x30 = TechnicLargeAngularMotor) on port 0.
    let attach = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&attach);

    // Subscribe to mode 2 (rotation / POS) and populate a reading.
    hub.set_subscribed_mode(0, 0x02);
    let deg_bytes = 700_i32.to_le_bytes();
    let msg = vec![9, 0x00, 0x45, 0x00, deg_bytes[0], deg_bytes[1], deg_bytes[2], deg_bytes[3]];
    hub.process_message(&msg);
    assert_eq!(hub.last_reading(0), Some(&SensorReading::Number(700.0)));

    // Changing subscription to mode 3 (absolute / APOS) must clear the
    // previous reading.
    hub.set_subscribed_mode(0, 0x03);
    assert_eq!(hub.last_reading(0), None);

    // Re-subscribing to the SAME mode should NOT clear (reading stays valid).
    let apos_bytes = 60_i16.to_le_bytes();
    let apos_msg = vec![7, 0x00, 0x45, 0x00, apos_bytes[0], apos_bytes[1]];
    hub.process_message(&apos_msg);
    assert_eq!(hub.last_reading(0), Some(&SensorReading::Number(60.0)));
    hub.set_subscribed_mode(0, 0x03);
    assert_eq!(hub.last_reading(0), Some(&SensorReading::Number(60.0)));
}

#[test]
fn test_process_motor_rotation() {
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();

    let attach = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&attach);
    hub.set_subscribed_mode(0, 0x02);

    let deg_bytes = 720_i32.to_le_bytes();
    let msg = vec![
        9,
        0x00,
        0x45,
        0x00,
        deg_bytes[0],
        deg_bytes[1],
        deg_bytes[2],
        deg_bytes[3],
    ];
    let events = hub.process_message(&msg);

    assert_eq!(events.len(), 1);
    if let HubEvent::SensorValue { reading, .. } = &events[0] {
        assert_eq!(*reading, SensorReading::Number(720.0));
    }
}

#[test]
fn test_process_command_feedback() {
    let mut hub = Hub::new(HubType::Hub);
    hub.on_connected();

    let msg = vec![5, 0x00, 0x82, 0x00, 0x0a];
    let events = hub.process_message(&msg);

    assert_eq!(events.len(), 1);
    if let HubEvent::CommandFeedback {
        port_id,
        completed,
        discarded,
    } = &events[0]
    {
        assert_eq!(*port_id, 0);
        assert!(*completed);
        assert!(!*discarded);
    }
}

#[test]
fn test_process_battery() {
    let mut hub = Hub::new(HubType::Hub);
    let msg = vec![6, 0x00, 0x01, 0x06, 0x06, 75];
    hub.process_message(&msg);
    assert_eq!(hub.battery, 75);
}

#[test]
fn test_port_name_mapping() {
    let hub = Hub::new(HubType::TechnicMediumHub);
    assert_eq!(hub.port_id_by_name("A"), Some(0));
    assert_eq!(hub.port_id_by_name("B"), Some(1));
    assert_eq!(hub.port_id_by_name("C"), Some(2));
    assert_eq!(hub.port_id_by_name("D"), Some(3));
    assert_eq!(hub.port_id_by_name("a"), Some(0));
    assert_eq!(hub.port_id_by_name("E"), None);
}

#[test]
fn test_port_name_wedo2() {
    let hub = Hub::new(HubType::WeDo2SmartHub);
    assert_eq!(hub.port_id_by_name("A"), Some(1));
    assert_eq!(hub.port_id_by_name("B"), Some(2));
}

#[test]
fn test_mode_for_event() {
    let mut hub = Hub::new(HubType::Hub);
    hub.on_connected();

    let attach = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&attach);

    assert_eq!(hub.mode_for_event(0, "color"), Some(0x00));
    assert_eq!(hub.mode_for_event(0, "light"), Some(0x01));
    assert_eq!(hub.mode_for_event(0, "nonexistent"), None);
    assert_eq!(hub.mode_for_event(99, "color"), None);
}

#[test]
fn test_find_device_by_type() {
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();

    let motor_attach = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&motor_attach);

    let color_attach = vec![
        15, 0x00, 0x04, 0x02, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&color_attach);

    let motor = hub.find_device_by_type(DeviceType::TechnicMediumAngularMotor);
    assert!(motor.is_some());
    assert_eq!(motor.unwrap().port_id, 0);

    let color = hub.find_device_by_type(DeviceType::TechnicColorSensor);
    assert!(color.is_some());
    assert_eq!(color.unwrap().port_id, 2);

    assert!(
        hub.find_device_by_type(DeviceType::TechnicForceSensor)
            .is_none()
    );
}

#[test]
fn test_get_device_at_port() {
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();

    let attach = vec![
        15, 0x00, 0x04, 0x02, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&attach);

    let device = hub.get_device_at_port("C");
    assert!(device.is_some());
    assert_eq!(device.unwrap().device_type, DeviceType::TechnicColorSensor);

    assert!(hub.get_device_at_port("A").is_none());
}

#[test]
fn test_wedo2_port_type() {
    let mut hub = Hub::new(HubType::WeDo2SmartHub);
    hub.on_connected();

    let msg = vec![1, 0x01, 0x00, 34];
    let events = hub.process_wedo2_port_type(&msg);

    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        HubEvent::DeviceAttached {
            port_id: 1,
            device_type: DeviceType::TiltSensor,
        }
    );
}

#[test]
fn test_no_sensor_value_without_subscription() {
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();

    let attach = vec![
        15, 0x00, 0x04, 0x00, 0x01, 0x3d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    hub.process_message(&attach);

    let msg = vec![6, 0x00, 0x45, 0x00, 0x03];
    let events = hub.process_message(&msg);
    assert!(events.is_empty());
}

#[test]
fn test_virtual_port_attach() {
    let mut hub = Hub::new(HubType::TechnicMediumHub);
    hub.on_connected();

    let msg = vec![9, 0x00, 0x04, 0x10, 0x02, 0x30, 0x00, 0x00, 0x01];
    let events = hub.process_message(&msg);

    assert_eq!(events.len(), 1);
    if let HubEvent::VirtualDeviceAttached {
        port_id,
        first_port,
        second_port,
        ..
    } = &events[0]
    {
        assert_eq!(*port_id, 0x10);
        assert_eq!(*first_port, 0x00);
        assert_eq!(*second_port, 0x01);
    }
}
