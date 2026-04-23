//! Transport abstraction for LCP. Implementations exchange raw LCP bytes
//! (no length prefix); the Bluetooth impl wraps and unwraps the 2-byte
//! little-endian length header internally, and USB leaves the bulk
//! endpoint to do the framing.

use std::time::Duration;

pub trait Transport: Send {
    /// Send one LCP packet (no length prefix). The transport is responsible
    /// for any transport-level framing.
    fn send(&mut self, lcp_bytes: &[u8]) -> Result<(), String>;

    /// Receive one LCP packet, with its framing already stripped. Blocks up
    /// to `timeout`.
    fn recv(&mut self, timeout: Duration) -> Result<Vec<u8>, String>;
}
