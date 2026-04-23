//! `bricklogo --firmware <device> [--image <path>]` dispatch.
//!
//! Lives in the top-level binary because it's a standalone tool: opens a
//! transport, runs an upload, and exits. Neither REPL nor scheduler is
//! involved.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::cli::{FirmwareArgs, FirmwareDevice};

// ── Entry point ─────────────────────────────────

pub fn run(args: FirmwareArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.device {
        FirmwareDevice::Rcx => run_rcx(args.image).map_err(Into::into),
        FirmwareDevice::Spike => run_spike(args.image).map_err(Into::into),
    }
}

// ── RCX ─────────────────────────────────────────

fn run_rcx(image: Option<PathBuf>) -> Result<(), String> {
    let path = image.unwrap_or_else(|| bundled_dir().join("rcx").join("firm0332.lgo"));
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let image = rust_rcx::srec::parse_srec(&content)?;

    // Match `connectto "rcx` exactly: if `bricklogo.config.json` lists an
    // `rcx` array, use the first entry as the serial-tower path; otherwise
    // fall through to USB tower discovery.
    let config = bricklogo_tui::bridge::BrickLogoConfig::load();
    let serial_path = config.rcx.first().map(|s| s.as_str());
    let (mut transport, tower_label) = match serial_path {
        Some(path) => (
            bricklogo_hal::adapters::rcx_adapter::open_transport(Some(path))?,
            format!("serial IR tower at {}", path),
        ),
        None => (
            bricklogo_hal::adapters::rcx_adapter::open_transport(None)?,
            "USB IR tower".to_string(),
        ),
    };
    println!("LEGO Mindstorms RCX detected via {}.", tower_label);
    println!("Uploading {}...", path.display());
    let progress: rust_rcx::firmware::ProgressFn = Box::new(|current, total, phase| {
        print_progress(current, total, phase);
    });
    rust_rcx::firmware::upload_firmware(
        &image,
        &mut |msg| transport.request_firmware(msg),
        &progress,
    )?;
    println!("\nDone. The RCX will restart.");
    Ok(())
}

// ── SPIKE Prime ─────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpikeChip {
    F4,
    H5,
}

impl SpikeChip {
    fn from_iproduct(name: &str) -> Option<Self> {
        if name.contains("STM32F4") { Some(SpikeChip::F4) }
        else if name.contains("STM32H5") { Some(SpikeChip::H5) }
        else { None }
    }

    /// Identify the chip from the DFU interface string descriptor's flash
    /// memory map. STM32F413 (original SPIKE Prime / MINDSTORMS hardware)
    /// has mixed 16/64/128 KiB sectors; STM32H562 (refreshed hardware
    /// revision shipped from 2026) has 8 KiB uniform sectors. The split is
    /// about hub silicon generation, not about which retail product the
    /// hub shipped in — SPIKE Prime 45678 and MINDSTORMS Robot Inventor
    /// 51515 share hardware and firmware, with both revisions existing for
    /// each product.
    fn from_interface_desc(desc: &str) -> Option<Self> {
        let d = desc.to_ascii_uppercase();
        if d.contains("128*008K") || d.contains("*008K") && !d.contains("016K") {
            Some(SpikeChip::H5)
        } else if d.contains("016K") || d.contains("064K") || d.contains("128K") {
            Some(SpikeChip::F4)
        } else {
            None
        }
    }

    fn dfuse_gz_filename(self) -> &'static str {
        match self {
            SpikeChip::F4 => "prime-f4-hubos-3.4.0-dfuse.gz",
            SpikeChip::H5 => "prime-h5-hubos-3.4.0-dfuse.gz",
        }
    }

    /// Short human-readable label, used only in CLI output.
    fn label(self) -> &'static str {
        match self {
            SpikeChip::F4 => "original hardware",
            SpikeChip::H5 => "2026+ hardware revision",
        }
    }
}

pub enum SpikeMode {
    /// Hub is in DFU bootloader mode (entered via BT button + USB plug).
    Bootloader(rust_stm32dfu::dfu::BootloaderInfo),
    /// Hub is attached and running normally; carries the CDC serial path
    /// so we can query the hub for its current firmware version before
    /// asking the user to reboot into DFU mode.
    Normal { serial_path: String },
    None,
}

fn run_spike(image: Option<PathBuf>) -> Result<(), String> {
    match detect_spike().unwrap_or(SpikeMode::None) {
        SpikeMode::Bootloader(info) => run_spike_dfu(info, image),
        SpikeMode::Normal { serial_path } => {
            match query_spike_info(&serial_path) {
                Ok(info) => println!(
                    "SPIKE Prime hub detected (firmware {}.{}.{}).",
                    info.firmware_major, info.firmware_minor, info.firmware_build,
                ),
                Err(_) => println!("SPIKE Prime hub detected."),
            }
            print_enter_dfu_instructions();
            std::process::exit(1);
        }
        SpikeMode::None => {
            println!("No SPIKE Prime hub detected.");
            print_enter_dfu_instructions();
            std::process::exit(1);
        }
    }
}

fn print_enter_dfu_instructions() {
    println!();
    println!("To update firmware, activate the hub's DFU mode:");
    println!("  1. Unplug the USB cable.");
    println!("  2. Remove the battery, then reinsert it. Leave the hub powered off.");
    println!("  3. Hold the Bluetooth button on the hub.");
    println!("  4. Plug the USB cable back in while still holding the button.");
    println!("  5. Release when the light ring starts pulsing rainbow.");
    println!("Then run `bricklogo --firmware spike` again.");
}

/// Firmware upload. Uses the bundled `.dfuse.gz` matched by hub hardware.
fn run_spike_dfu(
    info: rust_stm32dfu::dfu::BootloaderInfo,
    image: Option<PathBuf>,
) -> Result<(), String> {
    let chip = SpikeChip::from_interface_desc(&info.interface_desc)
        .or_else(|| SpikeChip::from_iproduct(&info.iproduct))
        .ok_or_else(|| "could not identify hub hardware revision".to_string())?;
    println!("SPIKE Prime hub ({}) in DFU mode.", chip.label());

    let path = image.unwrap_or_else(|| bundled_dir().join("spike-prime").join(chip.dfuse_gz_filename()));
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let file = if path.extension().and_then(|s| s.to_str()) == Some("gz") {
        rust_stm32dfu::dfuse::parse_gz(&bytes).map_err(|e| e.to_string())?
    } else {
        rust_stm32dfu::dfuse::parse(&bytes).map_err(|e| e.to_string())?
    };
    println!("Uploading {}...", path.display());

    let mut dev = rust_stm32dfu::dfu::open(&info).map_err(|e| e.to_string())?;
    let progress: rust_stm32dfu::ProgressFn = Box::new(|done, total, phase| {
        print_progress(done, total, phase);
    });
    dev.download(&file, &progress).map_err(|e| e.to_string())?;
    println!("\nDone. The hub will restart.");
    Ok(())
}

/// Open the hub's CDC serial port, send `InfoRequest`, and return the
/// parsed response. Used only to report the hub's current firmware
/// version before asking the user to reboot into DFU mode.
fn query_spike_info(serial_path: &str) -> Result<rust_spike::atlantis::InfoResponse, String> {
    use bricklogo_hal::adapters::spike_adapter::{SpikeSerialTransport, SpikeTransport};
    use rust_spike::{atlantis, cobs};

    let mut transport = SpikeSerialTransport::open(serial_path)?;

    // Drain any residual REPL / program output so the first frame we read is
    // the InfoResponse.
    let mut scratch = [0u8; 1024];
    let drain_deadline = Instant::now() + Duration::from_millis(100);
    while Instant::now() < drain_deadline {
        if <SpikeSerialTransport as SpikeTransport>::read(&mut transport, &mut scratch)? == 0 {
            break;
        }
    }

    let req = atlantis::info_request();
    let framed = cobs::pack(&req);
    <SpikeSerialTransport as SpikeTransport>::write_all(&mut transport, &framed)?;
    <SpikeSerialTransport as SpikeTransport>::flush(&mut transport)?;

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut buf = [0u8; 1024];
    let mut frame_buf: Vec<u8> = Vec::new();
    loop {
        while let Some(pos) = frame_buf.iter().position(|&b| b == cobs::END_FRAME) {
            let frame: Vec<u8> = frame_buf.drain(..=pos).collect();
            let body = match cobs::unpack(&frame[..frame.len() - 1]) {
                Ok(b) => b,
                Err(_) => continue,
            };
            if let Ok(atlantis::Message::InfoResponse(info)) = atlantis::parse(&body) {
                return Ok(info);
            }
        }
        if Instant::now() >= deadline {
            return Err("no InfoResponse".into());
        }
        match <SpikeSerialTransport as SpikeTransport>::read(&mut transport, &mut buf)? {
            0 => std::thread::sleep(Duration::from_millis(10)),
            n => frame_buf.extend_from_slice(&buf[..n]),
        }
    }
}

// ── SPIKE detection ─────────────────────────────

pub fn detect_spike() -> Result<SpikeMode, String> {
    // Bootloader first — if someone has already button-pressed the hub, use DFU.
    if let Some(info) = rust_stm32dfu::dfu::find_stm32_bootloader().map_err(|e| e.to_string())? {
        return Ok(SpikeMode::Bootloader(info));
    }

    // Normal-mode hub over USB CDC (LEGO VID 0x0694).
    const SPIKE_VID: u16 = 0x0694;
    let ports = serialport::available_ports().map_err(|e| e.to_string())?;
    for port in ports {
        if let serialport::SerialPortType::UsbPort(ref info) = port.port_type {
            if info.vid == SPIKE_VID {
                return Ok(SpikeMode::Normal { serial_path: port.port_name.clone() });
            }
        }
    }

    Ok(SpikeMode::None)
}

// ── Helpers ─────────────────────────────────────

/// Resolve the `firmware/` directory. Walks upward from the binary location
/// looking for a sibling `firmware/` (release layout) and falls back to the
/// repo-root `firmware/` when running via `cargo run` from the workspace.
fn bundled_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor: Option<&Path> = exe.parent();
        while let Some(dir) = cursor {
            let candidate = dir.join("firmware");
            if candidate.is_dir() {
                return candidate;
            }
            cursor = dir.parent();
        }
    }
    PathBuf::from("firmware")
}

fn print_progress(done: usize, total: usize, _phase: &str) {
    // Overwrite the current line. The final newline is printed by the
    // caller after the phase completes. `_phase` is intentionally unused:
    // the single "Uploading …" line printed above the bar is enough
    // context, and device-specific phase words ("erasing"/"writing"/
    // "clear"/"signature") vary so much across adapters that surfacing
    // them makes the output noisier, not clearer.
    let pct = if total == 0 { 100 } else { (done * 100) / total };
    let bar_width = 24usize;
    let filled = if total == 0 { bar_width } else { (done * bar_width) / total };
    let bar: String = (0..bar_width)
        .map(|i| if i < filled { '=' } else if i == filled { '>' } else { ' ' })
        .collect();
    let _ = write!(std::io::stderr(), "\r[{}] {:>3}%", bar, pct);
    let _ = std::io::stderr().flush();
}

#[cfg(test)]
#[path = "tests/firmware.rs"]
mod tests;
