//! BLE transport for SPIKE Prime. Wraps a btleplug peripheral behind the
//! `SpikeTransport` trait so the adapter's Atlantis/COBS code path is
//! transport-agnostic.
//!
//! GATT UUIDs come from LEGO's official `spike-prime-docs` app.py:
//!   Service: 0000fd02-0000-1000-8000-00805f9b34fb
//!   RX     : 0000fd02-0001-1000-8000-00805f9b34fb  (host → hub)
//!   TX     : 0000fd02-0002-1000-8000-00805f9b34fb  (hub → host)
//!
//! Peripherals already connected by another adapter are skipped during the
//! scan (btleplug's `is_connected()` check), giving cheap multi-hub claim
//! tracking without a process-wide registry.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use btleplug::api::{Central, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Peripheral};
use futures::stream::StreamExt;
use tokio::runtime::Runtime;
use uuid::Uuid;

use super::spike_adapter::SpikeTransport;

pub const SERVICE_UUID: &str = "0000fd02-0000-1000-8000-00805f9b34fb";
pub const WRITE_CHAR_UUID: &str = "0000fd02-0001-1000-8000-00805f9b34fb";
pub const NOTIFY_CHAR_UUID: &str = "0000fd02-0002-1000-8000-00805f9b34fb";

/// Conservative BLE write chunk size. 20 bytes fits the default GATT MTU
/// on every platform without relying on negotiated extension.
const WRITE_CHUNK: usize = 20;

pub struct SpikeBleTransport {
    peripheral: Peripheral,
    runtime: Arc<Runtime>,
    notification_rx: mpsc::Receiver<Vec<u8>>,
    leftover: Vec<u8>,
    connected: Arc<AtomicBool>,
    write_char: btleplug::api::Characteristic,
}

impl SpikeBleTransport {
    /// Scan BLE for an unclaimed SPIKE Prime hub, connect, and return a
    /// ready-to-use transport. Returns `Ok(None)` if no hub was found before
    /// the timeout expired.
    pub fn scan_and_connect(timeout: Duration) -> Result<Option<Self>, String> {
        let (runtime, adapter) = crate::ble::ble_context();

        runtime.block_on(async {
            adapter
                .start_scan(ScanFilter::default())
                .await
                .map_err(|e| format!("BLE scan failed: {}", e))
        })?;

        let service_uuid = Uuid::parse_str(SERVICE_UUID).unwrap();
        let deadline = Instant::now() + timeout;

        let result: Result<Option<Self>, String> = (|| {
            while Instant::now() <= deadline {
                let peripherals = runtime.block_on(async {
                    adapter
                        .peripherals()
                        .await
                        .map_err(|e| format!("BLE peripherals query failed: {}", e))
                })?;

                for p in peripherals {
                    if runtime.block_on(p.is_connected()).unwrap_or(false) {
                        continue;
                    }
                    let props = match runtime.block_on(p.properties()) {
                        Ok(Some(props)) => props,
                        _ => continue,
                    };
                    if !props.services.contains(&service_uuid) {
                        continue;
                    }
                    let transport = Self::connect_peripheral(runtime.clone(), &adapter, p)?;
                    return Ok(Some(transport));
                }

                std::thread::sleep(Duration::from_millis(200));
            }
            Ok(None)
        })();

        let _ = runtime.block_on(adapter.stop_scan());
        result
    }

    fn connect_peripheral(
        runtime: Arc<Runtime>,
        _adapter: &Adapter,
        peripheral: Peripheral,
    ) -> Result<Self, String> {
        runtime.block_on(async {
            peripheral
                .connect()
                .await
                .map_err(|e| format!("BLE connect failed: {}", e))
        })?;
        runtime.block_on(async {
            peripheral
                .discover_services()
                .await
                .map_err(|e| format!("BLE service discovery failed: {}", e))
        })?;

        let write_uuid = Uuid::parse_str(WRITE_CHAR_UUID).unwrap();
        let notify_uuid = Uuid::parse_str(NOTIFY_CHAR_UUID).unwrap();
        let chars = peripheral.characteristics();
        let write_char = chars
            .iter()
            .find(|c| c.uuid == write_uuid)
            .ok_or("SPIKE Prime write characteristic not found")?
            .clone();
        let notify_char = chars
            .iter()
            .find(|c| c.uuid == notify_uuid)
            .ok_or("SPIKE Prime notify characteristic not found")?
            .clone();

        runtime.block_on(async {
            peripheral
                .subscribe(&notify_char)
                .await
                .map_err(|e| format!("BLE subscribe failed: {}", e))
        })?;

        let (tx, rx) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(true));
        let connected_clone = connected.clone();
        let p_clone = peripheral.clone();
        runtime.spawn(async move {
            if let Ok(mut stream) = p_clone.notifications().await {
                while let Some(notif) = stream.next().await {
                    if tx.send(notif.value).is_err() {
                        break;
                    }
                }
            }
            connected_clone.store(false, Ordering::SeqCst);
        });

        Ok(SpikeBleTransport {
            peripheral,
            runtime,
            notification_rx: rx,
            leftover: Vec::new(),
            connected,
            write_char,
        })
    }
}

impl SpikeTransport for SpikeBleTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        if !self.leftover.is_empty() {
            let n = buf.len().min(self.leftover.len());
            buf[..n].copy_from_slice(&self.leftover[..n]);
            self.leftover.drain(..n);
            return Ok(n);
        }
        match self.notification_rx.try_recv() {
            Ok(data) => {
                let n = buf.len().min(data.len());
                buf[..n].copy_from_slice(&data[..n]);
                if n < data.len() {
                    self.leftover.extend_from_slice(&data[n..]);
                }
                Ok(n)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(0),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.connected.store(false, Ordering::SeqCst);
                Err("BLE notification stream closed".to_string())
            }
        }
    }

    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err("BLE not connected".to_string());
        }
        for chunk in data.chunks(WRITE_CHUNK) {
            let peripheral = self.peripheral.clone();
            let write_char = self.write_char.clone();
            let chunk = chunk.to_vec();
            self.runtime
                .block_on(async move {
                    peripheral
                        .write(&write_char, &chunk, WriteType::WithoutResponse)
                        .await
                        .map_err(|e| format!("BLE write failed: {}", e))
                })?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> { Ok(()) }
}

impl Drop for SpikeBleTransport {
    fn drop(&mut self) {
        let _ = self.runtime.block_on(self.peripheral.disconnect());
    }
}
