use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use btleplug::api::{
    Central, Manager as _, Peripheral as _, ScanFilter,
    WriteType,
};
use btleplug::platform::{Manager, Peripheral};
use futures::StreamExt;
use uuid::Uuid;

use crate::constants::*;
use crate::coral::Coral;

const SERVICE_UUID: &str = CORAL_SERVICE_UUID;
const WRITE_CHAR_UUID: &str = CORAL_WRITE_CHAR_UUID;
const NOTIFY_CHAR_UUID: &str = CORAL_NOTIFY_CHAR_UUID;

/// A BLE-connected Coral device.
pub struct CoralBle {
    pub coral: Coral,
    peripheral: Option<Peripheral>,
    runtime: tokio::runtime::Runtime,
    stop_flag: Option<Arc<AtomicBool>>,
    notification_rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>,
}

impl CoralBle {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        CoralBle {
            coral: Coral::new(),
            peripheral: None,
            runtime,
            stop_flag: None,
            notification_rx: None,
        }
    }

    pub fn set_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.stop_flag = Some(flag);
    }

    fn is_stop_requested(&self) -> bool {
        self.stop_flag.as_ref().map_or(false, |f| f.load(Ordering::SeqCst))
    }

    /// Scan for and connect to the first Coral device found.
    pub fn connect(&mut self) -> Result<(), String> {
        let manager = self.runtime.block_on(async {
            Manager::new().await.map_err(|e| format!("BLE init failed: {}", e))
        })?;
        let adapters = self.runtime.block_on(async {
            manager.adapters().await.map_err(|e| format!("No BLE adapter: {}", e))
        })?;
        let adapter = adapters.into_iter().next().ok_or("No BLE adapter found")?;

        self.runtime.block_on(async {
            adapter.start_scan(ScanFilter::default()).await.map_err(|e| format!("Scan failed: {}", e))
        })?;

        let service_uuid = Uuid::parse_str(SERVICE_UUID).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);

        loop {
            if std::time::Instant::now() > deadline {
                let _ = self.runtime.block_on(adapter.stop_scan());
                return Err("No LEGO Education Science device found (timeout)".to_string());
            }

            if self.is_stop_requested() {
                let _ = self.runtime.block_on(adapter.stop_scan());
                return Err("Cancelled".to_string());
            }

            let peripherals = self.runtime.block_on(async {
                adapter.peripherals().await.map_err(|e| format!("Scan error: {}", e))
            })?;

            for p in peripherals {
                let props = match self.runtime.block_on(p.properties()) {
                    Ok(Some(props)) => props,
                    _ => continue,
                };

                if !props.services.contains(&service_uuid) {
                    continue;
                }

                let _ = self.runtime.block_on(adapter.stop_scan());

                // Determine device kind from manufacturer data
                let kind = props.manufacturer_data.iter()
                    .find(|(id, _)| **id == LEGO_COMPANY_ID)
                    .and_then(|(_, data)| {
                        if data.len() >= 2 {
                            CoralDeviceKind::from_hardware_byte(data[1])
                        } else {
                            None
                        }
                    })
                    .unwrap_or(CoralDeviceKind::DoubleMotor);

                // Connect
                self.runtime.block_on(async {
                    p.connect().await.map_err(|e| format!("Connect failed: {}", e))
                })?;
                self.runtime.block_on(async {
                    p.discover_services().await.map_err(|e| format!("Service discovery failed: {}", e))
                })?;

                // Subscribe to notifications
                let chars = p.characteristics();
                let notify_uuid = Uuid::parse_str(NOTIFY_CHAR_UUID).unwrap();
                let notify_char = chars.iter()
                    .find(|c| c.uuid == notify_uuid)
                    .ok_or("Notify characteristic not found")?;

                self.runtime.block_on(async {
                    p.subscribe(notify_char).await.map_err(|e| format!("Subscribe failed: {}", e))
                })?;

                // Spawn background task to forward notifications to a channel
                let (tx, rx) = std::sync::mpsc::channel();
                let p_clone = p.clone();
                self.runtime.spawn(async move {
                    if let Ok(mut stream) = p_clone.notifications().await {
                        while let Some(notif) = stream.next().await {
                            if tx.send(notif.value).is_err() {
                                break; // receiver dropped
                            }
                        }
                    }
                });
                self.notification_rx = Some(rx);

                self.peripheral = Some(p.clone());
                self.coral.on_connected(kind);

                // Send info request and enable notifications
                let info_cmd = self.coral.cmd_info_request();
                self.send_to(&p, &info_cmd)?;

                let notif_cmd = self.coral.cmd_notification_request(DEFAULT_NOTIFICATION_INTERVAL_MS);
                self.send_to(&p, &notif_cmd)?;

                return Ok(());
            }

            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Disconnect from the device.
    pub fn disconnect(&mut self) {
        if let Some(ref peripheral) = self.peripheral {
            let _ = self.runtime.block_on(peripheral.disconnect());
        }
        self.peripheral = None;
        self.notification_rx = None;
        self.coral.on_disconnected();
    }

    /// Send a command to the device (fire-and-forget).
    pub fn send(&self, data: &[u8]) -> Result<(), String> {
        let peripheral = self.peripheral.as_ref().ok_or("Not connected")?;
        self.send_to(peripheral, data)
    }

    /// Send a command and wait for a response notification.
    pub fn request(&mut self, data: &[u8]) -> Result<(), String> {
        if self.peripheral.is_none() { return Err("Not connected".to_string()); }

        // Send the command
        self.send(data)?;

        // Wait for a response by polling the persistent notification channel
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        if let Some(ref rx) = self.notification_rx {
            loop {
                if std::time::Instant::now() > deadline {
                    return Err("Request timed out".to_string());
                }
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(data) => {
                        self.coral.process_notification(&data);
                        if !data.is_empty() {
                            return Ok(());
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return Err("Notification stream ended".to_string());
                    }
                }
            }
        } else {
            Err("No notification stream".to_string())
        }
    }

    /// Poll for notifications and update the Coral protocol state.
    pub fn poll(&mut self) -> Result<(), String> {
        if let Some(ref rx) = self.notification_rx {
            // Drain all pending notifications
            while let Ok(data) = rx.try_recv() {
                self.coral.process_notification(&data);
            }
        }
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.coral.is_connected()
    }

    // ── Internal helpers ────────────────────────

    fn send_to(&self, peripheral: &Peripheral, data: &[u8]) -> Result<(), String> {
        let write_uuid = Uuid::parse_str(WRITE_CHAR_UUID).unwrap();
        let chars = peripheral.characteristics();
        let write_char = chars.iter()
            .find(|c| c.uuid == write_uuid)
            .ok_or("Write characteristic not found")?;

        self.runtime.block_on(async {
            peripheral.write(write_char, data, WriteType::WithoutResponse)
                .await
                .map_err(|e| format!("Write failed: {}", e))
        })
    }
}
