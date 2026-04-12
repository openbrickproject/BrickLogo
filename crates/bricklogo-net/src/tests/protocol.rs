use super::*;

// ── JSON tests ──────────────────────────────────

#[test]
fn test_json_hello_minimal() {
    let msg = NetMessage::Hello { password: None, binary_protocol: false };
    let encoded = encode_json(&msg);
    assert!(encoded.contains(r#""type":"hello""#));
    assert!(!encoded.contains("password"));
    assert!(!encoded.contains("binaryProtocol"));
    let decoded = decode_json(&encoded).unwrap();
    match decoded {
        NetMessage::Hello { password, binary_protocol } => {
            assert!(password.is_none());
            assert!(!binary_protocol);
        }
        _ => panic!("Expected Hello"),
    }
}

#[test]
fn test_json_hello_with_password() {
    let msg = NetMessage::Hello { password: Some("secret".to_string()), binary_protocol: false };
    let encoded = encode_json(&msg);
    assert!(encoded.contains("secret"));
    let decoded = decode_json(&encoded).unwrap();
    match decoded {
        NetMessage::Hello { password, .. } => assert_eq!(password, Some("secret".to_string())),
        _ => panic!("Expected Hello"),
    }
}

#[test]
fn test_json_hello_with_binary_protocol() {
    let msg = NetMessage::Hello { password: None, binary_protocol: true };
    let encoded = encode_json(&msg);
    assert!(encoded.contains(r#""binaryProtocol":true"#));
    let decoded = decode_json(&encoded).unwrap();
    match decoded {
        NetMessage::Hello { binary_protocol, .. } => assert!(binary_protocol),
        _ => panic!("Expected Hello"),
    }
}

#[test]
fn test_json_hello_with_password_and_binary() {
    let msg = NetMessage::Hello { password: Some("test".to_string()), binary_protocol: true };
    let encoded = encode_json(&msg);
    let decoded = decode_json(&encoded).unwrap();
    match decoded {
        NetMessage::Hello { password, binary_protocol } => {
            assert_eq!(password, Some("test".to_string()));
            assert!(binary_protocol);
        }
        _ => panic!("Expected Hello"),
    }
}

#[test]
fn test_json_hi() {
    let msg = NetMessage::Hi;
    let encoded = encode_json(&msg);
    let decoded = decode_json(&encoded).unwrap();
    assert!(matches!(decoded, NetMessage::Hi));
}

#[test]
fn test_json_sync() {
    let msg = NetMessage::Sync;
    let encoded = encode_json(&msg);
    let decoded = decode_json(&encoded).unwrap();
    assert!(matches!(decoded, NetMessage::Sync));
}

#[test]
fn test_json_snapshot() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), LogoValue::Number(42.0));
    vars.insert("name".to_string(), LogoValue::Word("robot".to_string()));
    let msg = NetMessage::Snapshot { vars };
    let encoded = encode_json(&msg);
    let decoded = decode_json(&encoded).unwrap();
    match decoded {
        NetMessage::Snapshot { vars } => {
            assert_eq!(vars["x"], LogoValue::Number(42.0));
            assert_eq!(vars["name"], LogoValue::Word("robot".to_string()));
        }
        _ => panic!("Expected Snapshot"),
    }
}

#[test]
fn test_json_set_single() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), LogoValue::Number(7.0));
    let msg = NetMessage::Set { vars };
    let encoded = encode_json(&msg);
    let decoded = decode_json(&encoded).unwrap();
    match decoded {
        NetMessage::Set { vars } => {
            assert_eq!(vars.len(), 1);
            assert_eq!(vars["x"], LogoValue::Number(7.0));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_json_set_batch() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), LogoValue::Number(1.0));
    vars.insert("y".to_string(), LogoValue::Number(2.0));
    vars.insert("z".to_string(), LogoValue::Word("hello".to_string()));
    let msg = NetMessage::Set { vars };
    let encoded = encode_json(&msg);
    let decoded = decode_json(&encoded).unwrap();
    match decoded {
        NetMessage::Set { vars } => {
            assert_eq!(vars.len(), 3);
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_json_decode_invalid() {
    assert!(decode_json("not json").is_err());
}

#[test]
fn test_json_decode_unknown_type() {
    assert!(decode_json(r#"{"type":"unknown"}"#).is_err());
}

// ── Binary tests ────────────────────────────────

#[test]
fn test_binary_hello_minimal() {
    let msg = NetMessage::Hello { password: None, binary_protocol: false };
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    match decoded {
        NetMessage::Hello { password, binary_protocol } => {
            assert!(password.is_none());
            assert!(!binary_protocol);
        }
        _ => panic!("Expected Hello"),
    }
}

#[test]
fn test_binary_hello_with_password() {
    let msg = NetMessage::Hello { password: Some("secret".to_string()), binary_protocol: false };
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    match decoded {
        NetMessage::Hello { password, binary_protocol } => {
            assert_eq!(password, Some("secret".to_string()));
            assert!(!binary_protocol);
        }
        _ => panic!("Expected Hello"),
    }
}

#[test]
fn test_binary_hello_with_binary_protocol() {
    let msg = NetMessage::Hello { password: None, binary_protocol: true };
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    match decoded {
        NetMessage::Hello { password, binary_protocol } => {
            assert!(password.is_none());
            assert!(binary_protocol);
        }
        _ => panic!("Expected Hello"),
    }
}

#[test]
fn test_binary_hi() {
    let msg = NetMessage::Hi;
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    assert!(matches!(decoded, NetMessage::Hi));
}

#[test]
fn test_binary_sync() {
    let msg = NetMessage::Sync;
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    assert!(matches!(decoded, NetMessage::Sync));
}

#[test]
fn test_binary_snapshot() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), LogoValue::Number(42.0));
    vars.insert("name".to_string(), LogoValue::Word("robot".to_string()));
    vars.insert("colors".to_string(), LogoValue::List(vec![
        LogoValue::Number(1.0), LogoValue::Number(2.0),
    ]));
    let msg = NetMessage::Snapshot { vars };
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    match decoded {
        NetMessage::Snapshot { vars } => {
            assert_eq!(vars.len(), 3);
            assert_eq!(vars["x"], LogoValue::Number(42.0));
        }
        _ => panic!("Expected Snapshot"),
    }
}

#[test]
fn test_binary_set_single() {
    let mut vars = HashMap::new();
    vars.insert("speed".to_string(), LogoValue::Number(5.0));
    let msg = NetMessage::Set { vars };
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    match decoded {
        NetMessage::Set { vars } => {
            assert_eq!(vars["speed"], LogoValue::Number(5.0));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_binary_set_batch() {
    let mut vars = HashMap::new();
    vars.insert("a".to_string(), LogoValue::Number(1.0));
    vars.insert("b".to_string(), LogoValue::Word("hello".to_string()));
    let msg = NetMessage::Set { vars };
    let encoded = encode_binary(&msg);
    let decoded = decode_binary(&encoded).unwrap();
    match decoded {
        NetMessage::Set { vars } => {
            assert_eq!(vars.len(), 2);
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_binary_decode_empty_fails() {
    assert!(decode_binary(&[]).is_err());
}

#[test]
fn test_binary_decode_unknown_opcode_fails() {
    assert!(decode_binary(&[0, 0, 0, 1, 0xFF]).is_err());
}
