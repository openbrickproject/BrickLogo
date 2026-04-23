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

// ── Connection state transitions ────────────────

#[test]
fn test_set_power_before_connect_errors() {
    let mut wedo = WeDo::new();
    let err = wedo.set_power("a", 50).unwrap_err();
    assert!(!err.is_empty());
}

#[test]
fn test_poll_sensors_before_connect_errors() {
    let mut wedo = WeDo::new();
    assert!(wedo.poll_sensors().is_err());
}

#[test]
fn test_disconnect_on_never_connected_is_noop() {
    let mut wedo = WeDo::new();
    wedo.disconnect();
    assert!(!wedo.is_connected());
}

#[test]
fn test_with_id_tags_target() {
    let wedo = WeDo::with_id("serial-1234");
    // Target ID should be remembered so `connect` can match it later.
    assert_eq!(wedo.target_id, Some("serial-1234".to_string()));
}

// ── Sensor reading edge cases ───────────────────

#[test]
fn test_sensor_cache_returns_most_recent_value() {
    let mut wedo = WeDo::new();
    let older = SensorNotification {
        samples: vec![SensorSample {
            port: "A".to_string(),
            raw_value: 50,
            sensor_type_id: 180,
            sensor_type: SensorType::Distance,
        }],
    };
    let newer = SensorNotification {
        samples: vec![SensorSample {
            port: "A".to_string(),
            raw_value: 90,
            sensor_type_id: 180,
            sensor_type: SensorType::Distance,
        }],
    };
    wedo.process_sensor_notification(&older);
    wedo.process_sensor_notification(&newer);

    let dist = wedo.read("A", "distance").unwrap();
    if let WeDoSensorPayload::Distance(d) = dist {
        assert_eq!(d.raw_value, 90, "cache should retain the newer sample");
    } else {
        panic!("expected Distance");
    }
}

#[test]
fn test_read_wrong_event_type_returns_none() {
    // Sensor present on port A is a Distance sensor; a caller asking
    // for "tilt" should get None rather than a misleading cached value.
    let mut wedo = WeDo::new();
    wedo.process_sensor_notification(&SensorNotification {
        samples: vec![SensorSample {
            port: "A".to_string(),
            raw_value: 100,
            sensor_type_id: 180,
            sensor_type: SensorType::Distance,
        }],
    });
    assert!(wedo.read("A", "tilt").is_none());
}

#[test]
fn test_sensor_types_are_per_port() {
    let mut wedo = WeDo::new();
    wedo.process_sensor_notification(&SensorNotification {
        samples: vec![
            SensorSample {
                port: "A".to_string(),
                raw_value: 60,
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
    });
    assert!(wedo.read("A", "distance").is_some());
    assert!(wedo.read("B", "tilt").is_some());
    // Cross-port queries should miss.
    assert!(wedo.read("A", "tilt").is_none());
    assert!(wedo.read("B", "distance").is_none());
}

// ── Port name validation ────────────────────────

#[test]
fn test_normalize_port_rejects_empty() {
    let wedo = WeDo::new();
    assert!(wedo.normalize_port("").is_err());
}

#[test]
fn test_normalize_port_rejects_digits() {
    let wedo = WeDo::new();
    assert!(wedo.normalize_port("1").is_err());
}
