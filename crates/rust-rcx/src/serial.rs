use std::io::{Read, Write};
use std::time::{Duration, Instant};
use serialport::SerialPort;
use crate::constants::*;
use crate::protocol;

/// RCX communication via serial IR tower.
pub struct RcxSerial {
    port: Box<dyn SerialPort>,
}

impl RcxSerial {
    /// Open a serial IR tower at the given path.
    pub fn open(path: &str) -> Result<Self, String> {
        let port = serialport::new(path, DEFAULT_BAUD_RATE)
            .parity(serialport::Parity::Odd)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| format!("Failed to open serial port {}: {}", path, e))?;

        Ok(RcxSerial { port })
    }

    /// Send a pre-framed message and return the raw reply bytes.
    pub fn send(&mut self, msg: &[u8]) -> Result<(), String> {
        self.port.write_all(msg).map_err(|e| format!("Write failed: {}", e))?;
        self.port.flush().map_err(|e| format!("Flush failed: {}", e))?;
        Ok(())
    }

    /// Send a message and wait for a reply. Returns the parsed reply payload.
    pub fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.send(msg)?;

        let deadline = Instant::now() + Duration::from_millis(FIRMWARE_TIMEOUT_MS);
        let mut buf = [0u8; 256];
        let mut response = Vec::new();

        while Instant::now() < deadline {
            match self.port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    // Try parsing — the reply might be complete
                    // Skip echo of our sent message first
                    if let Some(payload) = find_reply_after_echo(&response, msg.len()) {
                        return Ok(payload);
                    }
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(format!("Read failed: {}", e)),
            }
        }

        Err("RCX reply timed out".to_string())
    }

    /// Check if the RCX is alive.
    pub fn ping(&mut self) -> Result<bool, String> {
        let msg = protocol::cmd_alive();
        match self.request(&msg) {
            Ok(payload) => Ok(!payload.is_empty()),
            Err(_) => Ok(false),
        }
    }

    /// Get a clone of the serial port for the driver thread.
    pub fn try_clone_port(&self) -> Result<Box<dyn SerialPort>, String> {
        self.port.try_clone().map_err(|e| format!("Clone failed: {}", e))
    }
}

/// The serial tower echoes back what we send. Skip past the echo to find the RCX reply.
fn find_reply_after_echo(data: &[u8], sent_len: usize) -> Option<Vec<u8>> {
    if data.len() <= sent_len {
        return None;
    }
    protocol::parse_reply(&data[sent_len..])
}

#[cfg(test)]
#[path = "tests/serial.rs"]
mod tests;
