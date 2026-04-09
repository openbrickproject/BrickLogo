#[allow(unused_imports)]
use std::io::Read;
use std::time::{Duration, Instant};
use crate::constants::*;
use crate::protocol::*;

/// Progress callback: (phase description).
pub type ProgressFn = Box<dyn Fn(&str) + Send>;

/// Detect the current state of the Build HAT.
pub fn detect_state(port: &mut dyn serialport::SerialPort) -> Result<HatState, String> {
    let cmd = cmd_version();
    for _ in 0..5 {
        port.write_all(cmd.as_bytes()).map_err(|e| format!("Write failed: {}", e))?;
        port.flush().map_err(|e| format!("Flush failed: {}", e))?;

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut buf = [0u8; 256];
        let mut response = String::new();

        while Instant::now() < deadline {
            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    response.push_str(&String::from_utf8_lossy(&buf[..n]));
                    for line in response.lines() {
                        if let Some(state) = parse_version(line) {
                            return Ok(state);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Err("No Build HAT detected".to_string())
}

/// Upload firmware to the Build HAT.
pub fn upload_firmware(
    port: &mut dyn serialport::SerialPort,
    firmware: &[u8],
    signature: &[u8],
    progress: &ProgressFn,
) -> Result<(), String> {
    progress("Clearing Build HAT...");
    write_and_flush(port, cmd_clear().as_bytes())?;
    wait_for_prompt(port, "BHBL>")?;

    progress("Uploading firmware...");
    let checksum = firmware_checksum(firmware);
    let load_cmd = cmd_load(firmware.len(), checksum);
    write_and_flush(port, load_cmd.as_bytes())?;
    std::thread::sleep(Duration::from_millis(100));

    // Send firmware binary: STX + data + ETX
    write_and_flush(port, &[STX])?;
    write_and_flush(port, firmware)?;
    write_and_flush(port, &[ETX, b'\r'])?;
    wait_for_prompt(port, "BHBL>")?;

    progress("Uploading signature...");
    let sig_cmd = cmd_signature(signature.len());
    write_and_flush(port, sig_cmd.as_bytes())?;
    std::thread::sleep(Duration::from_millis(100));

    write_and_flush(port, &[STX])?;
    write_and_flush(port, signature)?;
    write_and_flush(port, &[ETX, b'\r'])?;
    wait_for_prompt(port, "BHBL>")?;

    progress("Rebooting Build HAT...");
    write_and_flush(port, cmd_reboot().as_bytes())?;

    // Wait for firmware to boot
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut buf = [0u8; 256];
    let mut response = String::new();
    while Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                response.push_str(&String::from_utf8_lossy(&buf[..n]));
                for line in response.lines() {
                    if let Some(HatState::Firmware(_)) = parse_version(line) {
                        progress("Build HAT firmware ready");
                        return Ok(());
                    }
                }
            }
            _ => {}
        }
    }

    Err("Build HAT did not respond after firmware upload".to_string())
}

fn write_and_flush(port: &mut dyn serialport::SerialPort, data: &[u8]) -> Result<(), String> {
    port.write_all(data).map_err(|e| format!("Write failed: {}", e))?;
    port.flush().map_err(|e| format!("Flush failed: {}", e))?;
    Ok(())
}

fn wait_for_prompt(port: &mut dyn serialport::SerialPort, prompt: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut buf = [0u8; 256];
    let mut response = String::new();

    while Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                response.push_str(&String::from_utf8_lossy(&buf[..n]));
                if response.contains(prompt) {
                    return Ok(());
                }
            }
            _ => {}
        }
    }

    Err(format!("Timed out waiting for '{}'", prompt))
}
