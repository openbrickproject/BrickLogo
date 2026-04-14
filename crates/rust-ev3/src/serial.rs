//! Bluetooth SPP (or any other serial) transport for EV3.
//!
//! After the user pairs the EV3 at the OS level, the brick appears as a
//! serial device (`/dev/cu.EV3-SerialPort-*` on macOS, `/dev/rfcomm*` on
//! Linux after `rfcomm bind`, `COM*` on Windows). Unlike USB HID the
//! transport is stream-oriented — just write the Direct Command frame
//! and read it back, using the length prefix to know when the reply
//! ends.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use serialport::SerialPort;

use crate::transport::Transport;

pub struct SerialTransport {
    port: Box<dyn SerialPort>,
}

impl SerialTransport {
    /// Open the given serial path at the EV3's fixed bit rate (115200
    /// 8N1). The serialport crate sets sensible defaults for the other
    /// serial parameters.
    pub fn open(path: &str) -> Result<Self, String> {
        let port = serialport::new(path, 115_200)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| format!("Failed to open EV3 serial port {}: {}", path, e))?;
        Ok(SerialTransport { port })
    }
}

impl Transport for SerialTransport {
    fn send(&mut self, frame_bytes: &[u8]) -> Result<(), String> {
        self.port
            .write_all(frame_bytes)
            .map_err(|e| format!("Serial write failed: {}", e))?;
        self.port
            .flush()
            .map_err(|e| format!("Serial flush failed: {}", e))
    }

    fn recv(&mut self, timeout: Duration) -> Result<Vec<u8>, String> {
        let deadline = Instant::now() + timeout;
        let mut header = [0u8; 2];
        self.read_exact_until(&mut header, deadline)?;
        let length = u16::from_le_bytes(header) as usize;
        let mut out = Vec::with_capacity(length + 2);
        out.extend_from_slice(&header);
        let mut body = vec![0u8; length];
        self.read_exact_until(&mut body, deadline)?;
        out.extend_from_slice(&body);
        Ok(out)
    }
}

impl SerialTransport {
    /// Read exactly `buf.len()` bytes or return an error if the deadline
    /// passes. Handles the serialport crate's per-read timeouts.
    fn read_exact_until(&mut self, buf: &mut [u8], deadline: Instant) -> Result<(), String> {
        let mut filled = 0;
        while filled < buf.len() {
            if Instant::now() > deadline {
                return Err("EV3 serial read timeout".to_string());
            }
            match self.port.read(&mut buf[filled..]) {
                Ok(n) if n > 0 => filled += n,
                Ok(_) => std::thread::sleep(Duration::from_millis(1)),
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Per-read timeout fired; loop to check the overall
                    // deadline.
                }
                Err(e) => return Err(format!("Serial read failed: {}", e)),
            }
        }
        Ok(())
    }
}
