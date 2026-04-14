//! High-level EV3 handle. Wraps a `Transport`, owns a monotonic message
//! counter, and exposes methods that build+send Direct Commands and parse
//! replies.

use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use crate::protocol;
use crate::transport::Transport;

const DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);

pub struct Ev3 {
    transport: Box<dyn Transport>,
    counter: AtomicU16,
}

impl Ev3 {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Ev3 {
            transport,
            counter: AtomicU16::new(1),
        }
    }

    fn next_counter(&self) -> u16 {
        // Zero is reserved — wrap from u16::MAX back to 1.
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        if n == 0 { self.counter.fetch_add(1, Ordering::Relaxed) } else { n }
    }

    /// Send a fire-and-forget Direct Command.
    pub fn send_no_reply(&mut self, frame: protocol::Frame) -> Result<(), String> {
        self.transport.send(&frame.encode())
    }

    /// Send a Direct Command and block until its reply arrives (matched by
    /// counter). Returns the reply payload (bytes after the 5-byte
    /// [length][counter][type] header).
    pub fn request(&mut self, frame: protocol::Frame) -> Result<Vec<u8>, String> {
        let expected = frame.counter;
        self.transport.send(&frame.encode())?;
        loop {
            let raw = self.transport.recv(DEFAULT_TIMEOUT)?;
            if raw.len() < 5 {
                return Err(format!("EV3 reply too short: {} bytes", raw.len()));
            }
            // raw: [length:u16][counter:u16][reply_type:u8][payload...]
            let counter = u16::from_le_bytes([raw[2], raw[3]]);
            if counter != expected {
                // Stale reply from a previous request — skip and keep
                // reading until we find ours or time out.
                continue;
            }
            let reply_type = raw[4];
            // Reply type: 0x02 direct-ok, 0x04 direct-error, 0x03 system-ok,
            // 0x05 system-error. Error types still carry the payload.
            if reply_type == 0x04 || reply_type == 0x05 {
                return Err(format!("EV3 command failed (reply type {:#04x})", reply_type));
            }
            return Ok(raw[5..].to_vec());
        }
    }

    // ── Motor ops ────────────────────────────────

    pub fn set_power(&mut self, ports_mask: u8, power: i8) -> Result<(), String> {
        let c = self.next_counter();
        self.send_no_reply(protocol::cmd_output_power(c, ports_mask, power))
    }

    pub fn start(&mut self, ports_mask: u8) -> Result<(), String> {
        let c = self.next_counter();
        self.send_no_reply(protocol::cmd_output_start(c, ports_mask))
    }

    pub fn stop(&mut self, ports_mask: u8, brake: bool) -> Result<(), String> {
        let c = self.next_counter();
        self.send_no_reply(protocol::cmd_output_stop(c, ports_mask, brake))
    }

    pub fn step_power(
        &mut self,
        ports_mask: u8,
        power: i8,
        degrees: i32,
        brake: bool,
    ) -> Result<(), String> {
        let c = self.next_counter();
        // No ramp: all motion in the constant-speed phase.
        self.send_no_reply(protocol::cmd_output_step_power(
            c,
            ports_mask,
            power,
            0,
            degrees,
            0,
            brake,
        ))
    }

    pub fn time_power(
        &mut self,
        ports_mask: u8,
        power: i8,
        ms: i32,
        brake: bool,
    ) -> Result<(), String> {
        let c = self.next_counter();
        self.send_no_reply(protocol::cmd_output_time_power(
            c,
            ports_mask,
            power,
            0,
            ms,
            0,
            brake,
        ))
    }

    /// Returns `true` if any of `ports_mask`'s ports are still executing
    /// a STEP_* or TIME_* command.
    pub fn test_busy(&mut self, ports_mask: u8) -> Result<bool, String> {
        let c = self.next_counter();
        let reply = self.request(protocol::cmd_output_test_busy(c, ports_mask))?;
        Ok(reply.first().copied().unwrap_or(0) != 0)
    }

    pub fn clr_count(&mut self, ports_mask: u8) -> Result<(), String> {
        let c = self.next_counter();
        self.send_no_reply(protocol::cmd_output_clr_count(c, ports_mask))
    }

    /// Read the accumulated position counter (degrees since last
    /// `clr_count`). `port_index` is 0..=3, **not** a bitmask.
    pub fn get_count(&mut self, port_index: u8) -> Result<i32, String> {
        let c = self.next_counter();
        let reply = self.request(protocol::cmd_output_get_count(c, port_index))?;
        if reply.len() < 4 {
            return Err("EV3 get_count reply too short".to_string());
        }
        Ok(i32::from_le_bytes([reply[0], reply[1], reply[2], reply[3]]))
    }

    // ── Sensor ops ───────────────────────────────

    /// Read a sensor value in percent (0..=100). `port` is 0..=3.
    pub fn read_sensor_pct(
        &mut self,
        port: u8,
        sensor_type: u8,
        mode: u8,
    ) -> Result<u8, String> {
        let c = self.next_counter();
        let reply = self.request(protocol::cmd_input_read_pct(c, port, sensor_type, mode))?;
        Ok(reply.first().copied().unwrap_or(0))
    }

    /// Read a sensor value in SI units (f32). `port` is 0..=3.
    pub fn read_sensor_si(
        &mut self,
        port: u8,
        sensor_type: u8,
        mode: u8,
    ) -> Result<f32, String> {
        let c = self.next_counter();
        let reply = self.request(protocol::cmd_input_read_si(c, port, sensor_type, mode))?;
        if reply.len() < 4 {
            return Err("EV3 read_sensor_si reply too short".to_string());
        }
        Ok(f32::from_le_bytes([reply[0], reply[1], reply[2], reply[3]]))
    }

    /// Query the (type, mode) currently configured on a sensor port. A
    /// type of 0xFF means "no sensor connected."
    pub fn get_sensor_typemode(&mut self, port: u8) -> Result<(u8, u8), String> {
        let c = self.next_counter();
        let reply = self.request(protocol::cmd_input_get_typemode(c, port))?;
        if reply.len() < 2 {
            return Err("EV3 get_sensor_typemode reply too short".to_string());
        }
        Ok((reply[0], reply[1]))
    }
}
