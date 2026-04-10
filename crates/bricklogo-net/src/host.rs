use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;

use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};
use crate::NetStatus;

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;
type Clients = Arc<Mutex<Vec<(String, TcpStream)>>>;

fn update_status(clients: &Clients, status: &NetStatus) {
    let count = clients.lock().unwrap().len();
    let label = if count == 1 { "client" } else { "clients" };
    *status.lock().unwrap() = format!("hosting ({} {})", count, label);
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

    let clients: Clients = Arc::new(Mutex::new(Vec::new()));

    // Broadcast thread: reads local variable changes and sends to all clients
    let bc_clients = clients.clone();
    thread::spawn(move || {
        while let Ok((name, value)) = broadcast_rx.recv() {
            let msg = protocol::encode(&NetMessage::Set { name, value });
            let mut clients = bc_clients.lock().unwrap();
            clients.retain(|(_, stream)| {
                let mut s = stream;
                s.write_all(msg.as_bytes()).is_ok() && s.flush().is_ok()
            });
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

                    // Clone the stream for the client list (writing) and the reader thread
                    let reader_stream = stream.try_clone().expect("Failed to clone stream");
                    accept_clients.lock().unwrap().push((addr.clone(), stream));
                    update_status(&accept_clients, &accept_status);

                    // Spawn reader thread for this client
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
    clients: Clients,
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
                let mut clients_lock = clients.lock().unwrap();
                if let Some((_, s)) = clients_lock.iter_mut().find(|(a, _)| *a == addr) {
                    let _ = s.write_all(response.as_bytes());
                    let _ = s.flush();
                }
            }
            NetMessage::Set { name, value } => {
                // Write directly to global vars (no channel — avoids re-broadcast to self)
                global_vars.write().unwrap().insert(name.clone(), value.clone());

                // Broadcast to all OTHER clients
                let msg = protocol::encode(&NetMessage::Set { name, value });
                let mut clients_lock = clients.lock().unwrap();
                clients_lock.retain(|(a, s)| {
                    if *a == addr {
                        return true; // Keep sender, don't send back to them
                    }
                    let mut s = s;
                    s.write_all(msg.as_bytes()).is_ok() && s.flush().is_ok()
                });
            }
            NetMessage::Snapshot { .. } => {}
        }
    }

    // Client disconnected
    system_fn(&format!("{} disconnected", addr));
    clients.lock().unwrap().retain(|(a, _)| *a != addr);
    update_status(&clients, &status);
}

#[cfg(test)]
#[path = "tests/host.rs"]
mod tests;
