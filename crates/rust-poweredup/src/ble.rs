use btleplug::api::{Central, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Peripheral};
use futures::{FutureExt, StreamExt};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::constants::*;
use crate::hub::Hub;
use crate::hub::HubEvent;
use crate::protocol::{self, HubPropertyValue, PortFeedback};

const LEGO_COMPANY_ID: u16 = 0x0397;

/// BLE-connected Powered UP hub.
pub struct PoweredUpBle {
    pub hub: Arc<Mutex<Hub>>,
    peripheral: Option<Peripheral>,
    runtime: Arc<Runtime>,
    adapter: Adapter,
    stop_flag: Option<Arc<AtomicBool>>,
    feedback_rx: Option<std::sync::mpsc::Receiver<PortFeedback>>,
}

impl PoweredUpBle {
    /// Construct a `PoweredUpBle` against a shared BLE context. The same
    /// `Runtime` and `Adapter` should be passed to every BLE-using adapter
    /// in the process — see `bricklogo-hal`'s `ble::ble_context()`.
    pub fn new(runtime: Arc<Runtime>, adapter: Adapter) -> Self {
        PoweredUpBle {
            hub: Arc::new(Mutex::new(Hub::new(HubType::Unknown))),
            peripheral: None,
            runtime,
            adapter,
            stop_flag: None,
            feedback_rx: None,
        }
    }

    pub fn set_stop_flag(&mut self, flag: Arc<AtomicBool>) {
        self.stop_flag = Some(flag);
    }

    fn is_stop_requested(&self) -> bool {
        self.stop_flag
            .as_ref()
            .map_or(false, |f| f.load(Ordering::SeqCst))
    }

    /// Scan and connect to the first unclaimed Powered UP hub found.
    pub fn connect(&mut self) -> Result<(), String> {
        let adapter = &self.adapter;

        self.runtime.block_on(async {
            adapter
                .start_scan(ScanFilter::default())
                .await
                .map_err(|e| format!("Scan failed: {}", e))
        })?;

        let lpf2_uuid = Uuid::parse_str(LPF2_SERVICE_UUID).unwrap();
        let wedo2_uuid = Uuid::parse_str(WEDO2_SERVICE_UUID).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);

        loop {
            if std::time::Instant::now() > deadline {
                let _ = self.runtime.block_on(adapter.stop_scan());
                return Err("No Powered UP hub found (timeout)".to_string());
            }

            if self.is_stop_requested() {
                let _ = self.runtime.block_on(adapter.stop_scan());
                return Err("Cancelled".to_string());
            }

            let peripherals = self.runtime.block_on(async {
                adapter
                    .peripherals()
                    .await
                    .map_err(|e| format!("Scan error: {}", e))
            })?;

            for p in peripherals {
                // Skip peripherals already connected — either by this adapter
                // instance or by another `PoweredUpBle` / `CoralBle` sharing
                // the same central. btleplug's `connect()` is idempotent, so
                // without this check a second `connectto "pup` would silently
                // re-latch onto the already-owned hub.
                if self.runtime.block_on(p.is_connected()).unwrap_or(false) {
                    continue;
                }

                let props = match self.runtime.block_on(p.properties()) {
                    Ok(Some(props)) => props,
                    _ => continue,
                };

                let is_lpf2 = props.services.contains(&lpf2_uuid);
                let is_wedo2 = props.services.contains(&wedo2_uuid);

                if !is_lpf2 && !is_wedo2 {
                    continue;
                }

                let _ = self.runtime.block_on(adapter.stop_scan());

                // Identify hub type
                let hub_type = if is_wedo2 {
                    HubType::WeDo2SmartHub
                } else {
                    props
                        .manufacturer_data
                        .iter()
                        .find(|(id, _)| **id == LEGO_COMPANY_ID)
                        .and_then(|(_, data)| {
                            // btleplug strips company ID; hub type byte is at index 1
                            if data.len() >= 2 {
                                Some(hub_type_from_manufacturer_byte(data[1]))
                            } else {
                                None
                            }
                        })
                        .unwrap_or(HubType::Unknown)
                };

                // Connect
                self.runtime.block_on(async {
                    p.connect()
                        .await
                        .map_err(|e| format!("Connect failed: {}", e))
                })?;
                self.runtime.block_on(async {
                    p.discover_services()
                        .await
                        .map_err(|e| format!("Service discovery failed: {}", e))
                })?;

                // Create notification stream before subscribing
                let mut stream = self.runtime.block_on(async {
                    p.notifications()
                        .await
                        .map_err(|e| format!("Notification stream failed: {}", e))
                })?;

                // Subscribe to characteristics
                let chars = p.characteristics();

                if is_wedo2 {
                    for uuid_str in &[
                        WEDO2_PORT_TYPE_UUID,
                        WEDO2_SENSOR_VALUE_UUID,
                        WEDO2_BUTTON_UUID,
                    ] {
                        let uuid = Uuid::parse_str(uuid_str).unwrap();
                        if let Some(c) = chars.iter().find(|c| c.uuid == uuid) {
                            let _ = self.runtime.block_on(p.subscribe(c));
                        }
                    }
                } else {
                    let lpf2_char_uuid = Uuid::parse_str(LPF2_CHARACTERISTIC_UUID).unwrap();
                    if let Some(c) = chars.iter().find(|c| c.uuid == lpf2_char_uuid) {
                        self.runtime.block_on(async {
                            p.subscribe(c)
                                .await
                                .map_err(|e| format!("Subscribe failed: {}", e))
                        })?;
                    }
                }

                // Set up hub state
                let hub = Arc::new(Mutex::new(Hub::new(hub_type)));
                hub.lock().unwrap().on_connected();
                self.hub = hub.clone();

                // Spawn background task that processes all notifications
                let (feedback_tx, feedback_rx) = std::sync::mpsc::channel();
                let hub_clone = hub.clone();
                let is_wedo2_hub = hub_type.is_wedo2();

                let port_type_uuid = Uuid::parse_str(WEDO2_PORT_TYPE_UUID).unwrap();
                let sensor_uuid = Uuid::parse_str(WEDO2_SENSOR_VALUE_UUID).unwrap();
                let button_uuid = Uuid::parse_str(WEDO2_BUTTON_UUID).unwrap();

                self.runtime.spawn(async move {
                    // Catch panics from the underlying notification stream
                    // (bluez-async on Linux can panic during teardown when a
                    // peripheral disconnects mid-flight). The health monitor
                    // in `bricklogo-hal::health` will notice the dead
                    // connection on its next poll and remove the device.
                    let _ = AssertUnwindSafe(async move {
                        while let Some(notif) = stream.next().await {
                            let mut hub = hub_clone.lock().unwrap();
                            let events = if is_wedo2_hub {
                                if notif.uuid == port_type_uuid {
                                    hub.process_wedo2_port_type(&notif.value)
                                } else if notif.uuid == sensor_uuid {
                                    hub.process_wedo2_sensor_value(&notif.value)
                                } else if notif.uuid == button_uuid {
                                    let pressed = notif.value.first().copied().unwrap_or(0) == 1;
                                    vec![HubEvent::PropertyUpdate(HubPropertyValue::Button(pressed))]
                                } else {
                                    vec![]
                                }
                            } else {
                                hub.process_message(&notif.value)
                            };

                            // Forward command feedback events
                            for event in &events {
                                if let HubEvent::CommandFeedback {
                                    port_id,
                                    completed,
                                    discarded,
                                } = event
                                {
                                    let _ = feedback_tx.send(PortFeedback {
                                        port_id: *port_id,
                                        feedback: if *completed {
                                            0x0a
                                        } else if *discarded {
                                            0x04
                                        } else {
                                            0x00
                                        },
                                    });
                                }
                            }
                        }
                    }).catch_unwind().await;
                });

                self.feedback_rx = Some(feedback_rx);
                self.peripheral = Some(p.clone());

                // Request hub properties
                if !is_wedo2 {
                    let cmd = protocol::cmd_request_property(HubProperty::BatteryVoltage);
                    self.send_to(&p, &cmd)?;
                    let cmd = protocol::cmd_enable_property_updates(HubProperty::BatteryVoltage);
                    self.send_to(&p, &cmd)?;
                }

                // Wait briefly for initial device attach messages
                std::thread::sleep(Duration::from_millis(500));

                return Ok(());
            }

            std::thread::sleep(Duration::from_millis(200));
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(ref p) = self.peripheral {
            let _ = self.runtime.block_on(p.disconnect());
        }
        self.peripheral = None;
        self.feedback_rx = None;
        self.hub.lock().unwrap().on_disconnected();
    }

    pub fn is_connected(&self) -> bool {
        self.hub.lock().unwrap().is_connected()
    }

    /// Send a raw command to the hub.
    pub fn send(&self, data: &[u8]) -> Result<(), String> {
        let peripheral = self.peripheral.as_ref().ok_or("Not connected")?;

        if self.hub.lock().unwrap().hub_type.is_wedo2() {
            let uuid = Uuid::parse_str(WEDO2_MOTOR_VALUE_WRITE_UUID).unwrap();
            let chars = peripheral.characteristics();
            let c = chars
                .iter()
                .find(|c| c.uuid == uuid)
                .ok_or("Motor write characteristic not found")?;
            self.runtime.block_on(async {
                peripheral
                    .write(c, data, WriteType::WithoutResponse)
                    .await
                    .map_err(|e| format!("Write failed: {}", e))
            })
        } else {
            self.send_to(peripheral, data)
        }
    }

    fn send_to(&self, peripheral: &Peripheral, data: &[u8]) -> Result<(), String> {
        let uuid = Uuid::parse_str(LPF2_CHARACTERISTIC_UUID).unwrap();
        let chars = peripheral.characteristics();
        let c = chars
            .iter()
            .find(|c| c.uuid == uuid)
            .ok_or("LPF2 characteristic not found")?;
        self.runtime.block_on(async {
            peripheral
                .write(c, data, WriteType::WithoutResponse)
                .await
                .map_err(|e| format!("Write failed: {}", e))
        })
    }

    /// Send to a specific WeDo 2.0 characteristic.
    pub fn send_wedo2(&self, char_uuid: &str, data: &[u8]) -> Result<(), String> {
        let peripheral = self.peripheral.as_ref().ok_or("Not connected")?;
        let uuid = Uuid::parse_str(char_uuid).unwrap();
        let chars = peripheral.characteristics();
        let c = chars
            .iter()
            .find(|c| c.uuid == uuid)
            .ok_or("WeDo2 characteristic not found")?;
        self.runtime.block_on(async {
            peripheral
                .write(c, data, WriteType::WithoutResponse)
                .await
                .map_err(|e| format!("Write failed: {}", e))
        })
    }

    /// Send a command and wait for command feedback (completion or discard).
    pub fn request(&self, port_id: u8, data: &[u8]) -> Result<bool, String> {
        self.send(data)?;

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let rx = self.feedback_rx.as_ref().ok_or("No feedback channel")?;

        loop {
            if std::time::Instant::now() > deadline {
                return Err("Request timed out".to_string());
            }
            if self.is_stop_requested() {
                return Err("Cancelled".to_string());
            }

            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(fb) => {
                    if fb.port_id == port_id && (fb.is_completed() || fb.is_discarded()) {
                        return Ok(fb.is_completed());
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Connection lost".to_string());
                }
            }
        }
    }

    /// Send multiple commands and wait for all their feedback.
    pub fn request_all(&self, commands: &[(u8, Vec<u8>)]) -> Result<(), String> {
        let mut pending: Vec<u8> = Vec::new();
        for (port_id, data) in commands {
            self.send(data)?;
            pending.push(*port_id);
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let rx = self.feedback_rx.as_ref().ok_or("No feedback channel")?;

        while !pending.is_empty() {
            if std::time::Instant::now() > deadline {
                return Err("Request timed out".to_string());
            }
            if self.is_stop_requested() {
                return Err("Cancelled".to_string());
            }

            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(fb) => {
                    if fb.is_completed() || fb.is_discarded() {
                        pending.retain(|&id| id != fb.port_id);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Connection lost".to_string());
                }
            }
        }
        Ok(())
    }

    /// Subscribe to a sensor mode on a port.
    pub fn subscribe(&self, port_id: u8, mode: u8) -> Result<(), String> {
        let (is_wedo2, device_type) = {
            let hub = self.hub.lock().unwrap();
            let dt = hub.get_device(port_id).map(|d| d.device_type as u8);
            (hub.hub_type.is_wedo2(), dt)
        };
        if is_wedo2 {
            let dt = device_type
                .ok_or_else(|| format!("No device on port {} to subscribe", port_id))?;
            let cmd = protocol::wedo2_cmd_subscribe(port_id, dt, mode);
            self.send_wedo2(WEDO2_PORT_TYPE_WRITE_UUID, &cmd)?;
        } else {
            let cmd = protocol::cmd_subscribe(port_id, mode);
            self.send(&cmd)?;
        }
        self.hub.lock().unwrap().set_subscribed_mode(port_id, mode);
        Ok(())
    }
}
