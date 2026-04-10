use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};
use crate::NetStatus;

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;

pub fn start_client(
    addr: &str,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    broadcast_rx: mpsc::Receiver<(String, LogoValue)>,
    system_fn: SystemFn,
    status: NetStatus,
) -> Result<(), String> {
    let stream = TcpStream::connect(addr)
        .map_err(|e| format!("Failed to join {}: {}", addr, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    let writer_stream = stream.try_clone()
        .map_err(|e| format!("Failed to clone stream: {}", e))?;
    let reader_stream = stream.try_clone()
        .map_err(|e| format!("Failed to clone stream: {}", e))?;

    // Send sync request
    {
        let mut s = &stream;
        let sync_msg = protocol::encode(&NetMessage::Sync);
        s.write_all(sync_msg.as_bytes())
            .map_err(|e| format!("Failed to send sync: {}", e))?;
        s.flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;
    }

    // Wait for snapshot response
    {
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line)
            .map_err(|e| format!("Failed to read snapshot: {}", e))?;

        match protocol::decode(&line)? {
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

    // Clear the read timeout for the ongoing reader thread
    reader_stream.set_read_timeout(None)
        .map_err(|e| format!("Failed to clear timeout: {}", e))?;

    // Writer thread: sends local variable changes to host
    let writer_addr = addr.to_string();
    let writer_system = system_fn.clone();
    let writer_status = status.clone();
    thread::spawn(move || {
        let mut s = writer_stream;
        while let Ok((name, value)) = broadcast_rx.recv() {
            let msg = protocol::encode(&NetMessage::Set { name, value });
            if s.write_all(msg.as_bytes()).is_err() || s.flush().is_err() {
                writer_system(&format!("Disconnected from host ({})", writer_addr));
                *writer_status.lock().unwrap() = "disconnected".to_string();
                break;
            }
        }
    });

    // Reader thread: receives variable updates from host
    let reader_vars = global_vars.clone();
    let reader_system = system_fn.clone();
    let reader_addr = addr.to_string();
    let reader_status = status.clone();
    thread::spawn(move || {
        let reader = BufReader::new(reader_stream);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => {
                    reader_system(&format!("Disconnected from host ({})", reader_addr));
                    *reader_status.lock().unwrap() = "disconnected".to_string();
                    break;
                }
            };
            if line.trim().is_empty() {
                continue;
            }

            let msg = match protocol::decode(&line) {
                Ok(m) => m,
                Err(_) => continue,
            };

            match msg {
                NetMessage::Set { name, value } => {
                    reader_vars.write().unwrap().insert(name, value);
                }
                NetMessage::Snapshot { vars } => {
                    let mut gv = reader_vars.write().unwrap();
                    gv.clear();
                    for (k, v) in vars {
                        gv.insert(k, v);
                    }
                    reader_system("Reconnected to host");
                    *reader_status.lock().unwrap() = "connected".to_string();
                }
                NetMessage::Sync => {}
            }
        }
    });

    Ok(())
}

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
