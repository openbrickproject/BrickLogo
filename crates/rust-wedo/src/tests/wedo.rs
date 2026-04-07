use super::*;

#[test]
fn test_new() {
    let wedo = WeDo::new();
    assert_eq!(wedo.state(), WeDoState::NotReady);
    assert!(!wedo.is_connected());
}

#[test]
fn test_with_path() {
    let wedo = WeDo::with_path("/dev/hidraw0");
    assert_eq!(wedo.target_path, Some("/dev/hidraw0".to_string()));
}

#[test]
fn test_normalize_port() {
    let wedo = WeDo::new();
    assert_eq!(wedo.normalize_port("a").unwrap(), 0);
    assert_eq!(wedo.normalize_port("A").unwrap(), 0);
    assert_eq!(wedo.normalize_port("b").unwrap(), 1);
    assert_eq!(wedo.normalize_port("B").unwrap(), 1);
    assert!(wedo.normalize_port("c").is_err());
}

#[test]
fn test_sensor_cache() {
    let mut wedo = WeDo::new();
    let notification = SensorNotification {
        samples: vec![
            SensorSample {
                port: "A".to_string(),
                raw_value: 150,
                sensor_type_id: 180,
                sensor_type: SensorType::Distance,
            },
            SensorSample {
                port: "B".to_string(),
                raw_value: 120,
                sensor_type_id: 35,
                sensor_type: SensorType::Tilt,
            },
        ],
    };
    wedo.process_sensor_notification(&notification);

    let dist = wedo.read("A", "distance");
    assert!(dist.is_some());
    if let Some(WeDoSensorPayload::Distance(d)) = dist {
        assert_eq!(d.port, "A");
        assert!(d.distance > 0);
    }

    let tilt = wedo.read("B", "tilt");
    assert!(tilt.is_some());
    if let Some(WeDoSensorPayload::Tilt(t)) = tilt {
        assert_eq!(t.port, "B");
        assert_eq!(t.tilt, TiltEvent::Level);
    }
}

#[test]
fn test_read_missing() {
    let wedo = WeDo::new();
    assert!(wedo.read("A", "distance").is_none());
}
