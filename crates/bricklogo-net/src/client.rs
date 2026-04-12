use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use tungstenite::{Message, connect};
use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};
use crate::NetStatus;

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
const READ_TIMEOUT_MS: u64 = 10;

type WsStream = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>;

fn connect_and_sync(
    addr: &str,
    global_vars: &Arc<RwLock<HashMap<String, LogoValue>>>,
    password: &Option<String>,
) -> Result<WsStream, String> {
    let url = format!("ws://{}", addr);
    let (mut ws, _) = connect(&url)
        .map_err(|e| format!("Failed to join {}: {}", addr, e))?;

    // Send hello with auth and binary protocol request
    let hello = NetMessage::Hello {
        password: password.clone(),
        binary_protocol: true,
    };
    ws.send(Message::Text(protocol::encode_json(&hello).into()))
        .map_err(|e| format!("Failed to send hello: {}", e))?;

    // Expect binary Hi back
    let msg = ws.read()
        .map_err(|e| format!("Failed to read hi: {}", e))?;
    match &msg {
        Message::Binary(data) => {
            match protocol::decode_binary(data) {
                Ok(NetMessage::Hi) => {}
                _ => return Err("Expected Hi from host".to_string()),
            }
        }
        Message::Text(text) => {
            match protocol::decode_json(text) {
                Ok(NetMessage::Hi) => {}
                _ => return Err("Expected Hi from host".to_string()),
            }
        }
        _ => return Err("Expected Hi from host".to_string()),
    }

    // Send sync (binary)
    ws.send(Message::Binary(protocol::encode_binary(&NetMessage::Sync).into()))
        .map_err(|e| format!("Failed to send sync: {}", e))?;

    // Receive snapshot (binary)
    let snap_msg = ws.read()
        .map_err(|e| format!("Failed to read snapshot: {}", e))?;
    match &snap_msg {
        Message::Binary(data) => {
            match protocol::decode_binary(data)? {
                NetMessage::Snapshot { vars } => {
                    let mut gv = global_vars.write().unwrap();
                    gv.clear();
                    for (k, v) in vars {
                        gv.insert(k, v);
                    }
                }
                _ => return Err("Expected snapshot from host".to_string()),
            }
        }
        Message::Text(text) => {
            match protocol::decode_json(text)? {
                NetMessage::Snapshot { vars } => {
                    let mut gv = global_vars.write().unwrap();
                    gv.clear();
                    for (k, v) in vars {
                        gv.insert(k, v);
                    }
                }
                _ => return Err("Expected snapshot from host".to_string()),
            }
        }
        _ => return Err("Expected snapshot from host".to_string()),
    }

    Ok(ws)
}

fn run_loop(
    ws: &mut WsStream,
    broadcast_rx: &mpsc::Receiver<(String, LogoValue)>,
    global_vars: &Arc<RwLock<HashMap<String, LogoValue>>>,
) {
    // Set read timeout for interleaved read/write
    match ws.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)));
        }
        _ => {}
    }

    loop {
        // Read incoming (binary mode)
        match ws.read() {
            Ok(Message::Binary(data)) => {
                if let Ok(msg) = protocol::decode_binary(&data) {
                    handle_incoming(msg, global_vars);
                }
            }
            Ok(Message::Text(text)) => {
                if let Ok(msg) = protocol::decode_json(&text) {
                    handle_incoming(msg, global_vars);
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        // Send outgoing (binary mode)
        while let Ok((name, value)) = broadcast_rx.try_recv() {
            let mut vars = HashMap::new();
            vars.insert(name, value);
            let encoded = protocol::encode_binary(&NetMessage::Set { vars });
            if ws.send(Message::Binary(encoded.into())).is_err() {
                return;
            }
        }
    }
}

fn handle_incoming(msg: NetMessage, global_vars: &Arc<RwLock<HashMap<String, LogoValue>>>) {
    match msg {
        NetMessage::Set { vars } => {
            let mut gv = global_vars.write().unwrap();
            for (name, value) in vars {
                gv.insert(name, value);
            }
        }
        NetMessage::Snapshot { vars } => {
            let mut gv = global_vars.write().unwrap();
            gv.clear();
            for (k, v) in vars {
                gv.insert(k, v);
            }
        }
        _ => {}
    }
}

pub fn start_client(
    addr: &str,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    broadcast_rx: mpsc::Receiver<(String, LogoValue)>,
    system_fn: SystemFn,
    status: NetStatus,
    password: Option<String>,
) -> Result<(), String> {
    let mut ws = connect_and_sync(addr, &global_vars, &password)?;

    let mgr_addr = addr.to_string();
    let mgr_vars = global_vars.clone();
    let mgr_system = system_fn.clone();
    let mgr_status = status.clone();
    thread::spawn(move || {
        run_loop(&mut ws, &broadcast_rx, &mgr_vars);

        mgr_system(&format!("Disconnected from host ({})", mgr_addr));
        *mgr_status.lock().unwrap() = "disconnected".to_string();

        loop {
            thread::sleep(RECONNECT_INTERVAL);

            match connect_and_sync(&mgr_addr, &mgr_vars, &password) {
                Ok(mut new_ws) => {
                    mgr_system(&format!("Reconnected to host ({})", mgr_addr));
                    *mgr_status.lock().unwrap() = "connected".to_string();

                    run_loop(&mut new_ws, &broadcast_rx, &mgr_vars);

                    mgr_system(&format!("Disconnected from host ({})", mgr_addr));
                    *mgr_status.lock().unwrap() = "disconnected".to_string();
                }
                Err(_) => continue,
            }
        }
    });

    Ok(())
}

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
