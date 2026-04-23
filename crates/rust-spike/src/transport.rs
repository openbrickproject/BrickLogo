//! Abstract byte-level transport used by the SPIKE Prime Atlantis protocol.
//!
//! Lives in `rust-spike` so that both the adapter (in `bricklogo-hal`) and
//! the firmware uploader (in this crate) can speak to the same `&mut dyn
//! Transport`. Concrete implementations live in `bricklogo-hal`:
//! `SpikeSerialTransport` for USB CDC, `SpikeBleTransport` for BLE.

pub trait Transport: Send {
    /// Read up to `buf.len()` bytes. Return `0` on no data, `Err` on a
    /// transport-level failure (closed port, disconnected peripheral).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String>;
    /// Write every byte in `data`.
    fn write_all(&mut self, data: &[u8]) -> Result<(), String>;
    fn flush(&mut self) -> Result<(), String>;
}
