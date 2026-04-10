use super::*;

#[test]
fn test_encode_decode_sync() {
    let msg = NetMessage::Sync;
    let encoded = encode(&msg);
    assert_eq!(encoded.trim(), r#"{"type":"sync"}"#);
    let decoded = decode(&encoded).unwrap();
    assert!(matches!(decoded, NetMessage::Sync));
}

#[test]
fn test_encode_decode_set_number() {
    let msg = NetMessage::Set {
        name: "speed".to_string(),
        value: LogoValue::Number(42.0),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "speed");
            assert_eq!(value, LogoValue::Number(42.0));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_encode_decode_set_word() {
    let msg = NetMessage::Set {
        name: "greeting".to_string(),
        value: LogoValue::Word("hello".to_string()),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "greeting");
            assert_eq!(value, LogoValue::Word("hello".to_string()));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_encode_decode_set_list() {
    let msg = NetMessage::Set {
        name: "ports".to_string(),
        value: LogoValue::List(vec![
            LogoValue::Word("a".to_string()),
            LogoValue::Word("b".to_string()),
            LogoValue::Number(3.0),
        ]),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded).unwrap();
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
fn test_encode_decode_set_nested_list() {
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
    let decoded = decode(&encoded).unwrap();
    match decoded {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "nested");
            match value {
                LogoValue::List(items) => {
                    assert_eq!(items.len(), 2);
                    assert_eq!(items[0], LogoValue::Number(1.0));
                    assert_eq!(
                        items[1],
                        LogoValue::List(vec![LogoValue::Number(2.0), LogoValue::Number(3.0)])
                    );
                }
                _ => panic!("Expected List"),
            }
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_encode_decode_snapshot_empty() {
    let msg = NetMessage::Snapshot {
        vars: HashMap::new(),
    };
    let encoded = encode(&msg);
    let decoded = decode(&encoded).unwrap();
    match decoded {
        NetMessage::Snapshot { vars } => assert!(vars.is_empty()),
        _ => panic!("Expected Snapshot"),
    }
}

#[test]
fn test_encode_decode_snapshot_with_vars() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), LogoValue::Number(42.0));
    vars.insert("name".to_string(), LogoValue::Word("robot".to_string()));
    vars.insert(
        "colors".to_string(),
        LogoValue::List(vec![
            LogoValue::Number(1.0),
            LogoValue::Number(2.0),
        ]),
    );
    let msg = NetMessage::Snapshot { vars };
    let encoded = encode(&msg);
    let decoded = decode(&encoded).unwrap();
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
fn test_decode_invalid_json() {
    assert!(decode("not json").is_err());
}

#[test]
fn test_decode_unknown_type() {
    assert!(decode(r#"{"type":"unknown"}"#).is_err());
}

#[test]
fn test_set_number_json_format() {
    let msg = NetMessage::Set {
        name: "x".to_string(),
        value: LogoValue::Number(7.0),
    };
    let encoded = encode(&msg);
    // untagged LogoValue means value is just 7.0, not {"Number":7.0}
    assert!(encoded.contains(r#""value":7.0"#));
}

#[test]
fn test_set_word_json_format() {
    let msg = NetMessage::Set {
        name: "x".to_string(),
        value: LogoValue::Word("hello".to_string()),
    };
    let encoded = encode(&msg);
    assert!(encoded.contains(r#""value":"hello""#));
}

#[test]
fn test_set_list_json_format() {
    let msg = NetMessage::Set {
        name: "x".to_string(),
        value: LogoValue::List(vec![LogoValue::Number(1.0), LogoValue::Number(2.0)]),
    };
    let encoded = encode(&msg);
    assert!(encoded.contains(r#""value":[1.0,2.0]"#));
}
