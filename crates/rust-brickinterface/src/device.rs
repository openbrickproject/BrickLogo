use std::time::{Duration, Instant};
use crate::constants::*;
use crate::protocol::{build_frame, try_parse_frame};
use crate::transport::BrickInterfaceTransport;

const REPLY_TIMEOUT_MS: u64 = 1000;

struct Connection {
    transport: Box<dyn BrickInterfaceTransport>,
    seq: u8,
    read_buf: Vec<u8>,
}

impl Connection {
    fn next_seq(&mut self) -> u8 {
        let s = self.seq;
        self.seq = self.seq.wrapping_add(1);
        // Skip 0x00, reserved by the firmware for async events.
        if self.seq == 0 {
            self.seq = 1;
        }
        s
    }

    fn recv_one(&mut self) -> Result<(u8, Vec<u8>), String> {
        let deadline = Instant::now() + Duration::from_millis(REPLY_TIMEOUT_MS);
        let mut tmp = [0u8; 64];
        loop {
            if Instant::now() >= deadline {
                return Err("Reply timed out".to_string());
            }
            match self.transport.read(&mut tmp) {
                Ok(n) if n > 0 => self.read_buf.extend_from_slice(&tmp[..n]),
                Ok(_) => {}
                Err(e) => return Err(format!("Read failed: {}", e)),
            }
            if let Some((reply_cmd, reply_payload, consumed)) = try_parse_frame(&self.read_buf) {
                self.read_buf.drain(..consumed);
                return Ok((reply_cmd, reply_payload));
            }
        }
    }

    fn send_recv(&mut self, cmd: u8, payload: &[u8]) -> Result<(u8, Vec<u8>), String> {
        let seq = self.next_seq();
        let frame = build_frame(seq, cmd, payload);
        self.transport.write_all(&frame)?;
        self.transport.flush()?;
        self.recv_one()
    }
}

/// High-level driver for a BrickInterface or IRInterface device.
pub struct BrickInterface {
    conn: Connection,
}

impl BrickInterface {
    /// Open a connection to a BrickInterface on the given serial port.
    pub fn open(serial_path: &str) -> Result<Self, String> {
        let serial = serialport::new(serial_path, 115200)
            .timeout(Duration::from_millis(50))
            .open()
            .map_err(|e| format!("Could not open {}: {}", serial_path, e))?;
        Ok(Self::from_transport(Box::new(serial)))
    }

    /// Open using an injected transport (primarily for testing).
    pub fn from_transport(transport: Box<dyn BrickInterfaceTransport>) -> Self {
        BrickInterface {
            conn: Connection {
                transport,
                seq: 1,
                read_buf: Vec::new(),
            },
        }
    }

    // ── Interface A ───────────────────────────────────────────────────────────

    /// Set all six output duties at once (6-byte form — every output is updated).
    pub fn set_outputs(&mut self, duties: &[u8; 6]) -> Result<(), String> {
        let (reply, _) = self.conn.send_recv(CMD_IFACE_SET_OUTPUTS, duties.as_slice())?;
        expect_ok(reply)
    }

    /// Set one output duty without disturbing the others (7-byte masked form).
    /// `current_duties` is the caller's cached state for the other five outputs.
    pub fn set_output(&mut self, index: usize, duty: u8, current_duties: &[u8; 6]) -> Result<(), String> {
        if index > 5 {
            return Err(format!("Output index {} out of range (0..=5)", index));
        }
        let mut payload = [0u8; 7];
        payload[..6].copy_from_slice(current_duties.as_slice());
        payload[index] = duty;
        payload[6] = 1 << index;
        let (reply, _) = self.conn.send_recv(CMD_IFACE_SET_OUTPUTS, &payload)?;
        expect_ok(reply)
    }

    /// Read the raw input state byte.
    /// Bit 0 = input 6, bit 1 = input 7.
    /// Bit value 1 = open/pulled-up, 0 = closed/grounded.
    pub fn get_inputs(&mut self) -> Result<u8, String> {
        let (reply, payload) = self.conn.send_recv(CMD_IFACE_GET_INPUTS, &[])?;
        if reply != REPLY_IFACE_INPUTS {
            return Err(format!("Unexpected reply 0x{:02X}", reply));
        }
        payload.first().copied().ok_or_else(|| "Empty inputs reply".to_string())
    }

    /// Read the edge counters for both inputs.
    /// Returns `(count6, count7)`.
    pub fn get_counts(&mut self) -> Result<(u32, u32), String> {
        let (reply, payload) = self.conn.send_recv(CMD_IFACE_GET_COUNTS, &[])?;
        if reply != REPLY_IFACE_COUNTS {
            return Err(format!("Unexpected reply 0x{:02X}", reply));
        }
        if payload.len() < 8 {
            return Err("Short counts reply".to_string());
        }
        let c6 = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let c7 = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
        Ok((c6, c7))
    }

    /// Reset the edge counter for the given input (6 or 7).
    pub fn reset_count(&mut self, input: u8) -> Result<(), String> {
        if input != 6 && input != 7 {
            return Err(format!("Input {} out of range (6 or 7)", input));
        }
        let (reply, _) = self.conn.send_recv(CMD_IFACE_RESET_COUNT, &[input])?;
        expect_ok(reply)
    }

    // ── Power Functions IR ────────────────────────────────────────────────────

    /// Send a Single PWM command to one output of a PF receiver.
    /// `channel`: 0-3 (PF channels 1-4). `output_b`: false=A/red, true=B/blue.
    /// `step`: 0=float, 1-7=forward, 8=brake, 9-15=reverse (7/7 down to 1/7).
    /// Blocks until the firmware confirms the IR transmission is complete (REPLY_IR_DONE).
    pub fn pf_send_single_pwm(&mut self, channel: u8, output_b: bool, step: u8) -> Result<(), String> {
        if channel > 3 {
            return Err(format!("PF channel {} out of range (0..=3)", channel));
        }
        if step > 15 {
            return Err(format!("PF step {} out of range (0..=15)", step));
        }
        let data = if output_b { 0x10 | step } else { step };
        let (r1, _) = self.conn.send_recv(CMD_PF_SEND, &[channel, PF_MODE_SINGLE_PWM, data, 0x00])?;
        if r1 != REPLY_IR_ACCEPTED {
            return Err(format!("Expected IR_ACCEPTED (0x{:02X}), got 0x{:02X}", REPLY_IR_ACCEPTED, r1));
        }
        let (r2, _) = self.conn.recv_one()?;
        if r2 != REPLY_IR_DONE {
            return Err(format!("Expected IR_DONE (0x{:02X}), got 0x{:02X}", REPLY_IR_DONE, r2));
        }
        Ok(())
    }

    /// Send a Combo PWM command to both outputs of a PF receiver simultaneously.
    /// Uses the escape bit (nibble 0 bit 2 = 1) to select Combo PWM mode in the PF protocol.
    /// `channel`: 0-3. `step_a`: Output A (red) step. `step_b`: Output B (blue) step.
    /// Step encoding: 0=float, 1-7=forward, 8=brake, 9-15=reverse (7/7 down to 1/7).
    /// Blocks until the firmware confirms the IR transmission is complete (REPLY_IR_DONE).
    pub fn pf_send_combo_pwm(&mut self, channel: u8, step_a: u8, step_b: u8) -> Result<(), String> {
        if channel > 3 {
            return Err(format!("PF channel {} out of range (0..=3)", channel));
        }
        if step_a > 15 || step_b > 15 {
            return Err(format!("PF step out of range (0..=15): step_a={}, step_b={}", step_a, step_b));
        }
        let data = (step_b << 4) | step_a;
        let (r1, _) = self.conn.send_recv(CMD_PF_SEND, &[channel, PF_MODE_COMBO_PWM, data, 0x00])?;
        if r1 != REPLY_IR_ACCEPTED {
            return Err(format!("Expected IR_ACCEPTED (0x{:02X}), got 0x{:02X}", REPLY_IR_ACCEPTED, r1));
        }
        let (r2, _) = self.conn.recv_one()?;
        if r2 != REPLY_IR_DONE {
            return Err(format!("Expected IR_DONE (0x{:02X}), got 0x{:02X}", REPLY_IR_DONE, r2));
        }
        Ok(())
    }

    /// Abort all pending IR transmissions.
    pub fn ir_abort_all(&mut self) -> Result<(), String> {
        let (reply, _) = self.conn.send_recv(CMD_IR_ABORT_ALL, &[])?;
        expect_ok(reply)
    }

    // ── Core ──────────────────────────────────────────────────────────────────

    /// Send a PING and wait for PONG.
    pub fn ping(&mut self) -> Result<(), String> {
        let (reply, _) = self.conn.send_recv(CMD_PING, &[])?;
        if reply == REPLY_PONG { Ok(()) } else { Err(format!("Unexpected reply 0x{:02X}", reply)) }
    }

    /// Query the firmware and protocol version.
    /// Returns `(proto_major, proto_minor, fw_major, fw_minor)`.
    pub fn get_version(&mut self) -> Result<(u8, u8, u8, u8), String> {
        let (reply, payload) = self.conn.send_recv(CMD_GET_VERSION, &[])?;
        if reply != REPLY_VERSION {
            return Err(format!("Unexpected reply 0x{:02X}", reply));
        }
        if payload.len() < 4 {
            return Err("Short version reply".to_string());
        }
        Ok((payload[0], payload[1], payload[2], payload[3]))
    }

    /// Query the device capability bitmap.
    pub fn get_capabilities(&mut self) -> Result<u16, String> {
        let (reply, payload) = self.conn.send_recv(CMD_GET_CAPABILITIES, &[])?;
        if reply != REPLY_CAPABILITIES {
            return Err(format!("Unexpected reply 0x{:02X}", reply));
        }
        if payload.len() < 2 {
            return Err("Short capabilities reply".to_string());
        }
        Ok(u16::from_le_bytes([payload[0], payload[1]]))
    }
}

fn expect_ok(reply: u8) -> Result<(), String> {
    if reply == REPLY_OK {
        Ok(())
    } else {
        Err(format!("Unexpected reply 0x{:02X}", reply))
    }
}

#[cfg(test)]
#[path = "tests/device.rs"]
mod tests;
