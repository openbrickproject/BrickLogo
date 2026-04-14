//! USB HID transport for EV3.
//!
//! EV3 presents itself as HID device VID 0x0694 PID 0x0005. Each direction
//! uses 1024-byte reports. The first byte of each report is the HID report
//! ID (always 0x00 for EV3). The rest of the report carries the Direct
//! Command frame starting with its own length prefix; zero-pad to 1024.

use std::time::Duration;

use hidapi::{HidApi, HidDevice};

use crate::transport::Transport;

pub const EV3_VID: u16 = 0x0694;
pub const EV3_PID: u16 = 0x0005;
pub const HID_REPORT_SIZE: usize = 1024;

pub struct HidTransport {
    device: HidDevice,
}

impl HidTransport {
    /// Enumerate connected EV3 bricks and return their HID paths as UTF-8
    /// strings. Paths are platform-specific opaque identifiers that can be
    /// passed back to `open` to select a specific brick.
    pub fn enumerate() -> Result<Vec<String>, String> {
        let api = HidApi::new().map_err(|e| format!("HID init failed: {}", e))?;
        let paths = api
            .device_list()
            .filter(|d| d.vendor_id() == EV3_VID && d.product_id() == EV3_PID)
            .filter_map(|d| d.path().to_str().ok().map(|s| s.to_string()))
            .collect();
        Ok(paths)
    }

    /// Open an EV3 HID transport. If `path` is `None`, pick the first
    /// enumerated brick. If `Some`, open that specific path.
    pub fn open(path: Option<&str>) -> Result<Self, String> {
        let api = HidApi::new().map_err(|e| format!("HID init failed: {}", e))?;
        let device = if let Some(p) = path {
            let c_path = std::ffi::CString::new(p).map_err(|e| e.to_string())?;
            api.open_path(&c_path)
                .map_err(|e| format!("Failed to open EV3 at {}: {}", p, e))?
        } else {
            let info = api
                .device_list()
                .find(|d| d.vendor_id() == EV3_VID && d.product_id() == EV3_PID)
                .ok_or("No EV3 brick found on USB")?;
            api.open_path(info.path())
                .map_err(|e| format!("Failed to open EV3: {}", e))?
        };
        device
            .set_blocking_mode(false)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;
        Ok(HidTransport { device })
    }
}

impl Transport for HidTransport {
    fn send(&mut self, frame_bytes: &[u8]) -> Result<(), String> {
        if frame_bytes.len() + 1 > HID_REPORT_SIZE {
            return Err(format!(
                "Direct Command frame too large for HID report: {} bytes",
                frame_bytes.len()
            ));
        }
        // One HID report: [report_id=0x00] [frame bytes] [zero pad → 1024]
        let mut report = vec![0u8; HID_REPORT_SIZE];
        report[1..1 + frame_bytes.len()].copy_from_slice(frame_bytes);
        self.device
            .write(&report)
            .map(|_| ())
            .map_err(|e| format!("HID write failed: {}", e))
    }

    fn recv(&mut self, timeout: Duration) -> Result<Vec<u8>, String> {
        let deadline = std::time::Instant::now() + timeout;
        let mut buf = [0u8; HID_REPORT_SIZE];
        loop {
            if std::time::Instant::now() > deadline {
                return Err("EV3 HID read timeout".to_string());
            }
            match self.device.read(&mut buf) {
                Ok(n) if n >= 2 => {
                    // First two bytes of the HID payload are the Direct
                    // Command length prefix. Actual frame is length+2 bytes.
                    let length = u16::from_le_bytes([buf[0], buf[1]]) as usize;
                    let frame_len = length + 2;
                    if frame_len > n {
                        return Err(format!(
                            "HID report shorter than declared frame length ({} < {})",
                            n, frame_len
                        ));
                    }
                    return Ok(buf[..frame_len].to_vec());
                }
                Ok(_) => {
                    // Short read or zero bytes — keep polling until timeout.
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => return Err(format!("HID read failed: {}", e)),
            }
        }
    }
}
