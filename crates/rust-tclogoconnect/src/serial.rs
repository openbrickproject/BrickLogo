//! Serial transport for the TC Logo Connect's USB-CDC link.
//!
//! Opens at 115200 8N1 with flow control explicitly disabled — the device
//! has no hardware handshaking, and on Windows leaving flow control at its
//! default can leave writes waiting on a CTS/DSR state the device never
//! asserts, hanging the write.
//!
//! NEVER open at 1200 baud: the RP2040 firmware treats a 1200-baud open as
//! a request to reboot into the bootloader (the "1200bps touch" convention
//! shared by other RP2040 boards).

use std::io::{Read, Write};
use std::time::Duration;

use serialport::SerialPort;

use crate::constants::BAUD_RATE;
use crate::transport::Transport;

pub struct SerialTransport {
    port: Box<dyn SerialPort>,
}

impl SerialTransport {
    pub fn open(path: &str) -> Result<Self, String> {
        let port = serialport::new(path, BAUD_RATE)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(20))
            .open()
            .map_err(|e| format!("Failed to open serial port {}: {}", path, e))?;
        Ok(SerialTransport { port })
    }
}

impl Transport for SerialTransport {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.port
            .write_all(bytes)
            .map_err(|e| format!("Write failed: {}", e))?;
        self.port.flush().map_err(|e| format!("Flush failed: {}", e))
    }

    fn read_available(&mut self) -> Result<Vec<u8>, String> {
        let available = self.port.bytes_to_read().unwrap_or(0) as usize;
        if available == 0 {
            return Ok(Vec::new());
        }
        let mut buf = vec![0u8; available];
        match self.port.read(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(Vec::new()),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
            Err(e) => Err(format!("Read failed: {}", e)),
        }
    }
}
