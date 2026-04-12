use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use tungstenite::{Message, accept};
use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};
use crate::NetStatus;

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;

const CLIENT_CHANNEL_SIZE: usize = 256;
const READ_TIMEOUT_MS: u64 = 10;
const AUTH_TIMEOUT_MS: u64 = 5000;

enum ClientMessage {
    Json(String),
    Binary(Vec<u8>),
}

struct ClientEntry {
    addr: String,
    tx: mpsc::SyncSender<ClientMessage>,
    binary: bool,
}

type ClientList = Arc<Mutex<Vec<ClientEntry>>>;

fn update_status(clients: &ClientList, status: &NetStatus) {
    let count = clients.lock().unwrap().len();
    let label = if count == 1 { "client" } else { "clients" };
    *status.lock().unwrap() = format!("hosting ({} {})", count, label);
}

fn remove_client(clients: &ClientList, addr: &str, system_fn: &SystemFn, status: &NetStatus) {
    clients.lock().unwrap().retain(|c| c.addr != addr);
    system_fn(&format!("{} disconnected", addr));
    update_status(clients, status);
}

fn broadcast_to_others(clients: &ClientList, sender_addr: &str, json: &str, binary: &[u8]) {
    let mut list = clients.lock().unwrap();
    list.retain(|c| {
        if c.addr == sender_addr {
            return true;
        }
        let msg = if c.binary {
            ClientMessage::Binary(binary.to_vec())
        } else {
            ClientMessage::Json(json.to_string())
        };
        c.tx.try_send(msg).is_ok()
    });
}

fn broadcast_to_all(clients: &ClientList, json: &str, binary: &[u8]) {
    let mut list = clients.lock().unwrap();
    list.retain(|c| {
        let msg = if c.binary {
            ClientMessage::Binary(binary.to_vec())
        } else {
            ClientMessage::Json(json.to_string())
        };
        c.tx.try_send(msg).is_ok()
    });
}

pub fn start_host(
    port: u16,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    broadcast_rx: mpsc::Receiver<(String, LogoValue)>,
    system_fn: SystemFn,
    status: NetStatus,
    password_hash: Option<String>,
) -> Result<(), String> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to start host on port {}: {}", port, e))?;

    let clients: ClientList = Arc::new(Mutex::new(Vec::new()));

    // Broadcast thread: reads local variable changes and fans out to client channels
    let bc_clients = clients.clone();
    thread::spawn(move || {
        while let Ok((name, value)) = broadcast_rx.recv() {
            let mut vars = HashMap::new();
            vars.insert(name, value);
            let msg = NetMessage::Set { vars };
            let json = protocol::encode_json(&msg);
            let binary = protocol::encode_binary(&msg);
            broadcast_to_all(&bc_clients, &json, &binary);
        }
    });

    // Listener thread: accepts new WebSocket connections
    let accept_clients = clients.clone();
    let accept_vars = global_vars.clone();
    let accept_system = system_fn.clone();
    let accept_status = status.clone();
    let accept_pw = password_hash.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let addr = stream.peer_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|_| "unknown".to_string());

                    let ws = match accept(stream) {
                        Ok(ws) => ws,
                        Err(_) => continue,
                    };

                    let client_vars = accept_vars.clone();
                    let client_clients = accept_clients.clone();
                    let client_system = accept_system.clone();
                    let client_status = accept_status.clone();
                    let client_pw = accept_pw.clone();
                    thread::spawn(move || {
                        handle_client(
                            ws, addr, client_vars, client_clients,
                            client_system, client_status, client_pw,
                        );
                    });
                }
                Err(_) => continue,
            }
        }
    });

    Ok(())
}

fn handle_client(
    mut ws: tungstenite::WebSocket<std::net::TcpStream>,
    addr: String,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    clients: ClientList,
    system_fn: SystemFn,
    status: NetStatus,
    password_hash: Option<String>,
) {
    // Every connection requires hello within 5 seconds before being registered
    let _ = ws.get_mut().set_read_timeout(Some(Duration::from_millis(AUTH_TIMEOUT_MS)));
    let mut binary_mode = match ws.read() {
        Ok(msg) => {
            match decode_ws_message(&msg) {
                Some(NetMessage::Hello { password, binary_protocol }) => {
                    // If password required, validate
                    if let Some(ref pw) = password_hash {
                        if password.as_deref() != Some(pw.as_str()) {
                            return; // Wrong or missing password
                        }
                    }
                    send_message(&mut ws, &NetMessage::Hi, binary_protocol);
                    binary_protocol
                }
                _ => return, // Not a hello
            }
        }
        Err(_) => return, // Timeout or error
    };

    // Hello accepted — register the client
    system_fn(&format!("{} joined", addr));
    let (tx, rx) = mpsc::sync_channel::<ClientMessage>(CLIENT_CHANNEL_SIZE);
    clients.lock().unwrap().push(ClientEntry {
        addr: addr.clone(),
        tx,
        binary: binary_mode,
    });
    update_status(&clients, &status);

    // Normal operation loop
    let _ = ws.get_mut().set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)));

    loop {
        // Read incoming
        match ws.read() {
            Ok(msg) => {
                if let Some(net_msg) = decode_ws_message(&msg) {
                    match net_msg {
                        NetMessage::Hello { password, binary_protocol } => {
                            // Re-authenticate or mode switch
                            if let Some(ref pw) = password_hash {
                                if password.as_deref() != Some(pw.as_str()) {
                                    break; // Wrong password mid-session
                                }
                            }
                            binary_mode = binary_protocol;
                            if let Some(c) = clients.lock().unwrap().iter_mut().find(|c| c.addr == addr) {
                                c.binary = binary_mode;
                            }
                            send_message(&mut ws, &NetMessage::Hi, binary_mode);
                        }
                        NetMessage::Sync => {
                            let vars = global_vars.read().unwrap().clone();
                            send_message(&mut ws, &NetMessage::Snapshot { vars }, binary_mode);
                        }
                        NetMessage::Set { vars } => {
                            let mut changed_vars = HashMap::new();
                            {
                                let mut gv = global_vars.write().unwrap();
                                for (name, value) in vars {
                                    if gv.get(&name) != Some(&value) {
                                        gv.insert(name.clone(), value.clone());
                                        changed_vars.insert(name, value);
                                    }
                                }
                            }
                            if !changed_vars.is_empty() {
                                let msg = NetMessage::Set { vars: changed_vars };
                                let json = protocol::encode_json(&msg);
                                let binary = protocol::encode_binary(&msg);
                                broadcast_to_others(&clients, &addr, &json, &binary);
                            }
                        }
                        _ => {}
                    }
                } else if matches!(msg, Message::Close(_)) {
                    break;
                }
            }
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        // Drain outgoing
        while let Ok(msg) = rx.try_recv() {
            let result = match msg {
                ClientMessage::Json(text) => ws.send(Message::Text(text.into())),
                ClientMessage::Binary(data) => ws.send(Message::Binary(data.into())),
            };
            if result.is_err() {
                remove_client(&clients, &addr, &system_fn, &status);
                return;
            }
        }
    }

    remove_client(&clients, &addr, &system_fn, &status);
}

fn decode_ws_message(msg: &Message) -> Option<NetMessage> {
    match msg {
        Message::Text(text) => protocol::decode_json(text).ok(),
        Message::Binary(data) => protocol::decode_binary(data).ok(),
        _ => None,
    }
}

fn send_message(ws: &mut tungstenite::WebSocket<std::net::TcpStream>, msg: &NetMessage, binary: bool) {
    if binary {
        let _ = ws.send(Message::Binary(protocol::encode_binary(msg).into()));
    } else {
        let _ = ws.send(Message::Text(protocol::encode_json(msg).into()));
    }
}

#[cfg(test)]
#[path = "tests/host.rs"]
mod tests;
