//! LEGO Mindstorms NXT adapter.
//!
//! Talks to the stock NXT firmware's LCP (Direct Commands) over USB bulk
//! or Bluetooth SPP (via a pre-paired serial port). The adapter picks the
//! transport based on the `identifier` passed to `new()`.
//!
//! NXT motors (9842 Interactive Servo) expose a 1-count-per-degree
//! quadrature encoder. There is no hardware-marked absolute zero, so
//! `rotate_to_abs` returns an error — same as EV3 and RCX — and the
//! BrickLogo relative-position model (`rotateto`, `rotateby`, `resetzero`)
//! is implemented on top of `tacho_count`, which the firmware resets via
//! `ResetMotorPosition(port, relative=true)`.
//!
//! Within-hub parallelism for batch motor ops follows the "plan then fire
//! then poll" pattern used by the EV3 and Build HAT adapters. NXT firmware
//! has no multi-port opcode, so the slot fires `SetOutputState` per port
//! with `NO_REPLY_FLAG` back-to-back (microseconds apart), then polls each
//! port's `GetOutputState` for `run_state == IDLE`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use bricklogo_lang::value::LogoValue;

use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use rust_nxt::constants::{self as nxt_const, SensorKind};
use rust_nxt::nxt::{Nxt, OutputStateSpec};
use rust_nxt::protocol as p;
use rust_nxt::serial::SerialTransport;
use rust_nxt::transport::Transport;
use rust_nxt::usb::UsbTransport;

const OUTPUT_PORTS: &[&str] = &["a", "b", "c"];
const INPUT_PORTS: &[&str] = &["1", "2", "3", "4"];
const MAX_POWER: u8 = 100;

// ── Port / mode helpers ──────────────────────────

fn port_to_index(port: &str) -> Option<u8> {
    match port {
        "a" => Some(0),
        "b" => Some(1),
        "c" => Some(2),
        _ => None,
    }
}

fn sensor_port_index(port: &str) -> Option<u8> {
    match port {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        _ => None,
    }
}

fn to_signed_power(direction: PortDirection, power: u8) -> i8 {
    let p = power.min(MAX_POWER) as i8;
    match direction {
        PortDirection::Even => p,
        PortDirection::Odd => -p,
    }
}

fn run_spec_for_start(port: u8, power: i8) -> OutputStateSpec {
    OutputStateSpec {
        port,
        power,
        mode: p::MODE_MOTORON | p::MODE_REGULATED,
        regulation: p::REG_MOTOR_SPEED,
        turn_ratio: 0,
        run_state: p::RUN_RUNNING,
        tacho_limit: 0,
        reply_required: true,
    }
}

/// Active brake: the motor holds position instead of coasting. On the NXT
/// "brake" means `MODE_MOTORON | MODE_BRAKE` with `RUN_RUNNING` and zero
/// power — with `RUN_IDLE` the firmware disengages the H-bridge and the
/// shaft floats freely, which is not what `off` means in BrickLogo.
fn stop_spec(port: u8) -> OutputStateSpec {
    OutputStateSpec {
        port,
        power: 0,
        mode: p::MODE_MOTORON | p::MODE_BRAKE,
        regulation: p::REG_IDLE,
        turn_ratio: 0,
        run_state: p::RUN_RUNNING,
        tacho_limit: 0,
        reply_required: true,
    }
}

/// Rotation spec: drive the motor indefinitely at `power`, and let the
/// slot's pending-step polling count out the tacho delta in software.
/// Hardware `tacho_limit` is unreliable on the NXT when `MODE_REGULATED`
/// is engaged — the firmware either refuses to start or brakes before it
/// moves — so we run open-ended (`tacho_limit = 0`) and stop ourselves.
fn step_spec(port: u8, power: i8, reply_required: bool) -> OutputStateSpec {
    OutputStateSpec {
        port,
        power,
        mode: p::MODE_MOTORON | p::MODE_REGULATED,
        regulation: p::REG_MOTOR_SPEED,
        turn_ratio: 0,
        run_state: p::RUN_RUNNING,
        tacho_limit: 0,
        reply_required,
    }
}

fn time_run_spec(port: u8, power: i8, reply_required: bool) -> OutputStateSpec {
    OutputStateSpec {
        port,
        power,
        mode: p::MODE_MOTORON | p::MODE_REGULATED,
        regulation: p::REG_MOTOR_SPEED,
        turn_ratio: 0,
        run_state: p::RUN_RUNNING,
        tacho_limit: 0,
        reply_required,
    }
}

// ── Slot ─────────────────────────────────────────

type ReplyTx = mpsc::Sender<Result<(), String>>;
type SensorReplyTx = mpsc::Sender<Result<Option<LogoValue>, String>>;

enum NxtCommand {
    MotorSetAndStart { port: u8, power: i8, reply_tx: ReplyTx },
    MotorStop { port: u8, reply_tx: ReplyTx },
    MotorStep { port: u8, power: i8, degrees: u32, reply_tx: ReplyTx },
    MotorTime { port: u8, power: i8, ms: u64, reply_tx: ReplyTx },
    /// Fire per-port step commands with NO_REPLY_FLAG back-to-back, then
    /// poll each port's `GetOutputState` for completion. Each entry is
    /// (port_index, signed_power, abs_degrees).
    MotorStepBatch { entries: Vec<(u8, i8, u32)>, reply_tx: ReplyTx },
    /// Same pattern for timed runs. Entries are (port_index, signed_power),
    /// deadline is shared across ports.
    MotorTimeBatch { entries: Vec<(u8, i8)>, ms: u64, reply_tx: ReplyTx },
    MotorClrCount { port: u8, reply_tx: ReplyTx },
    ReadMotorCount { port: u8, reply_tx: SensorReplyTx },
    SetInputMode { port: u8, sensor_type: u8, sensor_mode: u8, reply_tx: ReplyTx },
    ReadSensor { port: u8, kind: SensorKind, reply_tx: SensorReplyTx },
}

/// Pending step completion tracked in software: each port's current
/// `tacho_count` is compared against a snapshot taken just before the
/// fire. When `|current - start| >= target_abs_degrees`, the slot sends an
/// explicit stop command for that port.
///
/// We use `tacho_count` (the firmware's never-resettable rolling counter)
/// rather than `rotation_count` so that a `resetzero` issued mid-flight —
/// which clears `rotation_count` — can't cause the delta calculation to
/// jump and spuriously complete the step.
struct PendingPort {
    port: u8,
    start_tacho: i32,
    target_abs_degrees: u32,
    done: bool,
}

struct PendingStep {
    ports: Vec<PendingPort>,
    reply_tx: ReplyTx,
}

/// Pending timed run: wait until `deadline`, then stop the listed ports.
struct PendingTime {
    ports: Vec<u8>,
    deadline: Instant,
    reply_tx: ReplyTx,
}

struct NxtSlot {
    nxt: Nxt,
    rx: mpsc::Receiver<NxtCommand>,
    alive: Arc<AtomicBool>,
    pending_steps: Vec<PendingStep>,
    pending_times: Vec<PendingTime>,
}

impl NxtSlot {
    /// Snapshot each port's `tacho_count`, then fire all SetOutputState
    /// commands back-to-back with NO_REPLY_FLAG. The snapshots go into the
    /// returned `PendingPort` list so the poll loop knows where each motor
    /// started and when to stop it.
    fn start_batch_step(
        &mut self,
        entries: &[(u8, i8, u32)],
    ) -> Result<Vec<PendingPort>, String> {
        let mut ports = Vec::with_capacity(entries.len());
        for &(port, _power, degrees) in entries {
            let start = self.nxt.get_output_state(port)?.tacho_count;
            ports.push(PendingPort {
                port,
                start_tacho: start,
                target_abs_degrees: degrees,
                done: false,
            });
        }
        for &(port, power, _) in entries {
            self.nxt.set_output_state(&step_spec(port, power, false))?;
        }
        Ok(ports)
    }

    fn fire_batch_time_start(&mut self, entries: &[(u8, i8)]) -> Result<(), String> {
        for &(port, power) in entries {
            self.nxt
                .set_output_state(&time_run_spec(port, power, false))?;
        }
        Ok(())
    }

    fn fire_batch_time_stop(&mut self, ports: &[u8]) {
        for &port in ports {
            let _ = self.nxt.set_output_state(&OutputStateSpec {
                reply_required: false,
                ..stop_spec(port)
            });
        }
    }
}

impl DeviceSlot for NxtSlot {
    fn tick(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                NxtCommand::MotorSetAndStart { port, power, reply_tx } => {
                    let r = self.nxt.set_output_state(&run_spec_for_start(port, power));
                    let _ = reply_tx.send(r);
                }
                NxtCommand::MotorStop { port, reply_tx } => {
                    let r = self.nxt.set_output_state(&stop_spec(port));
                    let _ = reply_tx.send(r);
                }
                NxtCommand::MotorStep { port, power, degrees, reply_tx } => {
                    match self.start_batch_step(&[(port, power, degrees)]) {
                        Ok(ports) => self
                            .pending_steps
                            .push(PendingStep { ports, reply_tx }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                NxtCommand::MotorTime { port, power, ms, reply_tx } => {
                    match self.nxt.set_output_state(&time_run_spec(port, power, false)) {
                        Ok(()) => self.pending_times.push(PendingTime {
                            ports: vec![port],
                            deadline: Instant::now() + Duration::from_millis(ms),
                            reply_tx,
                        }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                NxtCommand::MotorStepBatch { entries, reply_tx } => {
                    match self.start_batch_step(&entries) {
                        Ok(ports) => self
                            .pending_steps
                            .push(PendingStep { ports, reply_tx }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                NxtCommand::MotorTimeBatch { entries, ms, reply_tx } => {
                    let ports: Vec<u8> = entries.iter().map(|(p, _)| *p).collect();
                    match self.fire_batch_time_start(&entries) {
                        Ok(()) => self.pending_times.push(PendingTime {
                            ports,
                            deadline: Instant::now() + Duration::from_millis(ms),
                            reply_tx,
                        }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                NxtCommand::MotorClrCount { port, reply_tx } => {
                    // `ResetMotorPosition(relative=false)` clears
                    // `rotation_count`, which is the counter `sensor "rotation`
                    // exposes and `rotateto` measures against. The firmware's
                    // other counters (tacho_count, block_tacho_count) are
                    // untouched — that keeps a mid-flight rotate's software
                    // tracking intact even if the user calls resetzero while
                    // a motor is moving.
                    let r = self.nxt.reset_motor_position(port, false);
                    let _ = reply_tx.send(r);
                }
                NxtCommand::ReadMotorCount { port, reply_tx } => {
                    let r = self.nxt.get_output_state(port)
                        .map(|s| Some(LogoValue::Number(s.rotation_count as f64)));
                    let _ = reply_tx.send(r);
                }
                NxtCommand::SetInputMode { port, sensor_type, sensor_mode, reply_tx } => {
                    let r = self.nxt.set_input_mode(port, sensor_type, sensor_mode);
                    let _ = reply_tx.send(r);
                }
                NxtCommand::ReadSensor { port, kind, reply_tx } => {
                    let r = self.nxt.get_input_values(port).map(|v| {
                        let value = match kind {
                            SensorKind::Bool | SensorKind::Pct => v.scaled as f64,
                            SensorKind::Raw => v.raw_ad as f64,
                        };
                        Some(LogoValue::Number(value))
                    });
                    let _ = reply_tx.send(r);
                }
            }
        }

        // Poll step completions. Each port's current `rotation_count` is
        // compared against the snapshot taken just before the fire; once
        // the delta reaches the target, the slot sends an explicit brake.
        // The whole pending entry completes when every port is done.
        if !self.pending_steps.is_empty() {
            let mut still = Vec::with_capacity(self.pending_steps.len());
            let drained: Vec<PendingStep> = self.pending_steps.drain(..).collect();
            for mut entry in drained {
                let mut err: Option<String> = None;
                for pp in entry.ports.iter_mut() {
                    if pp.done {
                        continue;
                    }
                    match self.nxt.get_output_state(pp.port) {
                        Ok(s) => {
                            let delta =
                                (s.tacho_count - pp.start_tacho).unsigned_abs();
                            if delta >= pp.target_abs_degrees {
                                let _ = self.nxt.set_output_state(&OutputStateSpec {
                                    reply_required: false,
                                    ..stop_spec(pp.port)
                                });
                                pp.done = true;
                            }
                        }
                        Err(e) => {
                            err = Some(e);
                            break;
                        }
                    }
                }
                if let Some(e) = err {
                    let _ = entry.reply_tx.send(Err(e));
                    continue;
                }
                if entry.ports.iter().all(|p| p.done) {
                    let _ = entry.reply_tx.send(Ok(()));
                } else {
                    still.push(entry);
                }
            }
            self.pending_steps = still;
        }

        // Handle time-based runs: stop the ports when the deadline passes.
        if !self.pending_times.is_empty() {
            let now = Instant::now();
            let drained: Vec<PendingTime> = self.pending_times.drain(..).collect();
            let mut still = Vec::with_capacity(drained.len());
            for entry in drained {
                if now >= entry.deadline {
                    self.fire_batch_time_stop(&entry.ports);
                    let _ = entry.reply_tx.send(Ok(()));
                } else {
                    still.push(entry);
                }
            }
            self.pending_times = still;
        }
    }

    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

// ── Adapter ──────────────────────────────────────

pub struct NxtAdapter {
    tx: Option<mpsc::Sender<NxtCommand>>,
    slot_id: Option<usize>,
    alive: Arc<AtomicBool>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    identifier: Option<String>,
    /// Remembers the `(sensor_type, sensor_mode)` last applied to each
    /// sensor port, so consecutive reads of the same mode skip the
    /// redundant `SetInputMode` round-trip.
    sensor_modes: Arc<Mutex<HashMap<u8, (u8, u8)>>>,
}

impl NxtAdapter {
    pub fn new(identifier: Option<&str>) -> Self {
        NxtAdapter {
            tx: None,
            slot_id: None,
            alive: Arc::new(AtomicBool::new(false)),
            display_name: "LEGO Mindstorms NXT".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            identifier: identifier.map(|s| s.to_string()),
            sensor_modes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Dispatch the `identifier` to the correct transport.
    ///
    ///   - `None` / `"usb"` → USB bulk, first unclaimed NXT
    ///   - `"usb:<serial>"` → USB bulk, the brick with that iSerial
    ///   - anything else → Bluetooth SPP serial path
    fn open_transport(identifier: Option<&str>) -> Result<Box<dyn Transport>, String> {
        match identifier {
            None | Some("usb") => Ok(Box::new(UsbTransport::open(None)?)),
            Some(ident) if ident.starts_with("usb:") => {
                let serial = &ident["usb:".len()..];
                Ok(Box::new(UsbTransport::open(Some(serial))?))
            }
            Some(path) => Ok(Box::new(SerialTransport::open(path)?)),
        }
    }

    /// Construct an adapter with a pre-built `Nxt` handle. Used by tests to
    /// bypass the real transport; not part of the public hardware API.
    #[cfg(test)]
    pub(crate) fn with_connected_nxt(nxt: Nxt) -> (Self, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        let slot = NxtSlot {
            nxt,
            rx,
            alive: alive.clone(),
            pending_steps: Vec::new(),
            pending_times: Vec::new(),
        };
        let slot_id = scheduler::register_slot(Box::new(slot));
        let adapter = NxtAdapter {
            tx: Some(tx),
            slot_id: Some(slot_id),
            alive: alive.clone(),
            display_name: "LEGO Mindstorms NXT".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            identifier: None,
            sensor_modes: Arc::new(Mutex::new(HashMap::new())),
        };
        (adapter, alive)
    }

    fn send_and_wait(
        &self,
        build: impl FnOnce(ReplyTx) -> NxtCommand,
    ) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(build(tx))
            .map_err(|_| "NXT slot channel closed".to_string())?;
        rx.recv_timeout(Duration::from_secs(30))
            .map_err(|_| "NXT command timed out".to_string())?
    }

    fn read_motor_count(&self, port: u8) -> Result<i32, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(NxtCommand::ReadMotorCount { port, reply_tx: tx })
            .map_err(|_| "NXT slot channel closed".to_string())?;
        match rx
            .recv_timeout(Duration::from_millis(500))
            .map_err(|_| "NXT motor-count read timed out".to_string())??
        {
            Some(LogoValue::Number(n)) => Ok(n as i32),
            _ => Ok(0),
        }
    }
}

impl HardwareAdapter for NxtAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &self.input_ports }
    fn connected(&self) -> bool {
        self.tx.is_some() && self.alive.load(Ordering::SeqCst)
    }

    fn connect(&mut self) -> Result<(), String> {
        let transport = Self::open_transport(self.identifier.as_deref())?;
        let mut nxt = Nxt::new(transport);

        // Sanity-check the connection: querying firmware version rejects
        // wrong-device serial paths with a clean error instead of stalling.
        let _ = nxt
            .get_firmware_version()
            .map_err(|e| format!("No NXT brick responded on this transport: {}", e))?;

        let (tx, rx) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        let slot = NxtSlot {
            nxt,
            rx,
            alive: alive.clone(),
            pending_steps: Vec::new(),
            pending_times: Vec::new(),
        };
        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.slot_id = Some(slot_id);
        self.alive = alive;
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(tx) = self.tx.as_ref() {
            for &port in &[0u8, 1, 2] {
                let (reply_tx, _rx) = mpsc::channel();
                let _ = tx.send(NxtCommand::MotorStop { port, reply_tx });
            }
        }
        self.alive.store(false, Ordering::SeqCst);
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
        self.sensor_modes.lock().unwrap().clear();
    }

    fn max_power(&self) -> u8 { MAX_POWER }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if OUTPUT_PORTS.contains(&port) {
            Ok(())
        } else {
            Err(format!("Unknown output port \"{}\"", port))
        }
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        // Motor ports (a/b/c) are valid sensor ports when reading tacho
        // rotation: `listento "a sensor "rotation` is how the language
        // layer asks for motor position.
        if port_to_index(port).is_some() {
            match mode {
                Some("rotation") | Some("raw") => return Ok(()),
                Some(m) => {
                    return Err(format!("Mode \"{}\" not supported on motor ports", m));
                }
                None => return Ok(()),
            }
        }
        let _idx = sensor_port_index(port)
            .ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        if let Some(m) = mode {
            if nxt_const::lookup_mode(m).is_none() {
                return Err(format!("Unsupported sensor mode \"{}\" for NXT", m));
            }
        }
        Ok(())
    }

    fn start_port(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
    ) -> Result<(), String> {
        let idx = port_to_index(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let signed = to_signed_power(direction, power);
        self.send_and_wait(|tx| NxtCommand::MotorSetAndStart {
            port: idx,
            power: signed,
            reply_tx: tx,
        })
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let idx = port_to_index(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        self.send_and_wait(|tx| NxtCommand::MotorStop { port: idx, reply_tx: tx })
    }

    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String> {
        let idx = port_to_index(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let signed = to_signed_power(direction, power);
        self.send_and_wait(|tx| NxtCommand::MotorTime {
            port: idx,
            power: signed,
            ms: tenths as u64 * 100,
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
        let idx = port_to_index(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let signed = to_signed_power(direction, power);
        self.send_and_wait(|tx| NxtCommand::MotorStep {
            port: idx,
            power: signed,
            degrees: degrees.unsigned_abs(),
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
        let idx = port_to_index(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let current = self.read_motor_count(idx)?;
        let delta = crate::adapter::rotateto_delta(current, position, direction);
        if delta == 0 {
            return Ok(());
        }
        let signed = if delta > 0 {
            to_signed_power(PortDirection::Even, power)
        } else {
            to_signed_power(PortDirection::Odd, power)
        };
        self.send_and_wait(|tx| NxtCommand::MotorStep {
            port: idx,
            power: signed,
            degrees: delta.unsigned_abs(),
            reply_tx: tx,
        })
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        let idx = port_to_index(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        self.send_and_wait(|tx| NxtCommand::MotorClrCount { port: idx, reply_tx: tx })
    }

    fn rotate_to_abs(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
        _position: i32,
    ) -> Result<(), String> {
        Err("NXT motors have no absolute-position encoder; use rotate (relative) instead"
            .to_string())
    }

    fn read_sensor(
        &mut self,
        port: &str,
        mode: Option<&str>,
    ) -> Result<Option<LogoValue>, String> {
        // Motor-port rotation read: `listento "a` + `sensor "rotation`.
        if let Some(motor_idx) = port_to_index(port) {
            if mode == Some("rotation") || mode == Some("raw") {
                let (tx, rx) = mpsc::channel();
                self.tx
                    .as_ref()
                    .ok_or("Not connected")?
                    .send(NxtCommand::ReadMotorCount { port: motor_idx, reply_tx: tx })
                    .map_err(|_| "NXT slot channel closed".to_string())?;
                return rx
                    .recv_timeout(Duration::from_millis(500))
                    .map_err(|_| "NXT motor-count read timed out".to_string())?;
            }
            return Err(format!(
                "Mode \"{}\" not supported on motor ports",
                mode.unwrap_or("?")
            ));
        }

        let idx = sensor_port_index(port)
            .ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        let mode_name = mode.ok_or_else(|| {
            format!("Sensor mode required for port {} (e.g. \"touch\")", port)
        })?;
        let (sensor_type, sensor_mode, kind) = nxt_const::lookup_mode(mode_name)
            .ok_or_else(|| format!("Unsupported sensor mode \"{}\" for NXT", mode_name))?;

        // Configure the port only when the mode changes, to spare the
        // firmware a round-trip on every read.
        let need_configure = self
            .sensor_modes
            .lock()
            .unwrap()
            .get(&idx)
            .copied()
            != Some((sensor_type, sensor_mode));
        if need_configure {
            let (tx, rx) = mpsc::channel();
            self.tx
                .as_ref()
                .ok_or("Not connected")?
                .send(NxtCommand::SetInputMode {
                    port: idx,
                    sensor_type,
                    sensor_mode,
                    reply_tx: tx,
                })
                .map_err(|_| "NXT slot channel closed".to_string())?;
            rx.recv_timeout(Duration::from_millis(500))
                .map_err(|_| "NXT set-input-mode timed out".to_string())??;
            self.sensor_modes
                .lock()
                .unwrap()
                .insert(idx, (sensor_type, sensor_mode));
            // Firmware needs a beat to apply the new configuration before
            // the next GetInputValues returns meaningful data.
            std::thread::sleep(Duration::from_millis(20));
        }

        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(NxtCommand::ReadSensor {
                port: idx,
                kind,
                reply_tx: tx,
            })
            .map_err(|_| "NXT slot channel closed".to_string())?;
        rx.recv_timeout(Duration::from_millis(500))
            .map_err(|_| "NXT sensor read timed out".to_string())?
    }

    // ── Batch overrides ─────────────────────────

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        // NXT has no multi-port opcode; fire each SetOutputState back-to-back
        // with NO_REPLY_FLAG for the leading commands, and wait on the last.
        if commands.is_empty() {
            return Ok(());
        }
        let last = commands.len() - 1;
        for (i, cmd) in commands.iter().enumerate() {
            let idx = port_to_index(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let signed = to_signed_power(cmd.direction, cmd.power);
            if i == last {
                self.send_and_wait(|tx| NxtCommand::MotorSetAndStart {
                    port: idx,
                    power: signed,
                    reply_tx: tx,
                })?;
            } else {
                // Fire-and-forget via the slot so earlier ports don't wait
                // on a round-trip before the next one goes out.
                let (reply_tx, _rx) = mpsc::channel();
                self.tx
                    .as_ref()
                    .ok_or("Not connected")?
                    .send(NxtCommand::MotorSetAndStart {
                        port: idx,
                        power: signed,
                        reply_tx,
                    })
                    .map_err(|_| "NXT slot channel closed".to_string())?;
            }
        }
        Ok(())
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        if ports.is_empty() {
            return Ok(());
        }
        let last = ports.len() - 1;
        for (i, port) in ports.iter().enumerate() {
            let idx = port_to_index(port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
            if i == last {
                self.send_and_wait(|tx| NxtCommand::MotorStop { port: idx, reply_tx: tx })?;
            } else {
                let (reply_tx, _rx) = mpsc::channel();
                self.tx
                    .as_ref()
                    .ok_or("Not connected")?
                    .send(NxtCommand::MotorStop { port: idx, reply_tx })
                    .map_err(|_| "NXT slot channel closed".to_string())?;
            }
        }
        Ok(())
    }

    fn rotate_ports_by_degrees(
        &mut self,
        commands: &[PortCommand],
        degrees: i32,
    ) -> Result<(), String> {
        let mut entries = Vec::with_capacity(commands.len());
        for cmd in commands {
            let idx = port_to_index(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let power = to_signed_power(cmd.direction, cmd.power);
            entries.push((idx, power, degrees.unsigned_abs()));
        }
        if entries.is_empty() {
            return Ok(());
        }
        self.send_and_wait(|tx| NxtCommand::MotorStepBatch { entries, reply_tx: tx })
    }

    fn rotate_ports_to_position(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        // Plan: read each port's current tacho, compute per-port delta, drop
        // ports that are already at target, then fire the remaining ones in
        // parallel via the batch command.
        let mut entries = Vec::with_capacity(commands.len());
        for cmd in commands {
            let idx = port_to_index(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let current = self.read_motor_count(idx)?;
            let delta = crate::adapter::rotateto_delta(current, position, cmd.direction);
            if delta == 0 {
                continue;
            }
            let power = if delta > 0 {
                to_signed_power(PortDirection::Even, cmd.power)
            } else {
                to_signed_power(PortDirection::Odd, cmd.power)
            };
            entries.push((idx, power, delta.unsigned_abs()));
        }
        if entries.is_empty() {
            return Ok(());
        }
        self.send_and_wait(|tx| NxtCommand::MotorStepBatch { entries, reply_tx: tx })
    }

    fn run_ports_for_time(
        &mut self,
        commands: &[PortCommand],
        tenths: u32,
    ) -> Result<(), String> {
        let mut entries = Vec::with_capacity(commands.len());
        for cmd in commands {
            let idx = port_to_index(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let power = to_signed_power(cmd.direction, cmd.power);
            entries.push((idx, power));
        }
        if entries.is_empty() {
            return Ok(());
        }
        self.send_and_wait(|tx| NxtCommand::MotorTimeBatch {
            entries,
            ms: tenths as u64 * 100,
            reply_tx: tx,
        })
    }
}

#[cfg(test)]
#[path = "../tests/nxt_adapter.rs"]
mod tests;
