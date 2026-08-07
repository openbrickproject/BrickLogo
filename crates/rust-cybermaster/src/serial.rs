use serialport::SerialPort;
use std::time::Duration;
use crate::constants::*;

/// Open a CyberMaster RF tower serial port, configured for the tower's line
/// control: 2400 baud / 8 data bits / odd parity / 1 stop bit, with DTR
/// asserted and RTS de-asserted (NQC's kCyberMasterMode).
pub fn open_port(path: &str) -> Result<Box<dyn SerialPort>, String> {
    let mut port = serialport::new(path, DEFAULT_BAUD_RATE)
        .parity(serialport::Parity::Odd)
        .data_bits(serialport::DataBits::Eight)
        .stop_bits(serialport::StopBits::One)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| format!("Failed to open serial port {}: {}", path, e))?;

    // The CH-style tower gates the radio on these signals: DTR high, RTS low.
    port.write_data_terminal_ready(true)
        .map_err(|e| format!("Failed to assert DTR on {}: {}", path, e))?;
    port.write_request_to_send(false)
        .map_err(|e| format!("Failed to clear RTS on {}: {}", path, e))?;

    Ok(port)
}
