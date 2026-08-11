//! High-level device handle: opens/probes the link and exposes the small
//! command surface the HAL adapter needs (set outputs, stream a PWM batch,
//! drain pending input). Built on the `Transport` trait, so it works
//! identically over a real serial port or a mock in tests.

use std::time::{Duration, Instant};

use crate::constants::*;
use crate::protocol::{self, Line};
use crate::serial::SerialTransport;
use crate::transport::Transport;

pub struct Device {
    transport: Box<dyn Transport>,
    rx_buffer: Vec<u8>,
    /// The most recent input bitmask seen. `None` until the first `Ixx`
    /// report arrives — the device is push-driven, so there is no way to
    /// poll for a starting value.
    last_input: Option<u8>,
    firmware_version: String,
}

impl Device {
    /// Open a serial port and run the discovery handshake: probe (`R` ->
    /// `CF`) then query the firmware version (`V`). On any failure the
    /// transport is dropped along with this call, closing the port — so
    /// this doubles as a discovery probe: try a candidate path, keep it on
    /// `Ok`, move on to the next on `Err`.
    pub fn open(path: &str) -> Result<Self, String> {
        let transport: Box<dyn Transport> = Box::new(SerialTransport::open(path)?);
        Self::connect(transport)
    }

    /// Run the same handshake against an already-constructed transport
    /// (real or mock). Used by `open` and directly by tests.
    pub fn connect(transport: Box<dyn Transport>) -> Result<Self, String> {
        let mut device = Device {
            transport,
            rx_buffer: Vec::new(),
            last_input: None,
            firmware_version: String::new(),
        };
        device.probe()?;
        device.firmware_version = device.query_version()?;
        Ok(device)
    }

    pub fn firmware_version(&self) -> &str {
        &self.firmware_version
    }

    fn probe(&mut self) -> Result<(), String> {
        self.transport.write(CMD_PROBE)?;
        match self.wait_for_reply(PROBE_TIMEOUT_MS)? {
            s if s == REPLY_PROBE => Ok(()),
            other => Err(format!("Unexpected probe reply: {:?}", other)),
        }
    }

    fn query_version(&mut self) -> Result<String, String> {
        self.transport.write(CMD_VERSION)?;
        match self.wait_for_reply(PROBE_TIMEOUT_MS)? {
            s if s.starts_with('V') => Ok(s),
            other => Err(format!("Unexpected version reply: {:?}", other)),
        }
    }

    /// Block (polling `read_available`) until one command-reply line
    /// arrives or `timeout_ms` elapses. Input reports (`Ixx`) seen while
    /// waiting are absorbed into `last_input` rather than discarded, so a
    /// probe/version query never eats a real sensor edge.
    fn wait_for_reply(&mut self, timeout_ms: u64) -> Result<String, String> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            for line in self.poll_lines()? {
                match line {
                    Line::Input(mask) => self.last_input = Some(mask),
                    Line::Reply(s) => return Ok(s),
                }
            }
            if Instant::now() >= deadline {
                return Err("Timed out waiting for reply".to_string());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn poll_lines(&mut self) -> Result<Vec<Line>, String> {
        let bytes = self.transport.read_available()?;
        if !bytes.is_empty() {
            self.rx_buffer.extend_from_slice(&bytes);
        }
        Ok(protocol::drain_lines(&mut self.rx_buffer))
    }

    /// Set all six outputs from one bitmask in a single `D` frame.
    pub fn set_outputs(&mut self, mask: u8) -> Result<(), String> {
        self.transport
            .write(protocol::encode_set_outputs(mask).as_bytes())
    }

    /// Write a batch of `D` frames — one per desired 1ms replay slot — in a
    /// single serial write, for host-side PWM streaming.
    pub fn write_batch(&mut self, masks: &[u8]) -> Result<(), String> {
        self.transport.write(protocol::encode_batch(masks).as_bytes())
    }

    /// Drain whatever bytes are currently available (non-blocking) and
    /// return every input bitmask seen, in arrival order, updating
    /// `last_input` to the final one. The device only sends `Ixx` on a
    /// state *change*, so a press and a release can both land in a single
    /// drain (e.g. a fast tap inside one ~16ms scheduler tick) — callers
    /// that care about edges (like `counter`) must walk the whole list, not
    /// just the last entry, or they'll miss transitions that cancel out
    /// within one call. Non-input lines (stray command replies) are
    /// dropped.
    pub fn read_pending_input(&mut self) -> Result<Vec<u8>, String> {
        let mut masks = Vec::new();
        for line in self.poll_lines()? {
            if let Line::Input(mask) = line {
                self.last_input = Some(mask);
                masks.push(mask);
            }
        }
        Ok(masks)
    }

    /// The most recently known input bitmask, from either a prior
    /// `read_pending_input` call or one absorbed during the handshake.
    /// `None` until the first `Ixx` report has ever been seen.
    pub fn last_input(&self) -> Option<u8> {
        self.last_input
    }
}

#[cfg(test)]
#[path = "tests/device.rs"]
mod tests;
