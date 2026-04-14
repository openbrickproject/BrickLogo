//! Wi-Fi transport for EV3 — stubbed until implemented.
//!
//! The real protocol:
//!   1. UDP broadcast from the brick on port 3015 announces presence with
//!      its serial number:
//!      `Serial-No: XXXXXXXXXXXX\r\nPort: 5555\r\nName: EV3\r\nProtocol: EV3\r\n`
//!   2. TCP connect to the advertised IP on port 5555.
//!   3. HTTP-ish handshake:
//!      request  `GET /target?sn=<serial> VMTP1.0\r\nProtocol: EV3\r\n`
//!      response `Accept:EV340\r\n\r\n`
//!   4. From then on, the same Direct Command stream as USB / SPP.
//!
//! Requires a Netgear WNA1100 USB Wi-Fi dongle on the brick (the only
//! officially-supported adapter).

use std::time::Duration;

use crate::transport::Transport;

pub enum WifiTarget {
    /// Listen for a UDP broadcast on port 3015 and pick the first
    /// responder.
    Discover,
    /// Connect directly to the given IP (or host) on TCP port 5555.
    Address(String),
}

pub struct WifiTransport;

impl WifiTransport {
    pub fn open(_target: WifiTarget) -> Result<Self, String> {
        Err("EV3 Wi-Fi transport not yet implemented".to_string())
    }
}

impl Transport for WifiTransport {
    fn send(&mut self, _frame_bytes: &[u8]) -> Result<(), String> {
        unreachable!("WifiTransport cannot be constructed yet")
    }

    fn recv(&mut self, _timeout: Duration) -> Result<Vec<u8>, String> {
        unreachable!("WifiTransport cannot be constructed yet")
    }
}
