use std::time::{Duration, Instant};
use crate::constants::*;
use crate::protocol::{build_frame, try_parse_frame};
use crate::transport::BrickInterfaceTransport;

const REPLY_TIMEOUT_MS: u64 = 1000;
// A PF burst takes up to ~0.56 s (channel 4, spec-spaced repeats, fw >= 0.4)
// from accept to IR_DONE. Waiting starts right after the previous accept, so
// allow one full burst plus USB margin.
const IR_DONE_TIMEOUT_MS: u64 = 1500;
// After IR_ABORT_ALL, fw >= 0.4 emits the aborted burst's IR_DONE within one
// firmware loop pass; 0.3 firmware swallows it. Short best-effort drain.
const ABORT_DRAIN_TIMEOUT_MS: u64 = 250;

struct Connection {
    transport: Box<dyn BrickInterfaceTransport>,
    seq: u8,
    read_buf: Vec<u8>,
    // True while an accepted IR burst is still transmitting (its IR_DONE has
    // not been seen yet). Set on IR_ACCEPTED, cleared whenever an IR_DONE
    // event is consumed — by wait_ir_done or in passing by recv_reply.
    ir_pending: bool,
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

    /// Read until one complete frame is available or `deadline` passes.
    /// Returns `(seq, cmd, payload)`.
    fn recv_frame(&mut self, deadline: Instant) -> Result<(u8, u8, Vec<u8>), String> {
        let mut tmp = [0u8; 64];
        loop {
            if let Some((seq, cmd, payload, consumed)) = try_parse_frame(&self.read_buf) {
                self.read_buf.drain(..consumed);
                return Ok((seq, cmd, payload));
            }
            if Instant::now() >= deadline {
                return Err("Reply timed out".to_string());
            }
            match self.transport.read(&mut tmp) {
                Ok(n) if n > 0 => self.read_buf.extend_from_slice(&tmp[..n]),
                Ok(_) => {}
                Err(e) => return Err(format!("Read failed: {}", e)),
            }
        }
    }

    /// Wait for the reply to the command just sent. Frames with SEQ 0x00 are
    /// asynchronous events, not replies — an IR_DONE clears `ir_pending`, and
    /// every event is skipped so it can never be mistaken for a reply.
    fn recv_reply(&mut self) -> Result<(u8, Vec<u8>), String> {
        let deadline = Instant::now() + Duration::from_millis(REPLY_TIMEOUT_MS);
        loop {
            let (seq, cmd, payload) = self.recv_frame(deadline)?;
            if seq == 0x00 {
                if cmd == REPLY_IR_DONE {
                    self.ir_pending = false;
                }
                continue;
            }
            return Ok((cmd, payload));
        }
    }

    /// Wait for the in-flight IR burst (if any) to report IR_DONE.
    /// Returns `Ok(true)` once the transmitter is idle (immediately so when
    /// no burst is outstanding), `Ok(false)` if the burst is still in the
    /// air when `timeout_ms` expires, or `Err` on a transport failure.
    fn wait_ir_done(&mut self, timeout_ms: u64) -> Result<bool, String> {
        if !self.ir_pending {
            return Ok(true);
        }
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            match self.recv_frame(deadline) {
                Ok((seq, cmd, _)) => {
                    if seq == 0x00 && cmd == REPLY_IR_DONE {
                        self.ir_pending = false;
                        return Ok(true);
                    }
                    // No command is outstanding here; anything else is a
                    // stale frame — drop it.
                }
                Err(e) if e == "Reply timed out" => return Ok(false),
                Err(e) => return Err(e),
            }
        }
    }

    fn send_recv(&mut self, cmd: u8, payload: &[u8]) -> Result<(u8, Vec<u8>), String> {
        let seq = self.next_seq();
        let frame = build_frame(seq, cmd, payload);
        self.transport.write_all(&frame)?;
        self.transport.flush()?;
        self.recv_reply()
    }
}

/// High-level driver for a BrickInterface or IRInterface device.
pub struct BrickInterface {
    conn: Connection,
}

impl BrickInterface {
    /// Open a connection to a BrickInterface on the given serial port.
    pub fn open(serial_path: &str) -> Result<Self, String> {
        // Short read timeout: recv loops check their deadlines between
        // blocking reads, so this bounds how far a sliced wait (pf_wait_idle
        // under a shared lock) can overshoot its slice. 10 ms keeps
        // worst-case lock hold below a 60 Hz scheduler tick.
        let mut serial = serialport::new(serial_path, 115200)
            .timeout(Duration::from_millis(10))
            .open()
            .map_err(|e| format!("Could not open {}: {}", serial_path, e))?;
        // The CH55x USB-CDC firmware only transmits while the host has DTR
        // asserted (it gates all TX on the control-line state). macOS asserts
        // DTR when opening a cu.* port; Windows does not, so without this the
        // device receives commands but never sends a reply. Assert it
        // explicitly. Errors are ignored — a port that can't set DTR will
        // surface a clear timeout on the first query instead.
        let _ = serial.write_data_terminal_ready(true);
        Ok(Self::from_transport(Box::new(serial)))
    }

    /// Open using an injected transport (primarily for testing).
    pub fn from_transport(transport: Box<dyn BrickInterfaceTransport>) -> Self {
        BrickInterface {
            conn: Connection {
                transport,
                seq: 1,
                read_buf: Vec::new(),
                ir_pending: false,
            },
        }
    }

    // ── Interface A ───────────────────────────────────────────────────────────

    /// Set selected output duties. `mask` bit i set = update output i.
    /// `duties` contains exactly one byte per set bit in `mask`, in ascending
    /// bit order. E.g. mask=0x05 with duties=[a, b] sets output 0 to `a` and
    /// output 2 to `b`; outputs 1, 3, 4, 5 are left untouched by the firmware.
    pub fn set_outputs_masked(&mut self, mask: u8, duties: &[u8]) -> Result<(), String> {
        let mask = mask & 0x3F;
        let mut payload = Vec::with_capacity(1 + duties.len());
        payload.push(mask);
        payload.extend_from_slice(duties);
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
    ///
    /// Returns as soon as the firmware accepts the burst (REPLY_IR_ACCEPTED);
    /// the IR transmission itself (~0.4-0.6 s of spec-spaced repeats) carries
    /// on in the background and its IR_DONE event is consumed by a later
    /// call. If a previous burst is still in flight, blocks first until that
    /// one completes — the firmware has a single transmit slot.
    pub fn pf_send_single_pwm(&mut self, channel: u8, output_b: bool, step: u8) -> Result<(), String> {
        if channel > 3 {
            return Err(format!("PF channel {} out of range (0..=3)", channel));
        }
        if step > 15 {
            return Err(format!("PF step {} out of range (0..=15)", step));
        }
        let data = if output_b { 0x10 | step } else { step };
        if !self.conn.wait_ir_done(IR_DONE_TIMEOUT_MS)? {
            return Err("Timed out waiting for IR transmission to complete".to_string());
        }
        let (r1, _) = self.conn.send_recv(CMD_PF_SEND, &[channel, PF_MODE_SINGLE_PWM, data, 0x00])?;
        if r1 != REPLY_IR_ACCEPTED {
            return Err(format!("Expected IR_ACCEPTED (0x{:02X}), got 0x{:02X}", REPLY_IR_ACCEPTED, r1));
        }
        self.conn.ir_pending = true;
        Ok(())
    }

    /// Send a Combo PWM command to both outputs of a PF receiver simultaneously.
    /// Uses the escape bit (nibble 0 bit 2 = 1) to select Combo PWM mode in the PF protocol.
    /// `channel`: 0-3. `step_a`: Output A (red) step. `step_b`: Output B (blue) step.
    /// Step encoding: 0=float, 1-7=forward, 8=brake, 9-15=reverse (7/7 down to 1/7).
    ///
    /// Returns as soon as the firmware accepts the burst (REPLY_IR_ACCEPTED);
    /// see [`Self::pf_send_single_pwm`] for the in-flight burst semantics.
    /// Note: Combo PWM receivers stop on a lost-IR timeout — re-send
    /// periodically to hold a state.
    pub fn pf_send_combo_pwm(&mut self, channel: u8, step_a: u8, step_b: u8) -> Result<(), String> {
        if channel > 3 {
            return Err(format!("PF channel {} out of range (0..=3)", channel));
        }
        if step_a > 15 || step_b > 15 {
            return Err(format!("PF step out of range (0..=15): step_a={}, step_b={}", step_a, step_b));
        }
        let data = (step_b << 4) | step_a;
        if !self.conn.wait_ir_done(IR_DONE_TIMEOUT_MS)? {
            return Err("Timed out waiting for IR transmission to complete".to_string());
        }
        let (r1, _) = self.conn.send_recv(CMD_PF_SEND, &[channel, PF_MODE_COMBO_PWM, data, 0x00])?;
        if r1 != REPLY_IR_ACCEPTED {
            return Err(format!("Expected IR_ACCEPTED (0x{:02X}), got 0x{:02X}", REPLY_IR_ACCEPTED, r1));
        }
        self.conn.ir_pending = true;
        Ok(())
    }

    /// Wait up to `timeout_ms` for any in-flight IR burst to complete.
    ///
    /// Returns `Ok(true)` when the transmitter is idle (immediately so if no
    /// burst is outstanding), `Ok(false)` if a burst is still in the air when
    /// the timeout expires, `Err` only on transport failure.
    ///
    /// Intended for callers that share one device across threads (e.g. the
    /// BrickLogo HAL): wait in short slices and release the shared lock
    /// between slices so other traffic — Interface A in particular — can
    /// interleave. Once this returns `true`, the next `pf_send_*` proceeds
    /// without any further wait.
    pub fn pf_wait_idle(&mut self, timeout_ms: u64) -> Result<bool, String> {
        self.conn.wait_ir_done(timeout_ms)
    }

    /// Abort any in-flight or queued IR transmission.
    ///
    /// Firmware >= 0.4 still resolves the aborted burst's token with an
    /// IR_DONE event moments after the OK; drain it (best-effort — 0.3
    /// firmware swallows it) so the connection ends up idle either way.
    pub fn ir_abort_all(&mut self) -> Result<(), String> {
        let (reply, _) = self.conn.send_recv(CMD_IR_ABORT_ALL, &[])?;
        expect_ok(reply)?;
        if self.conn.ir_pending {
            let _ = self.conn.wait_ir_done(ABORT_DRAIN_TIMEOUT_MS);
            self.conn.ir_pending = false;
        }
        Ok(())
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
