//! LEGO SPIKE Prime / Robot Inventor adapter.
//!
//! Talks to the hub's MicroPython REPL over USB serial. Uses raw REPL mode
//! (Ctrl+A) for code execution. Motor commands are Python one-liners;
//! parallel operations use `runloop.gather()`. Sensor reads use `print()`
//! and parse stdout.

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bricklogo_lang::value::LogoValue;

use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use rust_spike::constants::*;
use rust_spike::protocol::*;

const OUTPUT_PORTS: &[&str] = &["a", "b", "c", "d", "e", "f"];
const MAX_POWER: u8 = 100;

// ── Transport trait ─────────────────────────────

pub trait SpikeTransport: Send {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String>;
    fn write_all(&mut self, data: &[u8]) -> Result<(), String>;
    fn flush(&mut self) -> Result<(), String>;
}

impl SpikeTransport for Box<dyn serialport::SerialPort> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        match Read::read(self.as_mut(), buf) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(e.to_string()),
        }
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        Write::write_all(self.as_mut(), data).map_err(|e| e.to_string())
    }
    fn flush(&mut self) -> Result<(), String> {
        Write::flush(self.as_mut()).map_err(|e| e.to_string())
    }
}

// ── Helpers ─────────────────────────────────────

/// Convert BrickLogo power (0-100) to SPIKE velocity (deg/s).
/// SPIKE motors max around 1000 deg/s; map power linearly.
fn to_velocity(direction: PortDirection, power: u8) -> i32 {
    let vel = power.min(MAX_POWER) as i32 * 10;
    match direction {
        PortDirection::Even => vel,
        PortDirection::Odd => -vel,
    }
}

// ── Slot commands ───────────────────────────────

type ReplyTx = mpsc::Sender<Result<String, String>>;

enum SpikeCommand {
    /// Send raw REPL code and get the response.
    Execute { code: Vec<u8>, reply_tx: ReplyTx },
}

// ── Slot ────────────────────────────────────────

struct SpikeSlot {
    port: Box<dyn SpikeTransport>,
    rx: mpsc::Receiver<SpikeCommand>,
    response_buf: Vec<u8>,
    alive: bool,
    /// The reply channel for the currently-executing command, if any.
    pending: Option<ReplyTx>,
}

impl SpikeSlot {
    /// Count how many \x04 bytes are in the response buffer.
    fn eot_count(&self) -> usize {
        self.response_buf.iter().filter(|&&b| b == CTRL_D).count()
    }
}

impl DeviceSlot for SpikeSlot {
    fn tick(&mut self) {
        // ── Read serial data ──────────────────
        let mut buf = [0u8; 1024];
        match self.port.read(&mut buf) {
            Ok(n) if n > 0 => {
                self.response_buf.extend_from_slice(&buf[..n]);
            }
            Err(_) => {
                self.alive = false;
                return;
            }
            _ => {}
        }

        // ── Check for command completion ──────
        // Raw REPL response ends with two \x04 bytes: OK<stdout>\x04<stderr>\x04
        if self.pending.is_some() && self.eot_count() >= 2 {
            let reply_tx = self.pending.take().unwrap();
            let result = parse_raw_repl_response(&self.response_buf);
            let _ = reply_tx.send(result);
            self.response_buf.clear();
        }

        // ── Drain command queue ───────────────
        // Only send next command if no command is pending.
        if self.pending.is_none() {
            if let Ok(cmd) = self.rx.try_recv() {
                match cmd {
                    SpikeCommand::Execute { code, reply_tx } => {
                        self.response_buf.clear();
                        match self.port.write_all(&code).and_then(|_| self.port.flush()) {
                            Ok(()) => {
                                self.pending = Some(reply_tx);
                            }
                            Err(e) => {
                                let _ = reply_tx.send(Err(e));
                            }
                        }
                    }
                }
            }
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── Adapter ─────────────────────────────────────

pub struct SpikeAdapter {
    tx: Option<mpsc::Sender<SpikeCommand>>,
    slot_id: Option<usize>,
    display_name: String,
    identifier: Option<String>,
}

impl SpikeAdapter {
    pub fn new(identifier: Option<&str>) -> Self {
        SpikeAdapter {
            tx: None,
            slot_id: None,
            display_name: "LEGO SPIKE Prime".to_string(),
            identifier: identifier.map(|s| s.to_string()),
        }
    }

    /// Send a raw REPL command and wait for the response.
    fn execute(&self, code: Vec<u8>) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(SpikeCommand::Execute { code, reply_tx: tx })
            .map_err(|_| "SPIKE slot channel closed".to_string())?;
        rx.recv_timeout(Duration::from_secs(30))
            .map_err(|_| "SPIKE command timed out".to_string())?
    }

    /// Execute a command, ignore stdout, return Ok/Err.
    fn execute_void(&self, code: Vec<u8>) -> Result<(), String> {
        self.execute(code).map(|_| ())
    }
}

impl HardwareAdapter for SpikeAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &[] }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.tx.is_some() }
    fn max_power(&self) -> u8 { MAX_POWER }

    fn connect(&mut self) -> Result<(), String> {
        let path = if let Some(ref id) = self.identifier {
            id.clone()
        } else {
            let ports = serialport::available_ports().map_err(|e| e.to_string())?;
            let lego_port = ports.iter().find(|p| {
                if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
                    info.vid == 0x0694
                } else {
                    false
                }
            });
            match lego_port {
                Some(p) => p.port_name.clone(),
                None => return Err("No SPIKE Prime hub found on USB. For Bluetooth, add the serial path to bricklogo.config.json".to_string()),
            }
        };

        let serial = serialport::new(&path, 115200)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| format!("Could not open {}: {}", path, e))?;

        let mut transport: Box<dyn SpikeTransport> = Box::new(serial);

        // Interrupt any running program and get to REPL
        transport.write_all(&[CTRL_C])?;
        transport.flush()?;
        std::thread::sleep(Duration::from_millis(500));
        transport.write_all(&[CTRL_C])?;
        transport.flush()?;
        std::thread::sleep(Duration::from_millis(500));

        // Enter raw REPL mode
        transport.write_all(&[CTRL_A])?;
        transport.flush()?;
        std::thread::sleep(Duration::from_millis(500));

        // Drain any startup text
        let mut drain_buf = [0u8; 4096];
        let _ = transport.read(&mut drain_buf);

        // Send imports and verify we get OK back
        let init = cmd_init_imports();
        transport.write_all(&init)?;
        transport.flush()?;

        // Wait for the response (OK\x04\x04)
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut response = Vec::new();
        while Instant::now() < deadline {
            let mut buf = [0u8; 256];
            match transport.read(&mut buf) {
                Ok(n) if n > 0 => {
                    response.extend_from_slice(&buf[..n]);
                    let eot_count = response.iter().filter(|&&b| b == CTRL_D).count();
                    if eot_count >= 2 {
                        break;
                    }
                }
                _ => std::thread::sleep(Duration::from_millis(50)),
            }
        }

        let init_result = parse_raw_repl_response(&response);
        if let Err(ref e) = init_result {
            return Err(format!("SPIKE init failed: {}", e));
        }

        let (tx, rx) = mpsc::channel();
        let slot = SpikeSlot {
            port: transport,
            rx,
            response_buf: Vec::new(),
            alive: true,
            pending: None,
        };
        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.slot_id = Some(slot_id);
        Ok(())
    }

    fn disconnect(&mut self) {
        // Stop all motors
        if self.tx.is_some() {
            for port in OUTPUT_PORTS {
                let _ = self.execute_void(cmd_motor_stop(port));
            }
        }
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if OUTPUT_PORTS.contains(&port) {
            Ok(())
        } else {
            Err(format!("Unknown output port \"{}\"", port))
        }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        if port_index(port).is_some() {
            if let Some(m) = mode {
                let known = matches!(
                    m,
                    "rotation" | "speed" | "absolute" | "color" | "light"
                        | "distance" | "force" | "touched" | "raw"
                );
                if !known {
                    return Err(format!("Unsupported sensor mode \"{}\"", m));
                }
            }
            return Ok(());
        }
        Err(format!("Unknown sensor port \"{}\"", port))
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        self.execute_void(cmd_motor_run(port, to_velocity(direction, power)))
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        self.execute_void(cmd_motor_stop(port))
    }

    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String> {
        let ms = tenths * 100;
        let vel = to_velocity(direction, power);
        self.execute_void(cmd_motor_run_for_time(port, ms, vel))
    }

    fn rotate_port_by_degrees(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        degrees: i32,
    ) -> Result<(), String> {
        let vel = to_velocity(direction, power);
        self.execute_void(cmd_motor_run_for_degrees(port, degrees.abs(), vel))
    }

    fn rotate_port_to_position(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
    ) -> Result<(), String> {
        // Read current relative position, compute delta
        let current = self.execute(cmd_read_relative_position(port))?
            .trim().parse::<i32>().map_err(|e| e.to_string())?;
        let delta = crate::adapter::rotateto_delta(current, position, direction);
        if delta == 0 {
            return Ok(());
        }
        let speed = if delta > 0 {
            (power.min(MAX_POWER) as i32) * 10
        } else {
            -((power.min(MAX_POWER) as i32) * 10)
        };
        self.execute_void(cmd_motor_run_for_degrees(port, delta.abs(), speed))
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        self.execute_void(cmd_motor_reset_relative_position(port, 0))
    }

    fn rotate_to_abs(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
    ) -> Result<(), String> {
        let vel = (power.min(MAX_POWER) as i32) * 10;
        let dir = match direction {
            PortDirection::Even => 1, // clockwise
            PortDirection::Odd => 2,  // counterclockwise
        };
        self.execute_void(cmd_motor_run_to_absolute_position(port, position, vel, dir))
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let mode_name = mode.unwrap_or("raw");
        let code = match mode_name {
            "rotation" => cmd_read_relative_position(port),
            "speed" => cmd_read_velocity(port),
            "absolute" => cmd_read_absolute_position(port),
            "color" => cmd_read_color(port),
            "light" => cmd_read_reflection(port),
            "distance" => cmd_read_distance(port),
            "force" => cmd_read_force(port),
            "touched" => cmd_read_force_touched(port),
            "raw" => cmd_read_relative_position(port),
            _ => return Err(format!("Unsupported sensor mode \"{}\"", mode_name)),
        };
        let stdout = self.execute(code)?;
        if stdout.is_empty() {
            return Ok(Some(LogoValue::Number(0.0)));
        }
        // "touched" / "pressed" returns True/False
        if mode_name == "touched" {
            let pressed = stdout.to_lowercase().contains("true");
            return Ok(Some(LogoValue::Word(
                if pressed { "true" } else { "false" }.to_string(),
            )));
        }
        // Numeric value
        match stdout.trim().parse::<f64>() {
            Ok(n) => Ok(Some(LogoValue::Number(n))),
            Err(_) => Ok(Some(LogoValue::Word(stdout))),
        }
    }

    // ── Batch overrides ─────────────────────────

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        let ms = tenths * 100;
        let entries: Vec<(&str, i32)> = commands
            .iter()
            .map(|c| (c.port, to_velocity(c.direction, c.power)))
            .collect();
        self.execute_void(cmd_parallel_run_for_time(&entries, ms))
    }

    fn rotate_ports_by_degrees(
        &mut self,
        commands: &[PortCommand],
        degrees: i32,
    ) -> Result<(), String> {
        let entries: Vec<(&str, i32, i32)> = commands
            .iter()
            .map(|c| (c.port, degrees.abs(), to_velocity(c.direction, c.power)))
            .collect();
        self.execute_void(cmd_parallel_run_for_degrees(&entries))
    }

    fn rotate_ports_to_abs(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        let entries: Vec<(&str, i32, i32, u8)> = commands
            .iter()
            .map(|c| {
                let vel = (c.power.min(MAX_POWER) as i32) * 10;
                let dir = match c.direction {
                    PortDirection::Even => 1,
                    PortDirection::Odd => 2,
                };
                (c.port, position, vel, dir)
            })
            .collect();
        self.execute_void(cmd_parallel_run_to_absolute(&entries))
    }
}

#[cfg(test)]
#[path = "../tests/spike_adapter.rs"]
mod tests;
