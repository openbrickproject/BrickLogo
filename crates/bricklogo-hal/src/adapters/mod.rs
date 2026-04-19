pub mod buildhat_adapter;
pub mod controllab_adapter;
pub mod coral_adapter;
pub mod ev3_adapter;
pub mod poweredup_adapter;
pub mod rcx_adapter;
pub mod spike_adapter;
pub mod wedo_adapter;

/// Retry a BLE connect operation that may panic due to bluez-async D-Bus race conditions.
/// Catches panics and retries up to `max_attempts` times.
#[cfg(target_os = "linux")]
pub(crate) fn ble_connect_with_retry<F>(mut connect_fn: F, max_attempts: u32) -> Result<(), String>
where
    F: FnMut() -> Result<(), String>,
{
    for attempt in 1..=max_attempts {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| connect_fn())) {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(e)) => return Err(e), // Real error, don't retry
            Err(_panic) => {
                if attempt == max_attempts {
                    return Err(
                        "BLE connection failed (D-Bus error, retries exhausted)".to_string()
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
    unreachable!()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn ble_connect_with_retry<F>(mut connect_fn: F, _max_attempts: u32) -> Result<(), String>
where
    F: FnMut() -> Result<(), String>,
{
    connect_fn()
}
