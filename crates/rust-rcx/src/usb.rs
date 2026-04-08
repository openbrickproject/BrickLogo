use std::time::{Duration, Instant};
use rusb::{Context, DeviceHandle, UsbContext};
use crate::constants::*;
use crate::protocol;

/// RCX communication via USB IR tower.
pub struct RcxUsb {
    handle: DeviceHandle<Context>,
    endpoint_in: u8,
    endpoint_out: u8,
}

impl RcxUsb {
    /// Scan for and open the LEGO USB IR tower.
    pub fn open() -> Result<Self, String> {
        let context = Context::new().map_err(|e| format!("USB init failed: {}", e))?;

        let device = context.devices()
            .map_err(|e| format!("USB scan failed: {}", e))?
            .iter()
            .find(|d| {
                d.device_descriptor().map_or(false, |desc| {
                    desc.vendor_id() == USB_VENDOR_ID && desc.product_id() == USB_PRODUCT_ID
                })
            })
            .ok_or("No LEGO USB IR tower found")?;

        let handle = device.open()
            .map_err(|e| format!("Failed to open USB tower: {}", e))?;

        // Claim interface 0
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(0)
            .map_err(|e| format!("Failed to claim USB interface: {}", e))?;

        // Find interrupt endpoints
        let config = device.active_config_descriptor()
            .map_err(|e| format!("Failed to get USB config: {}", e))?;

        let mut ep_in: Option<u8> = None;
        let mut ep_out: Option<u8> = None;

        for iface in config.interfaces() {
            for desc in iface.descriptors() {
                for ep in desc.endpoint_descriptors() {
                    match ep.direction() {
                        rusb::Direction::In => { ep_in = Some(ep.address()); }
                        rusb::Direction::Out => { ep_out = Some(ep.address()); }
                    }
                }
            }
        }

        let endpoint_in = ep_in.ok_or("USB tower: no IN endpoint found")?;
        let endpoint_out = ep_out.ok_or("USB tower: no OUT endpoint found")?;

        Ok(RcxUsb { handle, endpoint_in, endpoint_out })
    }

    /// Send a pre-framed message.
    pub fn send(&self, msg: &[u8]) -> Result<(), String> {
        let timeout = Duration::from_millis(USB_TIMEOUT_MS);
        self.handle.write_interrupt(self.endpoint_out, msg, timeout)
            .map_err(|e| format!("USB write failed: {}", e))?;
        Ok(())
    }

    /// Send a message and wait for a reply. Returns the parsed reply payload.
    /// Unlike serial, the USB tower does not echo back the sent message.
    pub fn request(&self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.send(msg)?;

        let deadline = Instant::now() + Duration::from_millis(USB_TIMEOUT_MS);
        let mut response = Vec::new();
        let mut buf = [0u8; 64];

        while Instant::now() < deadline {
            let remaining = deadline.duration_since(Instant::now());
            match self.handle.read_interrupt(self.endpoint_in, &mut buf, remaining) {
                Ok(n) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    if let Some(payload) = protocol::parse_reply(&response) {
                        return Ok(payload);
                    }
                }
                Ok(_) => {}
                Err(rusb::Error::Timeout) => {}
                Err(e) => return Err(format!("USB read failed: {}", e)),
            }
        }

        Err("RCX reply timed out".to_string())
    }

    /// Check if the RCX is alive.
    pub fn ping(&self) -> Result<bool, String> {
        let msg = protocol::cmd_alive();
        match self.request(&msg) {
            Ok(payload) => Ok(!payload.is_empty()),
            Err(_) => Ok(false),
        }
    }

    /// Consume into parts for use as a transport.
    pub fn into_parts(self) -> (DeviceHandle<Context>, u8, u8) {
        (self.handle, self.endpoint_in, self.endpoint_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // USB tests require hardware — just test that open fails gracefully
    #[test]
    fn test_open_no_device() {
        // Should return an error, not panic
        let result = RcxUsb::open();
        // Either succeeds (if a tower is connected) or fails with an error message
        if let Err(e) = result {
            assert!(e.contains("No LEGO USB IR tower found") || e.contains("USB"));
        }
    }
}
