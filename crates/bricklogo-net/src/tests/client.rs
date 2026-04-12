use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use tungstenite::{accept, Message};
use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};

/// Minimal mock host that handles one WebSocket client.
/// Responds to Hello with Hi (binary if requested), Sync with Snapshot,
/// and collects any Set messages.
fn start_mock_host(port: u16, initial_vars: HashMap<String, LogoValue>) -> (
    Arc<Mutex<Vec<HashMap<String, LogoValue>>>>,  // received sets
) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let recv_clone = received.clone();

    thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        if let Ok((stream, _)) = listener.accept() {
            let mut ws = accept(stream).unwrap();
            ws.get_mut().set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut binary_mode = false;

            loop {
                let msg = match ws.read() {
                    Ok(m) => m,
                    Err(tungstenite::Error::Io(ref e))
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(_) => break,
                };

                let net_msg = match &msg {
                    Message::Text(text) => protocol::decode_json(text).ok(),
                    Message::Binary(data) => protocol::decode_binary(data).ok(),
                    Message::Close(_) => break,
                    _ => None,
                };

                if let Some(net_msg) = net_msg {
                    match net_msg {
                        NetMessage::Hello { binary_protocol, .. } => {
                            binary_mode = binary_protocol;
                            if binary_mode {
                                let _ = ws.send(Message::Binary(protocol::encode_binary(&NetMessage::Hi).into()));
                            } else {
                                let _ = ws.send(Message::Text(protocol::encode_json(&NetMessage::Hi).into()));
                            }
                        }
                        NetMessage::Sync => {
                            let snapshot = NetMessage::Snapshot { vars: initial_vars.clone() };
                            if binary_mode {
                                let _ = ws.send(Message::Binary(protocol::encode_binary(&snapshot).into()));
                            } else {
                                let _ = ws.send(Message::Text(protocol::encode_json(&snapshot).into()));
                            }
                        }
                        NetMessage::Set { vars } => {
                            recv_clone.lock().unwrap().push(vars);
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    thread::sleep(Duration::from_millis(50)); // Let listener bind
    (received,)
}

#[test]
fn test_client_receives_snapshot_on_connect() {
    let mut initial = HashMap::new();
    initial.insert("x".to_string(), LogoValue::Number(99.0));
    initial.insert("msg".to_string(), LogoValue::Word("hello".to_string()));

    let (_received,) = start_mock_host(19760, initial);

    let global_vars = Arc::new(RwLock::new(HashMap::new()));
    let (_tx, rx) = mpsc::channel();
    let system_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|_| {});
    let status = Arc::new(Mutex::new("connecting".to_string()));

    crate::client::start_client(
        "127.0.0.1:19760",
        global_vars.clone(),
        rx,
        system_fn,
        status.clone(),
        None,
    ).unwrap();

    let vars = global_vars.read().unwrap();
    assert_eq!(vars.get("x"), Some(&LogoValue::Number(99.0)));
    assert_eq!(vars.get("msg"), Some(&LogoValue::Word("hello".to_string())));
}

#[test]
fn test_client_sends_local_set_to_host() {
    let (received,) = start_mock_host(19761, HashMap::new());

    let global_vars = Arc::new(RwLock::new(HashMap::new()));
    let (tx, rx) = mpsc::channel();
    let system_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|_| {});
    let status = Arc::new(Mutex::new("connecting".to_string()));

    crate::client::start_client(
        "127.0.0.1:19761",
        global_vars.clone(),
        rx,
        system_fn,
        status.clone(),
        None,
    ).unwrap();

    // Simulate a local make
    tx.send(("speed".to_string(), LogoValue::Number(7.0))).unwrap();
    thread::sleep(Duration::from_millis(200));

    let sets = received.lock().unwrap();
    assert!(sets.iter().any(|v| v.get("speed") == Some(&LogoValue::Number(7.0))));
}

#[test]
fn test_client_connection_refused() {
    let global_vars = Arc::new(RwLock::new(HashMap::new()));
    let (_tx, rx) = mpsc::channel();
    let system_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|_| {});
    let status = Arc::new(Mutex::new("connecting".to_string()));

    let result = crate::client::start_client(
        "127.0.0.1:19799",
        global_vars,
        rx,
        system_fn,
        status,
        None,
    );
    assert!(result.is_err());
}
