//! LEGO Mindstorms EV3 adapter.
//!
//! Talks to the stock EV3 firmware's Direct Command protocol over USB HID,
//! Bluetooth SPP (via a pre-paired serial port), or Wi-Fi (future). All
//! transports carry the same byte stream; the adapter picks the right one
//! based on the `identifier` passed to `new()`.
//!
//! Motor control uses the PWM-power opcodes (`opOUTPUT_POWER`,
//! `opOUTPUT_STEP_POWER`, `opOUTPUT_TIME_POWER`), matching the raw-duty-cycle
//! semantics of every other BrickLogo adapter. The PID-speed opcodes exist
//! on the EV3 but we don't use them — `setpower 50` means 50% PWM, same as
//! on Powered UP and Build HAT.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use bricklogo_lang::value::LogoValue;

use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use rust_ev3::constants::{self, SensorKind};
use rust_ev3::ev3::Ev3;
use rust_ev3::serial::SerialTransport;
use rust_ev3::transport::Transport;
use rust_ev3::usb::HidTransport;
use rust_ev3::wifi::{WifiTarget, WifiTransport};

const OUTPUT_PORTS: &[&str] = &["a", "b", "c", "d"];
const INPUT_PORTS: &[&str] = &["1", "2", "3", "4"];
const MAX_POWER: u8 = 100;

// ── Port / mode helpers ──────────────────────────

fn port_to_mask(port: &str) -> Option<u8> {
    match port {
        "a" => Some(0x01),
        "b" => Some(0x02),
        "c" => Some(0x04),
        "d" => Some(0x08),
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

fn port_to_index(port: &str) -> Option<u8> {
    match port {
        "a" => Some(0),
        "b" => Some(1),
        "c" => Some(2),
        "d" => Some(3),
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

// ── Slot ─────────────────────────────────────────

type ReplyTx = mpsc::Sender<Result<(), String>>;
type SensorReplyTx = mpsc::Sender<Result<Option<LogoValue>, String>>;

enum EV3Command {
    MotorSetAndStart { mask: u8, power: i8, reply_tx: ReplyTx },
    MotorStop        { mask: u8, brake: bool, reply_tx: ReplyTx },
    MotorStepPower   { mask: u8, power: i8, degrees: i32, brake: bool, reply_tx: ReplyTx },
    MotorTimePower   { mask: u8, power: i8, ms: i32, brake: bool, reply_tx: ReplyTx },
    /// Fire per-port step-power commands without blocking between them, then
    /// poll the combined mask for completion. Each entry is (single-port mask,
    /// power, degrees) so different ports can run at different speeds and
    /// different distances (needed by `rotateto` where each port's delta differs).
    MotorStepPowerBatch { entries: Vec<(u8, i8, i32)>, brake: bool, combined_mask: u8, reply_tx: ReplyTx },
    /// Same for timed runs.
    MotorTimePowerBatch { entries: Vec<(u8, i8)>, ms: i32, brake: bool, combined_mask: u8, reply_tx: ReplyTx },
    MotorClrCount    { mask: u8, reply_tx: ReplyTx },
    ReadMotorCount   { port_index: u8, reply_tx: SensorReplyTx },
    ReadSensor       { port: u8, sensor_type: u8, mode: u8, kind: SensorKind, reply_tx: SensorReplyTx },
    GetSensorType    { port: u8, reply_tx: mpsc::Sender<Result<u8, String>> },
}

/// An in-flight `opOUTPUT_STEP_POWER` / `opOUTPUT_TIME_POWER` whose reply
/// fires only when the brick reports the port(s) are no longer busy.
struct PendingCompletion {
    mask: u8,
    reply_tx: ReplyTx,
}

struct EV3Slot {
    ev3: Ev3,
    rx: mpsc::Receiver<EV3Command>,
    alive: Arc<AtomicBool>,
    pending: Vec<PendingCompletion>,
}

impl DeviceSlot for EV3Slot {
    fn tick(&mut self) {
        // Handle any incoming commands.
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                EV3Command::MotorSetAndStart { mask, power, reply_tx } => {
                    let r = self.ev3.set_power(mask, power)
                        .and_then(|_| self.ev3.start(mask));
                    let _ = reply_tx.send(r);
                }
                EV3Command::MotorStop { mask, brake, reply_tx } => {
                    let r = self.ev3.stop(mask, brake);
                    let _ = reply_tx.send(r);
                }
                EV3Command::MotorStepPower { mask, power, degrees, brake, reply_tx } => {
                    match self.ev3.step_power(mask, power, degrees, brake) {
                        Ok(()) => self.pending.push(PendingCompletion { mask, reply_tx }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                EV3Command::MotorTimePower { mask, power, ms, brake, reply_tx } => {
                    match self.ev3.time_power(mask, power, ms, brake) {
                        Ok(()) => self.pending.push(PendingCompletion { mask, reply_tx }),
                        Err(e) => { let _ = reply_tx.send(Err(e)); }
                    }
                }
                EV3Command::MotorStepPowerBatch { entries, brake, combined_mask, reply_tx } => {
                    let mut err = None;
                    for (mask, power, degrees) in entries {
                        if let Err(e) = self.ev3.step_power(mask, power, degrees, brake) {
                            err = Some(e);
                            break;
                        }
                    }
                    match err {
                        Some(e) => { let _ = reply_tx.send(Err(e)); }
                        None => self.pending.push(PendingCompletion { mask: combined_mask, reply_tx }),
                    }
                }
                EV3Command::MotorTimePowerBatch { entries, ms, brake, combined_mask, reply_tx } => {
                    let mut err = None;
                    for (mask, power) in entries {
                        if let Err(e) = self.ev3.time_power(mask, power, ms, brake) {
                            err = Some(e);
                            break;
                        }
                    }
                    match err {
                        Some(e) => { let _ = reply_tx.send(Err(e)); }
                        None => self.pending.push(PendingCompletion { mask: combined_mask, reply_tx }),
                    }
                }
                EV3Command::MotorClrCount { mask, reply_tx } => {
                    let r = self.ev3.clr_count(mask);
                    let _ = reply_tx.send(r);
                }
                EV3Command::ReadMotorCount { port_index, reply_tx } => {
                    let r = self.ev3.get_count(port_index)
                        .map(|v| Some(LogoValue::Number(v as f64)));
                    let _ = reply_tx.send(r);
                }
                EV3Command::ReadSensor { port, sensor_type, mode, kind, reply_tx } => {
                    let r = match kind {
                        SensorKind::Pct => self.ev3.read_sensor_pct(port, sensor_type, mode)
                            .map(|v| Some(LogoValue::Number(v as f64))),
                        SensorKind::Si => self.ev3.read_sensor_si(port, sensor_type, mode)
                            .map(|v| Some(LogoValue::Number(v as f64))),
                    };
                    let _ = reply_tx.send(r);
                }
                EV3Command::GetSensorType { port, reply_tx } => {
                    let r = self.ev3.get_sensor_typemode(port).map(|(t, _m)| t);
                    let _ = reply_tx.send(r);
                }
            }
        }

        // Poll any in-flight step/time completions.
        if !self.pending.is_empty() {
            let mut still_pending = Vec::with_capacity(self.pending.len());
            for entry in self.pending.drain(..) {
                match self.ev3.test_busy(entry.mask) {
                    Ok(true) => still_pending.push(entry),
                    Ok(false) => { let _ = entry.reply_tx.send(Ok(())); }
                    Err(e) => { let _ = entry.reply_tx.send(Err(e)); }
                }
            }
            self.pending = still_pending;
        }
    }

    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

// ── Adapter ──────────────────────────────────────

pub struct EV3Adapter {
    tx: Option<mpsc::Sender<EV3Command>>,
    slot_id: Option<usize>,
    alive: Arc<AtomicBool>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    identifier: Option<String>,
    /// Cached sensor types per port index, populated lazily by the first
    /// `read_sensor` call.
    sensor_types: Arc<Mutex<HashMap<u8, u8>>>,
}

impl EV3Adapter {
    pub fn new(identifier: Option<&str>) -> Self {
        EV3Adapter {
            tx: None,
            slot_id: None,
            alive: Arc::new(AtomicBool::new(false)),
            display_name: "LEGO Mindstorms EV3".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            identifier: identifier.map(|s| s.to_string()),
            sensor_types: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Dispatch the `identifier` to the correct transport.
    ///
    ///   - `None` / `"usb"` → USB HID, first unclaimed EV3
    ///   - `"usb:<path>"`   → USB HID, that specific HID path
    ///   - `"wifi:discover"` / `"wifi:<ip>"` → Wi-Fi (currently errors)
    ///   - anything else    → Bluetooth SPP serial path
    fn open_transport(identifier: Option<&str>) -> Result<Box<dyn Transport>, String> {
        match identifier {
            None | Some("usb") => {
                let t = HidTransport::open(None)?;
                Ok(Box::new(t))
            }
            Some(ident) if ident.starts_with("usb:") => {
                let path = &ident["usb:".len()..];
                let t = HidTransport::open(Some(path))?;
                Ok(Box::new(t))
            }
            Some(ident) if ident.starts_with("wifi:") => {
                let rest = &ident["wifi:".len()..];
                let target = if rest == "discover" {
                    WifiTarget::Discover
                } else {
                    WifiTarget::Address(rest.to_string())
                };
                let _ = WifiTransport::open(target)?;
                // If we ever implement Wi-Fi, plumb the constructed
                // transport through here.
                unreachable!("WifiTransport::open always errors today")
            }
            Some(path) => {
                let t = SerialTransport::open(path)?;
                Ok(Box::new(t))
            }
        }
    }

    fn send_and_wait(&self, cmd_builder: impl FnOnce(ReplyTx) -> EV3Command) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(cmd_builder(tx))
            .map_err(|_| "EV3 slot channel closed".to_string())?;
        rx.recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| "EV3 command timed out".to_string())?
    }

    fn cached_sensor_type(&self, port_index: u8) -> Result<u8, String> {
        if let Some(&t) = self.sensor_types.lock().unwrap().get(&port_index) {
            return Ok(t);
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(EV3Command::GetSensorType { port: port_index, reply_tx: tx })
            .map_err(|_| "EV3 slot channel closed".to_string())?;
        let t = rx.recv_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| "EV3 sensor-type query timed out".to_string())??;
        if t == 0x7E || t == 0xFF {
            return Err(format!("No sensor connected on port {}", port_index + 1));
        }
        self.sensor_types.lock().unwrap().insert(port_index, t);
        Ok(t)
    }
}

impl HardwareAdapter for EV3Adapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &self.input_ports }
    fn connected(&self) -> bool { self.tx.is_some() && self.alive.load(Ordering::SeqCst) }

    fn connect(&mut self) -> Result<(), String> {
        let transport = Self::open_transport(self.identifier.as_deref())?;
        let ev3 = Ev3::new(transport);
        let (tx, rx) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        let slot = EV3Slot {
            ev3,
            rx,
            alive: alive.clone(),
            pending: Vec::new(),
        };
        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.slot_id = Some(slot_id);
        self.alive = alive;
        Ok(())
    }

    fn disconnect(&mut self) {
        // Stop everything first (brake).
        if let Some(tx) = self.tx.as_ref() {
            let (reply_tx, _) = mpsc::channel();
            let _ = tx.send(EV3Command::MotorStop {
                mask: 0x0F,
                brake: true,
                reply_tx,
            });
        }
        // Signal slot to die.
        self.alive.store(false, Ordering::SeqCst);
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
        self.sensor_types.lock().unwrap().clear();
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
        let port_index = sensor_port_index(port)
            .ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        if let Some(m) = mode {
            if m == "raw" {
                return Ok(());
            }
            // Only validate that the NAME could exist on some sensor — the
            // specific sensor may not be connected yet or may be cached
            // with a different type. read_sensor does the strict check.
            let known = matches!(
                m,
                "touch" | "light" | "ambient" | "color" | "rgb"
                    | "distance" | "angle" | "rate"
                    | "seek" | "remote" | "sound" | "temperature"
                    | "rotation"
            );
            if !known {
                return Err(format!("Unsupported sensor mode \"{}\" for EV3", m));
            }
        }
        let _ = port_index; // currently unused beyond presence check
        Ok(())
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        let mask = port_to_mask(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let signed = to_signed_power(direction, power);
        self.send_and_wait(|tx| EV3Command::MotorSetAndStart {
            mask,
            power: signed,
            reply_tx: tx,
        })
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let mask = port_to_mask(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        self.send_and_wait(|tx| EV3Command::MotorStop {
            mask,
            brake: true,
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
        let mask = port_to_mask(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let signed = to_signed_power(direction, power);
        self.send_and_wait(|tx| EV3Command::MotorTimePower {
            mask,
            power: signed,
            ms: (tenths * 100) as i32,
            brake: true,
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
        let mask = port_to_mask(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let signed = to_signed_power(direction, power);
        self.send_and_wait(|tx| EV3Command::MotorStepPower {
            mask,
            power: signed,
            degrees: degrees.abs(),
            brake: true,
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
        let mask = port_to_mask(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let port_index = port_to_index(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        // Read current encoder count.
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(EV3Command::ReadMotorCount { port_index, reply_tx: tx })
            .map_err(|_| "EV3 slot channel closed".to_string())?;
        let current = match rx.recv_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| "EV3 motor-count read timed out".to_string())??
        {
            Some(LogoValue::Number(n)) => n as i32,
            _ => 0,
        };
        // Mod-360 delta respecting direction.
        let delta = crate::adapter::rotateto_delta(current, position, direction);
        if delta == 0 {
            return Ok(());
        }
        let signed_power = if delta > 0 {
            to_signed_power(PortDirection::Even, power)
        } else {
            to_signed_power(PortDirection::Odd, power)
        };
        self.send_and_wait(|tx| EV3Command::MotorStepPower {
            mask,
            power: signed_power,
            degrees: delta.abs(),
            brake: true,
            reply_tx: tx,
        })
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        let mask = port_to_mask(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        self.send_and_wait(|tx| EV3Command::MotorClrCount { mask, reply_tx: tx })
    }

    fn rotate_to_home(
        &mut self,
        _port: &str,
        _direction: PortDirection,
        _power: u8,
    ) -> Result<(), String> {
        Err("EV3 motors have no absolute-position encoder; use rotate (relative) instead"
            .to_string())
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        // Motor position via `listento "a` + `sensor "rotation`.
        if let Some(motor_index) = port_to_index(port) {
            if mode == Some("rotation") || mode == Some("raw") {
                let (tx, rx) = mpsc::channel();
                self.tx
                    .as_ref()
                    .ok_or("Not connected")?
                    .send(EV3Command::ReadMotorCount { port_index: motor_index, reply_tx: tx })
                    .map_err(|_| "EV3 slot channel closed".to_string())?;
                return rx.recv_timeout(std::time::Duration::from_millis(500))
                    .map_err(|_| "EV3 motor-count read timed out".to_string())?;
            }
            return Err(format!("Mode \"{}\" not supported on motor ports", mode.unwrap_or("?")));
        }

        let port_index = sensor_port_index(port)
            .ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        let sensor_type = self.cached_sensor_type(port_index)?;
        if !constants::is_known_sensor(sensor_type) {
            return Err(format!("Unknown sensor type {} on port {}", sensor_type, port));
        }

        let mode_name = mode.unwrap_or("raw");
        let (mode_byte, kind) = if mode_name == "raw" {
            // "raw" means "whichever mode the sensor is currently in" —
            // default to mode 0 in percent, which works for every EV3/NXT
            // sensor we support.
            (0u8, SensorKind::Pct)
        } else {
            constants::lookup_mode(sensor_type, mode_name).ok_or_else(|| {
                format!(
                    "Sensor mode \"{}\" not supported on this device (type {})",
                    mode_name, sensor_type
                )
            })?
        };

        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(EV3Command::ReadSensor {
                port: port_index,
                sensor_type,
                mode: mode_byte,
                kind,
                reply_tx: tx,
            })
            .map_err(|_| "EV3 slot channel closed".to_string())?;
        rx.recv_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| "EV3 sensor read timed out".to_string())?
    }

    // ── Batch overrides ─────────────────────────

    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        // Group ports with the same signed power into one call so e.g.
        // `on` on `[a b c d]` with matching power becomes one mask.
        use std::collections::HashMap;
        let mut groups: HashMap<i8, u8> = HashMap::new();
        for cmd in commands {
            let mask = port_to_mask(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let signed = to_signed_power(cmd.direction, cmd.power);
            *groups.entry(signed).or_insert(0) |= mask;
        }
        for (power, mask) in groups {
            self.send_and_wait(|tx| EV3Command::MotorSetAndStart { mask, power, reply_tx: tx })?;
        }
        Ok(())
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        let mut mask = 0u8;
        for p in ports {
            mask |= port_to_mask(p)
                .ok_or_else(|| format!("Unknown output port \"{}\"", p))?;
        }
        self.send_and_wait(|tx| EV3Command::MotorStop { mask, brake: true, reply_tx: tx })
    }

    fn rotate_ports_by_degrees(
        &mut self,
        commands: &[PortCommand],
        degrees: i32,
    ) -> Result<(), String> {
        let mut entries = Vec::with_capacity(commands.len());
        let mut combined_mask = 0u8;
        for cmd in commands {
            let mask = port_to_mask(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let power = to_signed_power(cmd.direction, cmd.power);
            entries.push((mask, power, degrees.abs()));
            combined_mask |= mask;
        }
        self.send_and_wait(|tx| EV3Command::MotorStepPowerBatch {
            entries,
            brake: true,
            combined_mask,
            reply_tx: tx,
        })
    }

    fn rotate_ports_to_position(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        // Read each port's encoder, compute per-port mod-360 delta, fire all
        // in parallel via the batch command.
        let mut entries = Vec::with_capacity(commands.len());
        let mut combined_mask = 0u8;
        for cmd in commands {
            let mask = port_to_mask(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let port_index = port_to_index(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let (tx, rx) = mpsc::channel();
            self.tx
                .as_ref()
                .ok_or("Not connected")?
                .send(EV3Command::ReadMotorCount { port_index, reply_tx: tx })
                .map_err(|_| "EV3 slot channel closed".to_string())?;
            let current = match rx.recv_timeout(std::time::Duration::from_millis(500))
                .map_err(|_| "EV3 motor-count read timed out".to_string())??
            {
                Some(LogoValue::Number(n)) => n as i32,
                _ => 0,
            };
            let delta = crate::adapter::rotateto_delta(current, position, cmd.direction);
            if delta == 0 {
                continue;
            }
            let power = if delta > 0 {
                to_signed_power(PortDirection::Even, cmd.power)
            } else {
                to_signed_power(PortDirection::Odd, cmd.power)
            };
            entries.push((mask, power, delta.abs()));
            combined_mask |= mask;
        }
        if entries.is_empty() {
            return Ok(());
        }
        self.send_and_wait(|tx| EV3Command::MotorStepPowerBatch {
            entries,
            brake: true,
            combined_mask,
            reply_tx: tx,
        })
    }

    fn run_ports_for_time(
        &mut self,
        commands: &[PortCommand],
        tenths: u32,
    ) -> Result<(), String> {
        let mut entries = Vec::with_capacity(commands.len());
        let mut combined_mask = 0u8;
        for cmd in commands {
            let mask = port_to_mask(cmd.port)
                .ok_or_else(|| format!("Unknown output port \"{}\"", cmd.port))?;
            let power = to_signed_power(cmd.direction, cmd.power);
            entries.push((mask, power));
            combined_mask |= mask;
        }
        self.send_and_wait(|tx| EV3Command::MotorTimePowerBatch {
            entries,
            ms: (tenths * 100) as i32,
            brake: true,
            combined_mask,
            reply_tx: tx,
        })
    }
}

#[cfg(test)]
#[path = "../tests/ev3_adapter.rs"]
mod tests;
