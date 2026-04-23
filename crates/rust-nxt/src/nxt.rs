//! High-level `Nxt` handle.
//!
//! Wraps a [`Transport`] and exposes typed methods for every LCP command
//! the BrickLogo adapter needs. Each reply-bearing method loops the
//! transport's `recv` until it sees a reply frame whose echoed opcode
//! matches (so stray replies from a previously aborted command are
//! skipped rather than misinterpreted).

use std::time::Duration;

use crate::protocol::{self, InputValues, OutputState};
use crate::transport::Transport;

/// Default per-command reply window. Matches nxt-python's default and is
/// comfortably longer than a BT round-trip (~30-60 ms) but short enough
/// that a dropped connection fails fast.
const REPLY_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct Nxt {
    transport: Box<dyn Transport>,
}

#[derive(Debug, Clone, Copy)]
pub struct OutputStateSpec {
    pub port: u8,
    pub power: i8,
    pub mode: u8,
    pub regulation: u8,
    pub turn_ratio: i8,
    pub run_state: u8,
    pub tacho_limit: u32,
    pub reply_required: bool,
}

impl Nxt {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Nxt { transport }
    }

    /// Send an LCP packet that expects no reply.
    pub fn send_no_reply(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.transport.send(bytes)
    }

    /// Send an LCP packet and wait for the matching reply.
    fn request(&mut self, bytes: &[u8], expected_op: u8) -> Result<Vec<u8>, String> {
        self.transport.send(bytes)?;
        let deadline = std::time::Instant::now() + REPLY_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "NXT timed out waiting for reply to 0x{:02X}",
                    expected_op
                ));
            }
            let reply = self.transport.recv(remaining)?;
            if reply.len() >= 2 && reply[0] == protocol::TYPE_REPLY && reply[1] == expected_op {
                return Ok(reply);
            }
            // Stale reply from an earlier command — drop and keep reading.
        }
    }

    pub fn get_firmware_version(&mut self) -> Result<(u8, u8, u8, u8), String> {
        let cmd = protocol::cmd_get_firmware_version();
        let reply = self.request(&cmd, protocol::SYS_GET_FIRMWARE_VERSION)?;
        protocol::parse_firmware_version(&reply)
    }

    pub fn get_battery_level(&mut self) -> Result<u16, String> {
        let cmd = protocol::cmd_get_battery_level();
        let reply = self.request(&cmd, protocol::OP_GET_BATTERY_LEVEL)?;
        protocol::parse_battery_level(&reply)
    }

    pub fn keep_alive(&mut self) -> Result<(), String> {
        let cmd = protocol::cmd_keep_alive();
        let reply = self.request(&cmd, protocol::OP_KEEP_ALIVE)?;
        protocol::check_reply(&reply, protocol::OP_KEEP_ALIVE).map(|_| ())
    }

    pub fn set_output_state(&mut self, s: &OutputStateSpec) -> Result<(), String> {
        let cmd = protocol::cmd_set_output_state(
            s.port,
            s.power,
            s.mode,
            s.regulation,
            s.turn_ratio,
            s.run_state,
            s.tacho_limit,
            s.reply_required,
        );
        if s.reply_required {
            let reply = self.request(&cmd, protocol::OP_SET_OUTPUT_STATE)?;
            protocol::check_reply(&reply, protocol::OP_SET_OUTPUT_STATE).map(|_| ())
        } else {
            self.transport.send(&cmd)
        }
    }

    pub fn get_output_state(&mut self, port: u8) -> Result<OutputState, String> {
        let cmd = protocol::cmd_get_output_state(port);
        let reply = self.request(&cmd, protocol::OP_GET_OUTPUT_STATE)?;
        protocol::parse_output_state(&reply)
    }

    pub fn reset_motor_position(&mut self, port: u8, relative: bool) -> Result<(), String> {
        let cmd = protocol::cmd_reset_motor_position(port, relative);
        let reply = self.request(&cmd, protocol::OP_RESET_MOTOR_POSITION)?;
        protocol::check_reply(&reply, protocol::OP_RESET_MOTOR_POSITION).map(|_| ())
    }

    pub fn set_input_mode(&mut self, port: u8, ty: u8, mode: u8) -> Result<(), String> {
        let cmd = protocol::cmd_set_input_mode(port, ty, mode, true);
        let reply = self.request(&cmd, protocol::OP_SET_INPUT_MODE)?;
        protocol::check_reply(&reply, protocol::OP_SET_INPUT_MODE).map(|_| ())
    }

    pub fn get_input_values(&mut self, port: u8) -> Result<InputValues, String> {
        let cmd = protocol::cmd_get_input_values(port);
        let reply = self.request(&cmd, protocol::OP_GET_INPUT_VALUES)?;
        protocol::parse_input_values(&reply)
    }

    pub fn play_tone(&mut self, freq_hz: u16, duration_ms: u16) -> Result<(), String> {
        let cmd = protocol::cmd_play_tone(freq_hz, duration_ms);
        let reply = self.request(&cmd, protocol::OP_PLAY_TONE)?;
        protocol::check_reply(&reply, protocol::OP_PLAY_TONE).map(|_| ())
    }

    pub fn stop_program(&mut self) -> Result<(), String> {
        let cmd = protocol::cmd_stop_program();
        let reply = self.request(&cmd, protocol::OP_STOP_PROGRAM)?;
        // 0xEC "no active program" is the common case; swallow it.
        match protocol::check_reply(&reply, protocol::OP_STOP_PROGRAM) {
            Ok(_) => Ok(()),
            Err(e) if e.contains("0xEC") => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
#[path = "tests/nxt.rs"]
mod tests;
