//! USB-HID device wrapper for the ToyPad.
//!
//! Mirrors `rust-wedo`'s `WeDo`: discovery, connect (open + non-blocking +
//! wake), raw 32-byte frame read, and command send with a rolling request id.
//! The HAL adapter does its own event/response dispatch, so it may use
//! [`crate::protocol`] and [`crate::constants`] directly with its own
//! transport; this struct is for discovery and standalone use.

use hidapi::{HidApi, HidDevice};

use crate::constants::{PACKET_LENGTH, PRODUCT_ID, VENDOR_ID, WAKE_SEQUENCE};
use crate::protocol::{encode_command, Command};

/// Identifying info for a connected ToyPad.
#[derive(Debug, Clone)]
pub struct ToyPadDeviceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
}

/// True if at least one ToyPad is plugged in.
pub fn toypad_usb_present() -> bool {
    HidApi::new()
        .ok()
        .map(|api| {
            api.device_list()
                .any(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
        })
        .unwrap_or(false)
}

pub struct ToyPad {
    device: Option<HidDevice>,
    target_path: Option<String>,
    request_id: u8,
}

impl ToyPad {
    pub fn new(path: Option<&str>) -> Self {
        ToyPad {
            device: None,
            target_path: path.map(|s| s.to_string()),
            request_id: 0,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    /// Discover all connected ToyPads.
    pub fn discover() -> Result<Vec<ToyPadDeviceInfo>, String> {
        let api = HidApi::new().map_err(|e| format!("Failed to init HID: {}", e))?;
        Ok(api
            .device_list()
            .filter(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
            .filter_map(|d| {
                d.path().to_str().ok().map(|p| ToyPadDeviceInfo {
                    path: p.to_string(),
                    vendor_id: d.vendor_id(),
                    product_id: d.product_id(),
                })
            })
            .collect())
    }

    /// Open the ToyPad, set non-blocking mode, and send the wake sequence.
    pub fn connect(&mut self) -> Result<(), String> {
        let api = HidApi::new().map_err(|e| format!("Failed to init HID: {}", e))?;
        let device = if let Some(ref path) = self.target_path {
            let c_path = std::ffi::CString::new(path.as_str()).map_err(|e| e.to_string())?;
            api.open_path(&c_path)
                .map_err(|e| format!("Failed to open ToyPad at {}: {}", path, e))?
        } else {
            let info = api
                .device_list()
                .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
                .ok_or_else(|| "No ToyPad device found".to_string())?;
            api.open_path(info.path())
                .map_err(|e| format!("Failed to open ToyPad: {}", e))?
        };
        device
            .set_blocking_mode(false)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;
        device
            .write(&WAKE_SEQUENCE)
            .map_err(|e| format!("Failed to wake ToyPad: {}", e))?;
        self.device = Some(device);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.device = None;
    }

    /// Send a command with the next rolling request id (1..=255, skipping 0).
    pub fn send(&mut self, command: &Command) -> Result<u8, String> {
        let device = self.device.as_ref().ok_or("ToyPad not connected")?;
        self.request_id = self.request_id.wrapping_add(1);
        if self.request_id == 0 {
            self.request_id = 1;
        }
        let packet = encode_command(command, self.request_id);
        device
            .write(&packet)
            .map_err(|e| format!("Failed to write: {}", e))?;
        Ok(self.request_id)
    }

    /// Read one 32-byte frame (non-blocking). Returns `None` when no data is
    /// available.
    pub fn read_frame(&self) -> Result<Option<[u8; PACKET_LENGTH]>, String> {
        let device = self.device.as_ref().ok_or("ToyPad not connected")?;
        let mut buf = [0u8; PACKET_LENGTH + 1];
        match device.read(&mut buf) {
            Ok(0) => Ok(None),
            Ok(n) => {
                let mut frame = [0u8; PACKET_LENGTH];
                let m = n.min(PACKET_LENGTH);
                frame[..m].copy_from_slice(&buf[..m]);
                Ok(Some(frame))
            }
            Err(e) => Err(format!("Failed to read: {}", e)),
        }
    }
}
