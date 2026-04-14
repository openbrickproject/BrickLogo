//! Transport abstraction — USB HID, Bluetooth SPP serial, or Wi-Fi. Each
//! carries the same Direct Command wire protocol, only the framing differs.

use std::time::Duration;

pub trait Transport: Send {
    /// Send a fully-framed Direct Command (length prefix included).
    fn send(&mut self, frame_bytes: &[u8]) -> Result<(), String>;
    /// Receive one complete reply frame. Returns the raw bytes beginning
    /// with the length prefix.
    fn recv(&mut self, timeout: Duration) -> Result<Vec<u8>, String>;
}
