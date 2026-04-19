//! LEGO SPIKE Prime / Robot Inventor adapter.
//!
//! Talks to the hub's Scratch VM runtime over serial (USB or Bluetooth SPP).
//! The protocol is JSON lines terminated with \r. The hub enters "play" mode
//! on init and accepts `scratch.*` motor/sensor commands, responding with
//! task-ID-correlated completions and continuous telemetry.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex, mpsc};
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

fn to_signed_speed(direction: PortDirection, power: u8) -> i32 {
    let p = power.min(MAX_POWER) as i32;
    match direction {
        PortDirection::Even => p,
        PortDirection::Odd => -p,
    }
}

fn port_to_upper(port: &str) -> String {
    port.to_uppercase()
}

// ── Shared state ────────────────────────────────

pub struct SpikeShared {
    pub ports: [PortTelemetry; PORT_COUNT],
    pub imu: ImuData,
}

impl SpikeShared {
    fn new() -> Self {
        SpikeShared {
            ports: Default::default(),
            imu: ImuData::default(),
        }
    }
}

// ── Slot commands ───────────────────────────────

type ReplyTx = mpsc::Sender<Result<(), String>>;

enum SpikeCommand {
    /// Fire-and-forget motor start.
    MotorStart { port: String, speed: i32, reply_tx: ReplyTx },
    /// Fire-and-forget motor stop.
    MotorStop { port: String, stop: u8, reply_tx: ReplyTx },
    /// Awaited: run for time.
    MotorRunTimed { port: String, speed: i32, time_ms: u32, stop: u8, reply_tx: ReplyTx },
    /// Awaited: run for degrees.
    MotorRunDegrees { port: String, speed: i32, degrees: i32, stop: u8, reply_tx: ReplyTx },
    /// Awaited: go to absolute position with direction.
    MotorGoToPosition { port: String, position: i32, speed: i32, direction: String, stop: u8, reply_tx: ReplyTx },
    /// Fire-and-forget: reset encoder.
    MotorSetPosition { port: String, offset: i32, reply_tx: ReplyTx },
    /// Fire-and-forget: dual motor start.
    MoveStartSpeeds { lmotor: String, rmotor: String, lspeed: i32, rspeed: i32, reply_tx: ReplyTx },
    /// Fire-and-forget: dual motor stop.
    MoveStop { lmotor: String, rmotor: String, stop: u8, reply_tx: ReplyTx },
}

// ── Slot ────────────────────────────────────────

struct PendingTask {
    task_id: String,
    reply_tx: ReplyTx,
}

struct SpikeSlot {
    port: Box<dyn SpikeTransport>,
    rx: mpsc::Receiver<SpikeCommand>,
    shared: Arc<Mutex<SpikeShared>>,
    read_buffer: String,
    alive: bool,
    task_id_gen: TaskIdGen,
    pending: Vec<PendingTask>,
}

impl SpikeSlot {
    fn write_cmd(&mut self, cmd: &str) -> Result<(), String> {
        self.port.write_all(cmd.as_bytes())?;
        self.port.flush()
    }

    fn next_id(&mut self) -> String {
        self.task_id_gen.next()
    }
}

impl DeviceSlot for SpikeSlot {
    fn tick(&mut self) {
        // ── Read serial data ──────────────────
        let mut buf = [0u8; 1024];
        match self.port.read(&mut buf) {
            Ok(n) if n > 0 => {
                self.read_buffer.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            Err(_) => {
                self.alive = false;
                return;
            }
            _ => {}
        }

        // Process complete lines (terminated by \r)
        while let Some(cr_pos) = self.read_buffer.find('\r') {
            let line = self.read_buffer[..cr_pos].trim().to_string();
            self.read_buffer = self.read_buffer[cr_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            match parse_message(&line) {
                SpikeMessage::TaskComplete { task_id, result } => {
                    // Fire any pending reply for this task ID.
                    if let Some(idx) = self.pending.iter().position(|p| p.task_id == task_id) {
                        let entry = self.pending.remove(idx);
                        // result == 0 or null typically means success
                        let _ = entry.reply_tx.send(Ok(()));
                        let _ = result; // consumed
                    }
                }
                SpikeMessage::Telemetry(data) => {
                    let mut shared = self.shared.lock().unwrap();
                    shared.ports = data.ports;
                    shared.imu = data.imu;
                }
                _ => {}
            }
        }

        // ── Drain command queue ───────────────
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                SpikeCommand::MotorStart { port, speed, reply_tx } => {
                    let id = self.next_id();
                    let r = self.write_cmd(&cmd_motor_start(&id, &port, speed, true));
                    let _ = reply_tx.send(r);
                }
                SpikeCommand::MotorStop { port, stop, reply_tx } => {
                    let id = self.next_id();
                    let r = self.write_cmd(&cmd_motor_stop(&id, &port, stop, DEFAULT_DECEL));
                    let _ = reply_tx.send(r);
                }
                SpikeCommand::MotorRunTimed { port, speed, time_ms, stop, reply_tx } => {
                    let id = self.next_id();
                    match self.write_cmd(&cmd_motor_run_timed(&id, &port, speed, time_ms, true, stop)) {
                        Ok(()) => self.pending.push(PendingTask { task_id: id, reply_tx }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                SpikeCommand::MotorRunDegrees { port, speed, degrees, stop, reply_tx } => {
                    let id = self.next_id();
                    match self.write_cmd(&cmd_motor_run_for_degrees(&id, &port, speed, degrees, true, stop)) {
                        Ok(()) => self.pending.push(PendingTask { task_id: id, reply_tx }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                SpikeCommand::MotorGoToPosition { port, position, speed, direction, stop, reply_tx } => {
                    let id = self.next_id();
                    match self.write_cmd(&cmd_motor_go_direction_to_position(
                        &id, &port, position, speed, &direction, true, stop,
                    )) {
                        Ok(()) => self.pending.push(PendingTask { task_id: id, reply_tx }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                SpikeCommand::MotorSetPosition { port, offset, reply_tx } => {
                    let id = self.next_id();
                    let r = self.write_cmd(&cmd_motor_set_position(&id, &port, offset));
                    let _ = reply_tx.send(r);
                }
                SpikeCommand::MoveStartSpeeds { lmotor, rmotor, lspeed, rspeed, reply_tx } => {
                    let id = self.next_id();
                    let r = self.write_cmd(&cmd_move_start_speeds(&id, &lmotor, &rmotor, lspeed, rspeed));
                    let _ = reply_tx.send(r);
                }
                SpikeCommand::MoveStop { lmotor, rmotor, stop, reply_tx } => {
                    let id = self.next_id();
                    let r = self.write_cmd(&cmd_move_stop(&id, &lmotor, &rmotor, stop));
                    let _ = reply_tx.send(r);
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
    shared: Arc<Mutex<SpikeShared>>,
    slot_id: Option<usize>,
    display_name: String,
    identifier: Option<String>,
}

impl SpikeAdapter {
    pub fn new(identifier: Option<&str>) -> Self {
        SpikeAdapter {
            tx: None,
            shared: Arc::new(Mutex::new(SpikeShared::new())),
            slot_id: None,
            display_name: "LEGO SPIKE Prime".to_string(),
            identifier: identifier.map(|s| s.to_string()),
        }
    }

    fn send_and_wait(&self, cmd_builder: impl FnOnce(ReplyTx) -> SpikeCommand) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(cmd_builder(tx))
            .map_err(|_| "SPIKE slot channel closed".to_string())?;
        rx.recv_timeout(Duration::from_secs(30))
            .map_err(|_| "SPIKE command timed out".to_string())?
    }

    fn require_tacho(&self, port: &str) -> Result<(), String> {
        let idx = port_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let shared = self.shared.lock().unwrap();
        let type_id = shared.ports[idx].device_type;
        if type_id == 0 {
            return Err(format!("No device connected on port {}", port));
        }
        if !is_tacho_motor(type_id) {
            return Err(format!("Device on port {} is not a tacho motor", port));
        }
        Ok(())
    }

    fn require_absolute(&self, port: &str) -> Result<(), String> {
        let idx = port_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let shared = self.shared.lock().unwrap();
        let type_id = shared.ports[idx].device_type;
        if !is_absolute_motor(type_id) {
            return Err(format!(
                "Device on port {} does not support absolute position — use rotateto after resetzero instead",
                port
            ));
        }
        Ok(())
    }

    fn read_motor_data(&self, port: &str) -> Result<[f64; 4], String> {
        let idx = port_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let shared = self.shared.lock().unwrap();
        Ok(shared.ports[idx].data)
    }
}

impl HardwareAdapter for SpikeAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &[] } // populated after connect
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool { self.tx.is_some() }
    fn max_power(&self) -> u8 { MAX_POWER }

    fn connect(&mut self) -> Result<(), String> {
        // Open serial port — either from identifier (config) or auto-detect USB
        let path = if let Some(ref id) = self.identifier {
            id.clone()
        } else {
            // Auto-detect: scan for LEGO USB VID
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

        // Wait for hub to be ready
        std::thread::sleep(Duration::from_secs(2));

        // Enter play mode
        let modechange = cmd_program_modechange();
        transport.write_all(modechange.as_bytes())?;
        transport.flush()?;

        // Read initial telemetry for ~1 second to populate port info
        let shared = Arc::new(Mutex::new(SpikeShared::new()));
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut buf = [0u8; 1024];
        let mut read_buffer = String::new();
        while Instant::now() < deadline {
            match transport.read(&mut buf) {
                Ok(n) if n > 0 => {
                    read_buffer.push_str(&String::from_utf8_lossy(&buf[..n]));
                    while let Some(cr_pos) = read_buffer.find('\r') {
                        let line = read_buffer[..cr_pos].trim().to_string();
                        read_buffer = read_buffer[cr_pos + 1..].to_string();
                        if let SpikeMessage::Telemetry(data) = parse_message(&line) {
                            let mut s = shared.lock().unwrap();
                            s.ports = data.ports;
                            s.imu = data.imu;
                        }
                    }
                }
                _ => std::thread::sleep(Duration::from_millis(50)),
            }
        }

        let (tx, rx) = mpsc::channel();
        let slot = SpikeSlot {
            port: transport,
            rx,
            shared: shared.clone(),
            read_buffer,
            alive: true,
            task_id_gen: TaskIdGen::new(),
            pending: Vec::new(),
        };
        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.shared = shared;
        self.slot_id = Some(slot_id);
        Ok(())
    }

    fn disconnect(&mut self) {
        // Stop all motors
        if self.tx.is_some() {
            for port in OUTPUT_PORTS {
                let _ = self.stop_port(port);
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
        // Accept any port letter (motors have encoder sensors) or IMU names
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
        // Hub-level IMU sensors
        if matches!(port, "tilt" | "gyro" | "accel") {
            return Ok(());
        }
        Err(format!("Unknown sensor port \"{}\"", port))
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let speed = to_signed_speed(direction, power);
        self.send_and_wait(|tx| SpikeCommand::MotorStart {
            port: port_to_upper(port),
            speed,
            reply_tx: tx,
        })
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        self.send_and_wait(|tx| SpikeCommand::MotorStop {
            port: port_to_upper(port),
            stop: STOP_BRAKE,
            reply_tx: tx,
        })
    }

    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String> {
        let speed = to_signed_speed(direction, power);
        self.send_and_wait(|tx| SpikeCommand::MotorRunTimed {
            port: port_to_upper(port),
            speed,
            time_ms: tenths * 100,
            stop: STOP_BRAKE,
            reply_tx: tx,
        })
    }

    fn rotate_port_by_degrees(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        degrees: i32,
    ) -> Result<(), String> {
        self.require_tacho(port)?;
        let speed = to_signed_speed(direction, power);
        self.send_and_wait(|tx| SpikeCommand::MotorRunDegrees {
            port: port_to_upper(port),
            speed,
            degrees: degrees.abs(),
            stop: STOP_BRAKE,
            reply_tx: tx,
        })
    }

    fn rotate_port_to_position(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
    ) -> Result<(), String> {
        self.require_tacho(port)?;
        let speed = power.min(MAX_POWER) as i32;
        let dir = match direction {
            PortDirection::Even => "clockwise",
            PortDirection::Odd => "counterclockwise",
        };
        self.send_and_wait(|tx| SpikeCommand::MotorGoToPosition {
            port: port_to_upper(port),
            position: position % 360,
            speed,
            direction: dir.to_string(),
            stop: STOP_BRAKE,
            reply_tx: tx,
        })
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        self.require_tacho(port)?;
        self.send_and_wait(|tx| SpikeCommand::MotorSetPosition {
            port: port_to_upper(port),
            offset: 0,
            reply_tx: tx,
        })
    }

    fn rotate_to_home(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
    ) -> Result<(), String> {
        self.require_absolute(port)?;
        let data = self.read_motor_data(port)?;
        let apos = data[2] as i32; // absolute position
        let delta = crate::adapter::rotate_home_delta(apos, direction);
        if delta == 0 {
            return Ok(());
        }
        let speed = if delta > 0 {
            power.min(MAX_POWER) as i32
        } else {
            -(power.min(MAX_POWER) as i32)
        };
        self.send_and_wait(|tx| SpikeCommand::MotorRunDegrees {
            port: port_to_upper(port),
            speed,
            degrees: delta.abs(),
            stop: STOP_BRAKE,
            reply_tx: tx,
        })
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        // Hub-level IMU sensors
        match port {
            "tilt" => {
                let shared = self.shared.lock().unwrap();
                let ypr = &shared.imu.yaw_pitch_roll;
                return Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(ypr[1]), // pitch
                    LogoValue::Number(ypr[2]), // roll
                ])));
            }
            "gyro" => {
                let shared = self.shared.lock().unwrap();
                let g = &shared.imu.gyro;
                return Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(g[0]),
                    LogoValue::Number(g[1]),
                    LogoValue::Number(g[2]),
                ])));
            }
            "accel" => {
                let shared = self.shared.lock().unwrap();
                let a = &shared.imu.accel;
                return Ok(Some(LogoValue::List(vec![
                    LogoValue::Number(a[0]),
                    LogoValue::Number(a[1]),
                    LogoValue::Number(a[2]),
                ])));
            }
            _ => {}
        }

        let idx = port_index(port).ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        let shared = self.shared.lock().unwrap();
        let pt = &shared.ports[idx];

        if pt.device_type == 0 {
            return Err(format!("No device connected on port {}", port));
        }

        let mode_name = mode.unwrap_or("raw");
        match mode_name {
            // Motor modes
            "rotation" => Ok(Some(LogoValue::Number(pt.data[1]))),
            "speed" => Ok(Some(LogoValue::Number(pt.data[0]))),
            "absolute" => {
                if !is_absolute_motor(pt.device_type) {
                    return Err("This motor does not have an absolute position encoder".to_string());
                }
                Ok(Some(LogoValue::Number(pt.data[2])))
            }
            // Color sensor
            "color" => Ok(Some(LogoValue::Number(pt.data[1]))),
            "light" => Ok(Some(LogoValue::Number(pt.data[0]))),
            // Distance sensor
            "distance" => Ok(Some(LogoValue::Number(pt.data[0]))),
            // Force sensor
            "force" => Ok(Some(LogoValue::Number(pt.data[0]))),
            "touched" => {
                let pressed = pt.data[0] > 0.0;
                Ok(Some(LogoValue::Word(if pressed { "true" } else { "false" }.to_string())))
            }
            // Raw: return first data value
            "raw" => Ok(Some(LogoValue::Number(pt.data[0]))),
            _ => Err(format!("Unsupported sensor mode \"{}\"", mode_name)),
        }
    }

    // ── Batch overrides ─────────────────────────

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        if commands.len() == 2 {
            let lspeed = to_signed_speed(commands[0].direction, commands[0].power);
            let rspeed = to_signed_speed(commands[1].direction, commands[1].power);
            return self.send_and_wait(|tx| SpikeCommand::MoveStartSpeeds {
                lmotor: port_to_upper(commands[0].port),
                rmotor: port_to_upper(commands[1].port),
                lspeed,
                rspeed,
                reply_tx: tx,
            });
        }
        for cmd in commands {
            self.start_port(cmd.port, cmd.direction, cmd.power)?;
        }
        Ok(())
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        if ports.len() == 2 {
            return self.send_and_wait(|tx| SpikeCommand::MoveStop {
                lmotor: port_to_upper(ports[0]),
                rmotor: port_to_upper(ports[1]),
                stop: STOP_BRAKE,
                reply_tx: tx,
            });
        }
        for port in ports {
            self.stop_port(port)?;
        }
        Ok(())
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        // Send all timed-run commands without blocking between them
        let mut receivers = Vec::with_capacity(commands.len());
        for cmd in commands {
            let speed = to_signed_speed(cmd.direction, cmd.power);
            let (tx, rx) = mpsc::channel();
            self.tx
                .as_ref()
                .ok_or("Not connected")?
                .send(SpikeCommand::MotorRunTimed {
                    port: port_to_upper(cmd.port),
                    speed,
                    time_ms: tenths * 100,
                    stop: STOP_BRAKE,
                    reply_tx: tx,
                })
                .map_err(|_| "SPIKE slot channel closed".to_string())?;
            receivers.push(rx);
        }
        // Wait for all completions
        for rx in receivers {
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|_| "SPIKE command timed out".to_string())??;
        }
        Ok(())
    }

    fn rotate_ports_by_degrees(
        &mut self,
        commands: &[PortCommand],
        degrees: i32,
    ) -> Result<(), String> {
        let mut receivers = Vec::with_capacity(commands.len());
        for cmd in commands {
            self.require_tacho(cmd.port)?;
            let speed = to_signed_speed(cmd.direction, cmd.power);
            let (tx, rx) = mpsc::channel();
            self.tx
                .as_ref()
                .ok_or("Not connected")?
                .send(SpikeCommand::MotorRunDegrees {
                    port: port_to_upper(cmd.port),
                    speed,
                    degrees: degrees.abs(),
                    stop: STOP_BRAKE,
                    reply_tx: tx,
                })
                .map_err(|_| "SPIKE slot channel closed".to_string())?;
            receivers.push(rx);
        }
        for rx in receivers {
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|_| "SPIKE command timed out".to_string())??;
        }
        Ok(())
    }

    fn rotate_ports_to_position(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        let mut receivers = Vec::with_capacity(commands.len());
        for cmd in commands {
            self.require_tacho(cmd.port)?;
            let speed = cmd.power.min(MAX_POWER) as i32;
            let dir = match cmd.direction {
                PortDirection::Even => "clockwise",
                PortDirection::Odd => "counterclockwise",
            };
            let (tx, rx) = mpsc::channel();
            self.tx
                .as_ref()
                .ok_or("Not connected")?
                .send(SpikeCommand::MotorGoToPosition {
                    port: port_to_upper(cmd.port),
                    position: position % 360,
                    speed,
                    direction: dir.to_string(),
                    stop: STOP_BRAKE,
                    reply_tx: tx,
                })
                .map_err(|_| "SPIKE slot channel closed".to_string())?;
            receivers.push(rx);
        }
        for rx in receivers {
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|_| "SPIKE command timed out".to_string())??;
        }
        Ok(())
    }

    fn rotate_ports_to_home(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        let mut receivers = Vec::with_capacity(commands.len());
        for cmd in commands {
            self.require_absolute(cmd.port)?;
            let data = self.read_motor_data(cmd.port)?;
            let apos = data[2] as i32;
            let delta = crate::adapter::rotate_home_delta(apos, cmd.direction);
            if delta == 0 {
                continue;
            }
            let speed = if delta > 0 {
                cmd.power.min(MAX_POWER) as i32
            } else {
                -(cmd.power.min(MAX_POWER) as i32)
            };
            let (tx, rx) = mpsc::channel();
            self.tx
                .as_ref()
                .ok_or("Not connected")?
                .send(SpikeCommand::MotorRunDegrees {
                    port: port_to_upper(cmd.port),
                    speed,
                    degrees: delta.abs(),
                    stop: STOP_BRAKE,
                    reply_tx: tx,
                })
                .map_err(|_| "SPIKE slot channel closed".to_string())?;
            receivers.push(rx);
        }
        for rx in receivers {
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|_| "SPIKE command timed out".to_string())??;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/spike_adapter.rs"]
mod tests;
