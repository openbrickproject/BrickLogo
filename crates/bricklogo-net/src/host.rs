use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;

use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};
use crate::NetStatus;

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;

const CLIENT_CHANNEL_SIZE: usize = 256;

struct ClientEntry {
    addr: String,
    tx: mpsc::SyncSender<String>,
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

fn broadcast_to_others(clients: &ClientList, sender_addr: &str, msg: &str) {
    let mut list = clients.lock().unwrap();
    list.retain(|c| {
        if c.addr == sender_addr {
            return true;
        }
        c.tx.try_send(msg.to_string()).is_ok()
    });
}

fn broadcast_to_all(clients: &ClientList, msg: &str) {
    let mut list = clients.lock().unwrap();
    list.retain(|c| c.tx.try_send(msg.to_string()).is_ok());
}

pub fn start_host(
    port: u16,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    broadcast_rx: mpsc::Receiver<(String, LogoValue)>,
    system_fn: SystemFn,
    status: NetStatus,
) -> Result<(), String> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to start host on port {}: {}", port, e))?;

    let clients: ClientList = Arc::new(Mutex::new(Vec::new()));

    // Broadcast thread: reads local variable changes and fans out to client channels
    let bc_clients = clients.clone();
    thread::spawn(move || {
        while let Ok((name, value)) = broadcast_rx.recv() {
            let msg = protocol::encode(&NetMessage::Set { name, value });
            broadcast_to_all(&bc_clients, &msg);
        }
    });

    // Listener thread: accepts new connections
    let accept_clients = clients.clone();
    let accept_vars = global_vars.clone();
    let accept_system = system_fn.clone();
    let accept_status = status.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let addr = stream.peer_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|_| "unknown".to_string());
                    accept_system(&format!("{} joined", addr));

                    let reader_stream = stream.try_clone().expect("Failed to clone stream");
                    let (tx, rx) = mpsc::sync_channel::<String>(CLIENT_CHANNEL_SIZE);

                    accept_clients.lock().unwrap().push(ClientEntry {
                        addr: addr.clone(),
                        tx,
                    });
                    update_status(&accept_clients, &accept_status);

                    // Per-client writer thread: drains channel to TCP
                    let writer_addr = addr.clone();
                    let writer_clients = accept_clients.clone();
                    let writer_system = accept_system.clone();
                    let writer_status = accept_status.clone();
                    thread::spawn(move || {
                        let mut s = stream;
                        while let Ok(msg) = rx.recv() {
                            if s.write_all(msg.as_bytes()).is_err() || s.flush().is_err() {
                                remove_client(&writer_clients, &writer_addr, &writer_system, &writer_status);
                                break;
                            }
                        }
                    });

                    // Per-client reader thread: reads from TCP
                    let client_vars = accept_vars.clone();
                    let client_clients = accept_clients.clone();
                    let client_system = accept_system.clone();
                    let client_status = accept_status.clone();
                    let client_addr = addr.clone();
                    thread::spawn(move || {
                        handle_client(
                            reader_stream,
                            client_addr,
                            client_vars,
                            client_clients,
                            client_system,
                            client_status,
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
    stream: TcpStream,
    addr: String,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    clients: ClientList,
    system_fn: SystemFn,
    status: NetStatus,
) {
    let reader = BufReader::new(&stream);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let msg = match protocol::decode(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match msg {
            NetMessage::Sync => {
                let vars = global_vars.read().unwrap().clone();
                let response = protocol::encode(&NetMessage::Snapshot { vars });
                let list = clients.lock().unwrap();
                if let Some(c) = list.iter().find(|c| c.addr == addr) {
                    let _ = c.tx.try_send(response);
                }
            }
            NetMessage::Set { name, value } => {
                global_vars.write().unwrap().insert(name.clone(), value.clone());
                let msg = protocol::encode(&NetMessage::Set { name, value });
                broadcast_to_others(&clients, &addr, &msg);
            }
            NetMessage::Snapshot { .. } => {}
        }
    }

    remove_client(&clients, &addr, &system_fn, &status);
}

#[cfg(test)]
#[path = "tests/host.rs"]
mod tests;
