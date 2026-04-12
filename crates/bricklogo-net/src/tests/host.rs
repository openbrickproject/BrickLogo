use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use tungstenite::{connect, Message};
use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};

type WsClient = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn ws_set_read_timeout(ws: &mut WsClient, timeout: Option<Duration>) {
    match ws.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(s) => {
            s.set_read_timeout(timeout).unwrap();
        }
        _ => {}
    }
}

fn start_test_host(port: u16, password_hash: Option<String>) -> (
    Arc<RwLock<HashMap<String, LogoValue>>>,
    mpsc::Sender<(String, LogoValue)>,
    Arc<Mutex<Vec<String>>>,
    Arc<Mutex<String>>,
) {
    let global_vars = Arc::new(RwLock::new(HashMap::new()));
    let (tx, rx) = mpsc::channel();
    let log = Arc::new(Mutex::new(Vec::new()));
    let log_clone = log.clone();
    let system_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |msg: &str| {
        log_clone.lock().unwrap().push(msg.to_string());
    });
    let status = Arc::new(Mutex::new("hosting (0 clients)".to_string()));

    super::start_host(port, global_vars.clone(), rx, system_fn, status.clone(), password_hash).unwrap();
    thread::sleep(Duration::from_millis(50)); // Let listener start

    (global_vars, tx, log, status)
}

/// JSON connect: hello then sync (no password)
fn ws_json_connect(port: u16) -> WsClient {
    let url = format!("ws://127.0.0.1:{}", port);
    let (mut ws, _) = connect(&url).unwrap();

    // Send hello
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Hello {
        password: None,
        binary_protocol: false,
    }).into())).unwrap();

    // Read hi
    let msg = ws.read().unwrap();
    match msg {
        Message::Text(text) => {
            assert!(matches!(protocol::decode_json(&text).unwrap(), NetMessage::Hi));
        }
        _ => panic!("Expected text hi"),
    }

    // Send sync
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Sync).into())).unwrap();

    // Read snapshot
    let msg = ws.read().unwrap();
    match msg {
        Message::Text(text) => {
            assert!(matches!(protocol::decode_json(&text).unwrap(), NetMessage::Snapshot { .. }));
        }
        _ => panic!("Expected text snapshot"),
    }

    ws
}

#[test]
fn test_host_hello_then_sync_works() {
    let (_vars, _tx, _log, _status) = start_test_host(19750, None);
    let _ws = ws_json_connect(19750);
}

#[test]
fn test_host_sync_without_hello_disconnects() {
    let (_vars, _tx, _log, _status) = start_test_host(19770, None);
    let url = "ws://127.0.0.1:19770";
    let (mut ws, _) = connect(url).unwrap();

    // Send sync without hello
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Sync).into())).unwrap();

    thread::sleep(Duration::from_millis(100));
    ws_set_read_timeout(&mut ws, Some(Duration::from_millis(200)));
    assert!(ws.read().is_err());
}

#[test]
fn test_host_sends_snapshot_with_vars() {
    let (vars, _tx, _log, _status) = start_test_host(19751, None);
    vars.write().unwrap().insert("x".to_string(), LogoValue::Number(42.0));

    let url = "ws://127.0.0.1:19751";
    let (mut ws, _) = connect(url).unwrap();

    // Hello first
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Hello {
        password: None, binary_protocol: false,
    }).into())).unwrap();
    let _ = ws.read().unwrap(); // Hi

    // Then sync
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Sync).into())).unwrap();
    let msg = ws.read().unwrap();
    match msg {
        Message::Text(text) => {
            match protocol::decode_json(&text).unwrap() {
                NetMessage::Snapshot { vars } => {
                    assert_eq!(vars["x"], LogoValue::Number(42.0));
                }
                _ => panic!("Expected Snapshot"),
            }
        }
        _ => panic!("Expected text message"),
    }
}

#[test]
fn test_host_broadcasts_local_set() {
    let (_vars, tx, _log, _status) = start_test_host(19752, None);
    let mut ws = ws_json_connect(19752);

    // Broadcast a local variable change
    tx.send(("speed".to_string(), LogoValue::Number(5.0))).unwrap();

    // Client should receive it
    let msg = ws.read().unwrap();
    match msg {
        Message::Text(text) => {
            match protocol::decode_json(&text).unwrap() {
                NetMessage::Set { vars } => {
                    assert_eq!(vars["speed"], LogoValue::Number(5.0));
                }
                _ => panic!("Expected Set"),
            }
        }
        _ => panic!("Expected text message"),
    }
}

#[test]
fn test_host_propagates_client_set() {
    let (vars, _tx, _log, _status) = start_test_host(19753, None);

    let mut ws_a = ws_json_connect(19753);
    let mut ws_b = ws_json_connect(19753);

    // Client A sends a set
    let mut set_vars = HashMap::new();
    set_vars.insert("color".to_string(), LogoValue::Word("red".to_string()));
    ws_a.send(Message::Text(protocol::encode_json(&NetMessage::Set { vars: set_vars }).into())).unwrap();

    thread::sleep(Duration::from_millis(100));

    // Host should have the variable
    assert_eq!(
        vars.read().unwrap().get("color"),
        Some(&LogoValue::Word("red".to_string()))
    );

    // Client B should receive it
    let msg = ws_b.read().unwrap();
    match msg {
        Message::Text(text) => {
            match protocol::decode_json(&text).unwrap() {
                NetMessage::Set { vars } => {
                    assert_eq!(vars["color"], LogoValue::Word("red".to_string()));
                }
                _ => panic!("Expected Set"),
            }
        }
        _ => panic!("Expected text message"),
    }

    // Client A should NOT receive its own set back
    ws_set_read_timeout(&mut ws_a, Some(Duration::from_millis(200)));
    assert!(ws_a.read().is_err());
}

#[test]
fn test_host_password_correct_auth() {
    let (_vars, _tx, _log, _status) = start_test_host(19756, Some("secret".to_string()));

    let url = "ws://127.0.0.1:19756";
    let (mut ws, _) = connect(url).unwrap();

    // Send hello with correct password
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Hello {
        password: Some("secret".to_string()),
        binary_protocol: false,
    }).into())).unwrap();

    // Should get hi back
    let msg = ws.read().unwrap();
    match msg {
        Message::Text(text) => {
            assert!(matches!(protocol::decode_json(&text).unwrap(), NetMessage::Hi));
        }
        _ => panic!("Expected text hi"),
    }
}

#[test]
fn test_host_password_wrong_auth_disconnects() {
    let (_vars, _tx, _log, _status) = start_test_host(19757, Some("secret".to_string()));

    let url = "ws://127.0.0.1:19757";
    let (mut ws, _) = connect(url).unwrap();

    // Send hello with wrong password
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Hello {
        password: Some("wrong".to_string()),
        binary_protocol: false,
    }).into())).unwrap();

    // Connection should be closed
    thread::sleep(Duration::from_millis(100));
    ws_set_read_timeout(&mut ws, Some(Duration::from_millis(200)));
    assert!(ws.read().is_err());
}


#[test]
fn test_host_binary_mode_switch() {
    let (_vars, _tx, _log, _status) = start_test_host(19759, None);

    let url = "ws://127.0.0.1:19759";
    let (mut ws, _) = connect(url).unwrap();

    // Send hello with binaryProtocol=true
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Hello {
        password: None,
        binary_protocol: true,
    }).into())).unwrap();

    // Should get binary hi back
    let msg = ws.read().unwrap();
    assert!(matches!(msg, Message::Binary(_)));
    match msg {
        Message::Binary(data) => {
            assert!(matches!(protocol::decode_binary(&data).unwrap(), NetMessage::Hi));
        }
        _ => panic!("Expected binary hi"),
    }
}

#[test]
fn test_host_logs_client_events() {
    let (_vars, _tx, log, _status) = start_test_host(19754, None);

    let url = "ws://127.0.0.1:19754";
    let (mut ws, _) = connect(url).unwrap();

    // Send hello to complete connection
    ws.send(Message::Text(protocol::encode_json(&NetMessage::Hello {
        password: None, binary_protocol: false,
    }).into())).unwrap();
    let _ = ws.read().unwrap(); // Hi

    thread::sleep(Duration::from_millis(100));

    // Should have logged the connection
    let msgs = log.lock().unwrap();
    assert!(msgs.iter().any(|m| m.contains("joined")));
    drop(msgs);

    // Disconnect
    drop(ws);
    thread::sleep(Duration::from_millis(200));

    let msgs = log.lock().unwrap();
    assert!(msgs.iter().any(|m| m.contains("disconnected")));
}
