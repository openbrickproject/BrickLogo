//! Transport abstraction for the TC Logo Connect's USB-CDC serial link. A
//! trait so tests can inject a mock without opening a real port.

pub trait Transport: Send {
    /// Write bytes to the device (blocking until accepted by the OS).
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
    /// Read whatever bytes are currently available without blocking.
    /// Returns an empty vec if nothing is available yet.
    fn read_available(&mut self) -> Result<Vec<u8>, String>;
}
