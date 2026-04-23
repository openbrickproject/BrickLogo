//! Tests for the shared BLE context.
//!
//! The `ble_context()` singleton calls into real system BLE APIs on first
//! use (CoreBluetooth on macOS, BlueZ on Linux, WinRT on Windows). Those
//! APIs can't reasonably be mocked at the btleplug layer, so these tests
//! focus on the contract of the cache: the first call initialises, every
//! subsequent call returns `Arc` / `Adapter` clones pointing at the same
//! underlying objects.
//!
//! Environments without a BLE radio (some CI runners) cannot exercise the
//! initialisation path. On those hosts `ble_context()` panics by design —
//! the tests below gate on adapter availability and are skipped with a
//! clear message when the stack is unavailable.

use super::*;

fn ble_available() -> bool {
    // Try to initialize a tokio runtime + Manager without touching the
    // OnceLock. If we can't even build a Manager, we're in a sandbox
    // without Bluetooth — skip the tests rather than crash the suite.
    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(_) => return false,
    };
    rt.block_on(async {
        match Manager::new().await {
            Ok(m) => match m.adapters().await {
                Ok(list) => !list.is_empty(),
                Err(_) => false,
            },
            Err(_) => false,
        }
    })
}

#[test]
fn test_ble_context_is_cached_across_calls() {
    if !ble_available() {
        eprintln!("skipping: no BLE adapter available on this host");
        return;
    }
    let (rt_a, _adapter_a) = ble_context();
    let (rt_b, _adapter_b) = ble_context();
    assert!(
        Arc::ptr_eq(&rt_a, &rt_b),
        "ble_context must hand out Arc clones of the same Runtime"
    );
}

#[test]
fn test_ble_context_returns_usable_runtime() {
    if !ble_available() {
        eprintln!("skipping: no BLE adapter available on this host");
        return;
    }
    let (rt, _adapter) = ble_context();
    let value = rt.block_on(async { 42 });
    assert_eq!(value, 42);
}
