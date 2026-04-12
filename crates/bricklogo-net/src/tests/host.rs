use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use bricklogo_lang::value::LogoValue;
use crate::protocol::{NetMessage, write_message, read_message};

fn start_test_host(port: u16) -> (
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

    super::start_host(port, global_vars.clone(), rx, system_fn, status.clone()).unwrap();
    thread::sleep(Duration::from_millis(50)); // Let listener start

    (global_vars, tx, log, status)
}

fn connect_and_sync(port: u16) -> TcpStream {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

    // Send sync
    write_message(&mut stream, &NetMessage::Sync).unwrap();

    // Read snapshot
    let msg = read_message(&mut stream).unwrap();
    assert!(matches!(msg, NetMessage::Snapshot { .. }));

    stream
}

#[test]
fn test_host_accepts_connection_and_sends_snapshot() {
    let (vars, _tx, _log, _status) = start_test_host(19750);
    vars.write().unwrap().insert("x".to_string(), LogoValue::Number(42.0));

    let mut stream = TcpStream::connect("127.0.0.1:19750").unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

    write_message(&mut stream, &NetMessage::Sync).unwrap();
    let msg = read_message(&mut stream).unwrap();
    match msg {
        NetMessage::Snapshot { vars } => {
            assert_eq!(vars["x"], LogoValue::Number(42.0));
        }
        _ => panic!("Expected Snapshot"),
    }
}

#[test]
fn test_host_empty_snapshot() {
    let (_vars, _tx, _log, _status) = start_test_host(19751);
    // The snapshot in connect_and_sync was empty — the assert!(matches!) confirms it was a Snapshot
    let _stream = connect_and_sync(19751);
}

#[test]
fn test_host_broadcasts_local_set() {
    let (_vars, tx, _log, _status) = start_test_host(19752);
    let mut stream = connect_and_sync(19752);

    // Broadcast a local variable change
    tx.send(("speed".to_string(), LogoValue::Number(5.0))).unwrap();

    // Client should receive it
    let msg = read_message(&mut stream).unwrap();
    match msg {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "speed");
            assert_eq!(value, LogoValue::Number(5.0));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_host_propagates_client_set_to_other_clients() {
    let (vars, _tx, _log, _status) = start_test_host(19753);

    let mut stream_a = connect_and_sync(19753);
    let mut stream_b = connect_and_sync(19753);

    // Client A sends a set
    write_message(&mut stream_a, &NetMessage::Set {
        name: "color".to_string(),
        value: LogoValue::Word("red".to_string()),
    }).unwrap();

    thread::sleep(Duration::from_millis(100));

    // Host should have the variable
    assert_eq!(
        vars.read().unwrap().get("color"),
        Some(&LogoValue::Word("red".to_string()))
    );

    // Client B should receive it
    let msg = read_message(&mut stream_b).unwrap();
    match msg {
        NetMessage::Set { name, value } => {
            assert_eq!(name, "color");
            assert_eq!(value, LogoValue::Word("red".to_string()));
        }
        _ => panic!("Expected Set"),
    }

    // Client A should NOT receive its own set back
    stream_a.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    assert!(read_message(&mut stream_a).is_err());
}

#[test]
fn test_host_logs_client_events() {
    let (_vars, _tx, log, _status) = start_test_host(19754);

    let stream = TcpStream::connect("127.0.0.1:19754").unwrap();
    thread::sleep(Duration::from_millis(100));

    // Should have logged the connection
    let msgs = log.lock().unwrap();
    assert!(msgs.iter().any(|m| m.contains("joined")));
    drop(msgs);

    // Disconnect
    drop(stream);
    thread::sleep(Duration::from_millis(200));

    let msgs = log.lock().unwrap();
    assert!(msgs.iter().any(|m| m.contains("disconnected")));
}
