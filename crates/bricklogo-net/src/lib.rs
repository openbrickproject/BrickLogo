pub mod protocol;
pub mod host;
pub mod client;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, mpsc};
use bricklogo_lang::value::LogoValue;

pub const DEFAULT_PORT: u16 = 9750;

#[derive(Debug, Clone)]
pub enum NetRole {
    Host(u16),
    Client(String),
}

type SystemFn = Arc<dyn Fn(&str) + Send + Sync>;
pub type NetStatus = Arc<Mutex<String>>;

/// Start the network layer. For Host, this spawns listener threads and returns immediately.
/// For Client, this blocks until the initial snapshot is received, then spawns background threads.
/// Returns Err if the connection fails (client can't reach host, or host can't bind port).
pub fn start_network(
    role: NetRole,
    global_vars: Arc<RwLock<HashMap<String, LogoValue>>>,
    broadcast_rx: mpsc::Receiver<(String, LogoValue)>,
    system_fn: SystemFn,
    status: NetStatus,
) -> Result<(), String> {
    match role {
        NetRole::Host(port) => {
            host::start_host(port, global_vars, broadcast_rx, system_fn, status)
        }
        NetRole::Client(addr) => {
            client::start_client(&addr, global_vars, broadcast_rx, system_fn, status)
        }
    }
}
