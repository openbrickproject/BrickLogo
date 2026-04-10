use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage};
use crate::NetStatus;

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);

/// Connect to host, send Sync, receive Snapshot, apply to global_vars.
/// Returns the connected stream on success.
fn connect_and_sync(
    addr: &str,
    global_vars: &Arc<RwLock<HashMap<String, LogoValue>>>,
) -> Result<TcpStream, String> {
    let stream = TcpStream::connect(addr)
        .map_err(|e| format!("Failed to join {}: {}", addr, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("{}", e))?;

    // Send sync
    {
        let mut s = &stream;
        s.write_all(protocol::encode(&NetMessage::Sync).as_bytes())
            .map_err(|e| format!("{}", e))?;
        s.flush().map_err(|e| format!("{}", e))?;
    }

    // Read snapshot
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

    stream.set_read_timeout(None).map_err(|e| format!("{}", e))?;
    Ok(stream)
}

/// Read from the stream until disconnect. Returns when the connection is lost.
fn read_loop(
    stream: &TcpStream,
    global_vars: &Arc<RwLock<HashMap<String, LogoValue>>>,
) {
    let reader = BufReader::new(stream);
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
            NetMessage::Set { name, value } => {
                global_vars.write().unwrap().insert(name, value);
            }
            NetMessage::Snapshot { vars } => {
                let mut gv = global_vars.write().unwrap();
                gv.clear();
                for (k, v) in vars {
                    gv.insert(k, v);
                }
            }
            NetMessage::Sync => {}
        }
    }
}

pub fn start_client(
    addr: &str,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    broadcast_rx: mpsc::Receiver<(String, LogoValue)>,
    system_fn: SystemFn,
    status: NetStatus,
) -> Result<(), String> {
    // Initial connect must succeed — caller terminates on failure
    let stream = connect_and_sync(addr, &global_vars)?;

    // Shared write stream: writer thread sends through this, connection manager swaps it on reconnect
    let write_stream: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(
        Some(stream.try_clone().map_err(|e| format!("{}", e))?)
    ));

    // Writer thread: drains broadcast_rx, sends to host (or drops messages while disconnected)
    let writer_handle = write_stream.clone();
    thread::spawn(move || {
        while let Ok((name, value)) = broadcast_rx.recv() {
            let msg = protocol::encode(&NetMessage::Set { name, value });
            let mut guard = writer_handle.lock().unwrap();
            if let Some(ref mut s) = *guard {
                if s.write_all(msg.as_bytes()).is_err() || s.flush().is_err() {
                    *guard = None; // Mark as disconnected, connection manager will handle reconnect
                }
            }
            // If None, we're disconnected — drop the message silently
        }
    });

    // Connection manager thread: handles read loop + reconnect
    let mgr_addr = addr.to_string();
    let mgr_vars = global_vars.clone();
    let mgr_write = write_stream.clone();
    let mgr_system = system_fn.clone();
    let mgr_status = status.clone();
    thread::spawn(move || {
        // First connection is already established — run the read loop
        read_loop(&stream, &mgr_vars);

        // Connection lost — enter reconnect cycle
        mgr_system(&format!("Disconnected from host ({})", mgr_addr));
        *mgr_status.lock().unwrap() = "disconnected".to_string();
        *mgr_write.lock().unwrap() = None;

        loop {
            thread::sleep(RECONNECT_INTERVAL);

            match connect_and_sync(&mgr_addr, &mgr_vars) {
                Ok(new_stream) => {
                    mgr_system(&format!("Reconnected to host ({})", mgr_addr));
                    *mgr_status.lock().unwrap() = "connected".to_string();

                    // Give writer thread the new stream
                    *mgr_write.lock().unwrap() = new_stream.try_clone().ok();

                    // Run read loop until next disconnect
                    read_loop(&new_stream, &mgr_vars);

                    // Disconnected again
                    mgr_system(&format!("Disconnected from host ({})", mgr_addr));
                    *mgr_status.lock().unwrap() = "disconnected".to_string();
                    *mgr_write.lock().unwrap() = None;
                }
                Err(_) => continue, // Retry after next sleep
            }
        }
    });

    Ok(())
}

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
