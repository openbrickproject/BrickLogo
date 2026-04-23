//! Bluetooth SPP transport for the NXT.
//!
//! After the user pairs the brick (PIN `1234`) at the OS level, it appears
//! as a serial device — `/dev/cu.NXT-DevB` on macOS, `/dev/rfcomm<N>` on
//! Linux after `rfcomm bind`, `COM<N>` on Windows. Unlike USB, Bluetooth
//! wraps every LCP packet in a 2-byte little-endian length prefix in both
//! directions. This transport adds and strips that prefix so the protocol
//! layer never has to care.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use serialport::SerialPort;

use crate::transport::Transport;

pub struct SerialTransport {
    port: Box<dyn SerialPort>,
}

impl SerialTransport {
    /// Open the given serial path. Baud is nominal — RFCOMM is packet-based
    /// and the `serialport` crate ignores it for BT devices on macOS / Linux,
    /// but we still have to supply a value.
    pub fn open(path: &str) -> Result<Self, String> {
        let port = serialport::new(path, 115_200)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| format!("Failed to open NXT serial port {}: {}", path, e))?;
        Ok(SerialTransport { port })
    }
}

impl Transport for SerialTransport {
    fn send(&mut self, lcp_bytes: &[u8]) -> Result<(), String> {
        if lcp_bytes.len() > u16::MAX as usize {
            return Err(format!("NXT LCP packet too large ({} bytes)", lcp_bytes.len()));
        }
        let length = lcp_bytes.len() as u16;
        let mut framed = Vec::with_capacity(2 + lcp_bytes.len());
        framed.extend_from_slice(&length.to_le_bytes());
        framed.extend_from_slice(lcp_bytes);
        self.port
            .write_all(&framed)
            .map_err(|e| format!("NXT serial write failed: {}", e))?;
        self.port
            .flush()
            .map_err(|e| format!("NXT serial flush failed: {}", e))
    }

    fn recv(&mut self, timeout: Duration) -> Result<Vec<u8>, String> {
        let deadline = Instant::now() + timeout;
        let mut header = [0u8; 2];
        self.read_exact_until(&mut header, deadline)?;
        let length = u16::from_le_bytes(header) as usize;
        let mut body = vec![0u8; length];
        self.read_exact_until(&mut body, deadline)?;
        Ok(body)
    }
}

impl SerialTransport {
    fn read_exact_until(&mut self, buf: &mut [u8], deadline: Instant) -> Result<(), String> {
        let mut filled = 0;
        while filled < buf.len() {
            if Instant::now() > deadline {
                return Err("NXT serial read timeout".to_string());
            }
            match self.port.read(&mut buf[filled..]) {
                Ok(n) if n > 0 => filled += n,
                Ok(_) => std::thread::sleep(Duration::from_millis(1)),
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Per-read timeout fired; loop re-checks the overall deadline.
                }
                Err(e) => return Err(format!("NXT serial read failed: {}", e)),
            }
        }
        Ok(())
    }
}
