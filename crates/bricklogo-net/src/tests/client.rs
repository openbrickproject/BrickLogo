use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use bricklogo_lang::value::LogoValue;
use crate::protocol::{NetMessage, write_message, read_message};

/// Minimal mock host that handles one client: responds to Sync with a snapshot,
/// and collects any Set messages the client sends.
fn start_mock_host(port: u16, initial_vars: HashMap<String, LogoValue>) -> (
    Arc<Mutex<Vec<(String, LogoValue)>>>,  // received sets
    Arc<Mutex<Option<std::net::TcpStream>>>,  // client write handle
) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let client_handle: Arc<Mutex<Option<std::net::TcpStream>>> = Arc::new(Mutex::new(None));
    let recv_clone = received.clone();
    let handle_clone = client_handle.clone();

    thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        if let Ok((mut stream, _)) = listener.accept() {
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let writer = stream.try_clone().unwrap();
            *handle_clone.lock().unwrap() = Some(writer);

            loop {
                match read_message(&mut stream) {
                    Ok(NetMessage::Sync) => {
                        if let Some(ref mut w) = handle_clone.lock().unwrap().as_mut() {
                            let _ = write_message(*w, &NetMessage::Snapshot {
                                vars: initial_vars.clone(),
                            });
                        }
                    }
                    Ok(NetMessage::Set { name, value }) => {
                        recv_clone.lock().unwrap().push((name, value));
                    }
                    _ => break,
                }
            }
        }
    });

    thread::sleep(Duration::from_millis(50)); // Let listener bind
    (received, client_handle)
}

#[test]
fn test_client_receives_snapshot_on_connect() {
    let mut initial = HashMap::new();
    initial.insert("x".to_string(), LogoValue::Number(99.0));
    initial.insert("msg".to_string(), LogoValue::Word("hello".to_string()));

    let (_received, _handle) = start_mock_host(19760, initial);

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
    ).unwrap();

    let vars = global_vars.read().unwrap();
    assert_eq!(vars.get("x"), Some(&LogoValue::Number(99.0)));
    assert_eq!(vars.get("msg"), Some(&LogoValue::Word("hello".to_string())));
}

#[test]
fn test_client_sends_local_set_to_host() {
    let (received, _handle) = start_mock_host(19761, HashMap::new());

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
    ).unwrap();

    // Simulate a local make
    tx.send(("speed".to_string(), LogoValue::Number(7.0))).unwrap();
    thread::sleep(Duration::from_millis(200));

    let sets = received.lock().unwrap();
    assert!(sets.iter().any(|(n, v)| n == "speed" && *v == LogoValue::Number(7.0)));
}

#[test]
fn test_client_receives_remote_set() {
    let (_received, handle) = start_mock_host(19762, HashMap::new());

    let global_vars = Arc::new(RwLock::new(HashMap::new()));
    let (_tx, rx) = mpsc::channel();
    let system_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|_| {});
    let status = Arc::new(Mutex::new("connecting".to_string()));

    crate::client::start_client(
        "127.0.0.1:19762",
        global_vars.clone(),
        rx,
        system_fn,
        status.clone(),
    ).unwrap();

    // Host sends a set to the client
    thread::sleep(Duration::from_millis(100));
    if let Some(ref mut w) = *handle.lock().unwrap() {
        write_message(w, &NetMessage::Set {
            name: "color".to_string(),
            value: LogoValue::Word("blue".to_string()),
        }).unwrap();
    }

    thread::sleep(Duration::from_millis(200));
    let vars = global_vars.read().unwrap();
    assert_eq!(vars.get("color"), Some(&LogoValue::Word("blue".to_string())));
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
    );
    assert!(result.is_err());
}
