use super::*;

#[test]
fn test_sync_round_trip() {
    let encoded = encode(&NetMessage::Sync);
    let decoded = decode(&encoded[4..]).unwrap();
    assert!(matches!(decoded, NetMessage::Sync));
}

#[test]
fn test_set_number_round_trip() {
    let msg = NetMessage::Set {
        name: "speed".to_string(),
        value: LogoValue::Number(42.0),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded[4..]).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "speed");
            assert_eq!(value, LogoValue::Number(42.0));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_set_word_round_trip() {
    let msg = NetMessage::Set {
        name: "greeting".to_string(),
        value: LogoValue::Word("hello".to_string()),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded[4..]).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "greeting");
            assert_eq!(value, LogoValue::Word("hello".to_string()));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_set_list_round_trip() {
    let msg = NetMessage::Set {
        name: "ports".to_string(),
        value: LogoValue::List(vec![
            LogoValue::Word("a".to_string()),
            LogoValue::Word("b".to_string()),
            LogoValue::Number(3.0),
        ]),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded[4..]).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "ports");
            assert_eq!(
                value,
                LogoValue::List(vec![
                    LogoValue::Word("a".to_string()),
                    LogoValue::Word("b".to_string()),
                    LogoValue::Number(3.0),
                ])
            );
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_set_nested_list_round_trip() {
    let msg = NetMessage::Set {
        name: "nested".to_string(),
        value: LogoValue::List(vec![
            LogoValue::Number(1.0),
            LogoValue::List(vec![
                LogoValue::Number(2.0),
                LogoValue::Number(3.0),
            ]),
        ]),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded[4..]).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "nested");
            assert_eq!(
                value,
                LogoValue::List(vec![
                    LogoValue::Number(1.0),
                    LogoValue::List(vec![LogoValue::Number(2.0), LogoValue::Number(3.0)]),
                ])
            );
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_snapshot_empty_round_trip() {
    let msg = NetMessage::Snapshot {
        vars: HashMap::new(),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded[4..]).unwrap();
    match decoded {
        NetMessage::Snapshot { vars } => assert!(vars.is_empty()),
        _ => panic!("Expected Snapshot"),
    }
}

#[test]
fn test_snapshot_with_vars_round_trip() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), LogoValue::Number(42.0));
    vars.insert("name".to_string(), LogoValue::Word("robot".to_string()));
    vars.insert(
        "colors".to_string(),
        LogoValue::List(vec![LogoValue::Number(1.0), LogoValue::Number(2.0)]),
    );
    let msg = NetMessage::Snapshot { vars };
    let encoded = encode(&msg);
    let decoded = decode(&encoded[4..]).unwrap();
    match decoded {
        NetMessage::Snapshot { vars } => {
            assert_eq!(vars.len(), 3);
            assert_eq!(vars["x"], LogoValue::Number(42.0));
            assert_eq!(vars["name"], LogoValue::Word("robot".to_string()));
            assert_eq!(
                vars["colors"],
                LogoValue::List(vec![LogoValue::Number(1.0), LogoValue::Number(2.0)])
            );
        }
        _ => panic!("Expected Snapshot"),
    }
}

#[test]
fn test_decode_empty_fails() {
    assert!(decode(&[]).is_err());
}

#[test]
fn test_decode_unknown_opcode_fails() {
    assert!(decode(&[0xFF]).is_err());
}

#[test]
fn test_stream_round_trip() {
    let msg = NetMessage::Set {
        name: "x".to_string(),
        value: LogoValue::Number(7.0),
    };
    let mut buf = Vec::new();
    write_message(&mut buf, &msg).unwrap();
    let decoded = read_message(&mut &buf[..]).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "x");
            assert_eq!(value, LogoValue::Number(7.0));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_stream_multiple_messages() {
    let mut buf = Vec::new();
    write_message(&mut buf, &NetMessage::Sync).unwrap();
    write_message(&mut buf, &NetMessage::Set {
        name: "a".to_string(),
        value: LogoValue::Number(1.0),
    }).unwrap();

    let mut cursor = &buf[..];
    let msg1 = read_message(&mut cursor).unwrap();
    let msg2 = read_message(&mut cursor).unwrap();
    assert!(matches!(msg1, NetMessage::Sync));
    assert!(matches!(msg2, NetMessage::Set { .. }));
}

#[test]
fn test_negative_number() {
    let msg = NetMessage::Set {
        name: "x".to_string(),
        value: LogoValue::Number(-3.14),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded[4..]).unwrap();
    match decoded {
        NetMessage::Set { value, .. } => {
            assert_eq!(value, LogoValue::Number(-3.14));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_message_reader_single() {
    let mut buf = Vec::new();
    write_message(&mut buf, &NetMessage::Set {
        name: "x".to_string(),
        value: LogoValue::Number(42.0),
    }).unwrap();

    let mut reader = MessageReader::new();
    let msg = reader.read(&mut &buf[..]).unwrap();
    match msg {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "x");
            assert_eq!(value, LogoValue::Number(42.0));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_message_reader_multiple_in_one_read() {
    // Simulate multiple messages arriving in a single TCP read
    let mut buf = Vec::new();
    write_message(&mut buf, &NetMessage::Sync).unwrap();
    write_message(&mut buf, &NetMessage::Set {
        name: "a".to_string(),
        value: LogoValue::Number(1.0),
    }).unwrap();
    write_message(&mut buf, &NetMessage::Set {
        name: "b".to_string(),
        value: LogoValue::Number(2.0),
    }).unwrap();

    let mut reader = MessageReader::new();
    let mut cursor = &buf[..];
    let msg1 = reader.read(&mut cursor).unwrap();
    let msg2 = reader.read(&mut cursor).unwrap();
    let msg3 = reader.read(&mut cursor).unwrap();

    assert!(matches!(msg1, NetMessage::Sync));
    match msg2 {
        NetMessage::Set { name, .. } => assert_eq!(name, "a"),
        _ => panic!("Expected Set"),
    }
    match msg3 {
        NetMessage::Set { name, .. } => assert_eq!(name, "b"),
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_message_reader_eof() {
    let buf: Vec<u8> = Vec::new();
    let mut reader = MessageReader::new();
    assert!(reader.read(&mut &buf[..]).is_err());
}
