//! Background peripheral health watchdog.
//!
//! Some BLE backends (notably bluez-async on Linux) can drop a peripheral
//! connection without delivering a clean disconnect event to our
//! notification stream — sometimes accompanied by a panic in a tokio worker
//! task. When that happens, the adapter's `connected()` flips to false but
//! BrickLogo's `port_manager` still holds the entry.
//!
//! `start` spawns a daemon thread that periodically asks every device's
//! adapter whether it still thinks it's connected. Devices that report false
//! get removed from the manager and a system message is emitted so the user
//! knows what happened.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::port_manager::PortManager;

const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Spawn the health monitor thread. Idempotent at the call site — only call
/// once per `PortManager`.
pub fn start(
    port_manager: Arc<Mutex<PortManager>>,
    system_fn: Arc<dyn Fn(&str) + Send + Sync>,
) {
    thread::spawn(move || {
        loop {
            thread::sleep(POLL_INTERVAL);
            if tick(&port_manager, &*system_fn).is_break() {
                return;
            }
        }
    });
}

/// Single health-check pass: find dead devices, emit a system message for
/// each, remove them from the manager. Returns `ControlFlow::Break` when
/// the port-manager lock is poisoned — the daemon thread exits silently
/// in that case because the caller can't recover.
///
/// Extracted so tests can exercise the reconciliation logic without a
/// real background thread.
pub(crate) fn tick(
    port_manager: &Mutex<PortManager>,
    system_fn: &(dyn Fn(&str) + Send + Sync),
) -> std::ops::ControlFlow<()> {
    let dead: Vec<String> = {
        let pm = match port_manager.lock() {
            Ok(g) => g,
            Err(_) => return std::ops::ControlFlow::Break(()),
        };
        pm.dead_device_names()
    };

    for name in dead {
        system_fn(&format!("Device \"{}\" lost connection", name));
        if let Ok(mut pm) = port_manager.lock() {
            pm.remove_device(&name);
        } else {
            return std::ops::ControlFlow::Break(());
        }
    }
    std::ops::ControlFlow::Continue(())
}

#[cfg(test)]
#[path = "tests/health.rs"]
mod tests;
