//! USB bulk transport for the NXT.
//!
//! The stock NXT firmware presents itself as a vendor-specific bulk device
//! with one interface and one bulk pair (OUT at 0x01, IN at 0x82). The
//! `bInterfaceNumber` in the config descriptor varies across firmware
//! revisions (nxt-python picks whichever interface the config exposes
//! rather than hard-coding one), so we scan the active configuration at
//! open time and claim whatever's there.

use std::time::Duration;

use rusb::{Direction, TransferType};

use crate::transport::Transport;

pub const NXT_VID: u16 = 0x0694;
pub const NXT_PID: u16 = 0x0002;

pub struct UsbTransport {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    iface: u8,
    ep_out: u8,
    ep_in: u8,
}

impl UsbTransport {
    /// Enumerate NXT bricks on the USB bus and return their serial numbers
    /// (as reported by the iSerial string descriptor, usually the brick's
    /// Bluetooth MAC address with colons stripped).
    pub fn enumerate() -> Result<Vec<String>, String> {
        let devices = rusb::devices().map_err(|e| format!("USB enumeration failed: {}", e))?;
        let mut out = Vec::new();
        for device in devices.iter() {
            let desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };
            if desc.vendor_id() != NXT_VID || desc.product_id() != NXT_PID {
                continue;
            }
            let handle = match device.open() {
                Ok(h) => h,
                Err(_) => continue,
            };
            let serial = handle
                .read_serial_number_string_ascii(&desc)
                .unwrap_or_default();
            out.push(serial);
        }
        Ok(out)
    }

    /// Open the first (or named) NXT brick on USB and claim its bulk
    /// interface.
    pub fn open(serial: Option<&str>) -> Result<Self, String> {
        let devices = rusb::devices().map_err(|e| format!("USB enumeration failed: {}", e))?;
        for device in devices.iter() {
            let desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };
            if desc.vendor_id() != NXT_VID || desc.product_id() != NXT_PID {
                continue;
            }
            let handle = match device.open() {
                Ok(h) => h,
                Err(e) => return Err(format!("Failed to open NXT USB device: {}", e)),
            };

            if let Some(want) = serial {
                let got = handle
                    .read_serial_number_string_ascii(&desc)
                    .unwrap_or_default();
                if got != want {
                    continue;
                }
            }

            // Walk the active config's interfaces looking for a bulk pair.
            // The NXT only has one interface, but `bInterfaceNumber` varies
            // across firmware revisions — older firmware reports 0, newer
            // (and some rebuilds) report 1 — so we don't hard-code it.
            let active_config = handle
                .active_configuration()
                .map_err(|e| format!("Failed to read NXT USB config: {}", e))?;
            let config_desc = device
                .config_descriptor(active_config.saturating_sub(1))
                .map_err(|e| format!("Failed to read NXT config descriptor: {}", e))?;

            let mut pick: Option<(u8, u8, u8)> = None;
            for interface in config_desc.interfaces() {
                for alt in interface.descriptors() {
                    let mut bulk_out = None;
                    let mut bulk_in = None;
                    for ep in alt.endpoint_descriptors() {
                        if ep.transfer_type() != TransferType::Bulk {
                            continue;
                        }
                        match ep.direction() {
                            Direction::Out if bulk_out.is_none() => {
                                bulk_out = Some(ep.address());
                            }
                            Direction::In if bulk_in.is_none() => {
                                bulk_in = Some(ep.address());
                            }
                            _ => {}
                        }
                    }
                    if let (Some(out), Some(inn)) = (bulk_out, bulk_in) {
                        pick = Some((interface.number(), out, inn));
                        break;
                    }
                }
                if pick.is_some() {
                    break;
                }
            }

            let (iface, ep_out, ep_in) = pick.ok_or_else(|| {
                "NXT USB device has no bulk-in/bulk-out interface".to_string()
            })?;

            #[cfg(target_os = "linux")]
            {
                if handle.kernel_driver_active(iface).unwrap_or(false) {
                    let _ = handle.detach_kernel_driver(iface);
                }
            }

            handle
                .claim_interface(iface)
                .map_err(|e| {
                    format!("Failed to claim NXT USB interface {}: {}", iface, e)
                })?;

            return Ok(UsbTransport {
                handle,
                iface,
                ep_out,
                ep_in,
            });
        }
        Err("No NXT brick found on USB".to_string())
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.iface);
    }
}

impl Transport for UsbTransport {
    fn send(&mut self, lcp_bytes: &[u8]) -> Result<(), String> {
        self.handle
            .write_bulk(self.ep_out, lcp_bytes, Duration::from_millis(500))
            .map(|_| ())
            .map_err(|e| format!("NXT USB write failed: {}", e))
    }

    fn recv(&mut self, timeout: Duration) -> Result<Vec<u8>, String> {
        let mut buf = [0u8; 64];
        let n = self
            .handle
            .read_bulk(self.ep_in, &mut buf, timeout)
            .map_err(|e| format!("NXT USB read failed: {}", e))?;
        Ok(buf[..n].to_vec())
    }
}
