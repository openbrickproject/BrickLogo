//! Process-wide BLE context shared by every BLE adapter crate.
//!
//! Every `PoweredUpBle` / `CoralBle` instance gets the same tokio Runtime
//! and the same btleplug Adapter (which wraps a single `CBCentralManager`
//! on macOS / a single BlueZ adapter handle on Linux / a single WinRT
//! watcher on Windows). This is required on macOS where multiple
//! `CBCentralManager` instances in the same process don't reliably route
//! advertisement discovery events when one of them has an active
//! connection, causing a second `connectto` to hang forever.

use btleplug::api::Manager as _;
use btleplug::platform::{Adapter, Manager};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

static BLE_CONTEXT: OnceLock<(Arc<Runtime>, Adapter)> = OnceLock::new();

#[cfg(test)]
#[path = "tests/ble.rs"]
mod tests;

/// Return a shared tokio Runtime and btleplug Adapter. Initializes on first
/// call. Panics if the runtime can't be created or no BLE radio is available
/// on this host — both represent environmental failures that the caller can't
/// meaningfully recover from at this layer.
pub fn ble_context() -> (Arc<Runtime>, Adapter) {
    let (rt, adapter) = BLE_CONTEXT.get_or_init(|| {
        let runtime = Arc::new(
            Runtime::new().expect("Failed to create shared tokio runtime"),
        );
        let adapter = runtime.block_on(async {
            let manager = Manager::new()
                .await
                .expect("BLE init failed: no Bluetooth manager");
            let adapters = manager
                .adapters()
                .await
                .expect("BLE init failed: could not list adapters");
            adapters
                .into_iter()
                .next()
                .expect("BLE init failed: no BLE adapter on this host")
        });
        (runtime, adapter)
    });
    (rt.clone(), adapter.clone())
}
