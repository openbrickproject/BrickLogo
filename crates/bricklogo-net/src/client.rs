use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread;
use std::time::Duration;

use bricklogo_lang::value::LogoValue;
use crate::protocol::{self, NetMessage, write_message, read_message};
use crate::NetStatus;

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);

fn connect_and_sync(
    addr: &str,
    global_vars: &Arc<RwLock<HashMap<String, LogoValue>>>,
) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect(addr)
        .map_err(|e| format!("Failed to join {}: {}", addr, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("{}", e))?;

    write_message(&mut stream, &NetMessage::Sync)
        .map_err(|e| format!("Failed to send sync: {}", e))?;

    let msg = read_message(&mut stream)
        .map_err(|e| format!("Failed to read snapshot: {}", e))?;

    match msg {
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

fn read_loop(
    stream: &mut TcpStream,
    global_vars: &Arc<RwLock<HashMap<String, LogoValue>>>,
) {
    let mut reader = protocol::MessageReader::new();
    loop {
        let msg = match reader.read(stream) {
            Ok(m) => m,
            Err(_) => break,
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
    let mut stream = connect_and_sync(addr, &global_vars)?;

    let write_stream: Arc<Mutex<Option<TcpStream>>> = Arc::new(Mutex::new(
        Some(stream.try_clone().map_err(|e| format!("{}", e))?)
    ));

    // Writer thread: drains broadcast_rx, sends to host
    let writer_handle = write_stream.clone();
    thread::spawn(move || {
        while let Ok((name, value)) = broadcast_rx.recv() {
            let encoded = protocol::encode(&NetMessage::Set { name, value });
            let mut guard = writer_handle.lock().unwrap();
            if let Some(ref mut s) = *guard {
                if s.write_all(&encoded).is_err() || s.flush().is_err() {
                    *guard = None;
                }
            }
        }
    });

    // Connection manager thread: handles read loop + reconnect
    let mgr_addr = addr.to_string();
    let mgr_vars = global_vars.clone();
    let mgr_write = write_stream.clone();
    let mgr_system = system_fn.clone();
    let mgr_status = status.clone();
    thread::spawn(move || {
        read_loop(&mut stream, &mgr_vars);

        mgr_system(&format!("Disconnected from host ({})", mgr_addr));
        *mgr_status.lock().unwrap() = "disconnected".to_string();
        *mgr_write.lock().unwrap() = None;

        loop {
            thread::sleep(RECONNECT_INTERVAL);

            match connect_and_sync(&mgr_addr, &mgr_vars) {
                Ok(mut new_stream) => {
                    mgr_system(&format!("Reconnected to host ({})", mgr_addr));
                    *mgr_status.lock().unwrap() = "connected".to_string();
                    *mgr_write.lock().unwrap() = new_stream.try_clone().ok();

                    read_loop(&mut new_stream, &mgr_vars);

                    mgr_system(&format!("Disconnected from host ({})", mgr_addr));
                    *mgr_status.lock().unwrap() = "disconnected".to_string();
                    *mgr_write.lock().unwrap() = None;
                }
                Err(_) => continue,
            }
        }
    });

    Ok(())
}

use std::io::Write;

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
