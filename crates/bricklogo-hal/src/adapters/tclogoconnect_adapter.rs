use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bricklogo_lang::value::LogoValue;
use rust_tclogoconnect::constants::{INPUT_BIT_PORT6, INPUT_BIT_PORT7};
use rust_tclogoconnect::device::Device;

use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};

const OUTPUT_PORTS: &[&str] = &["0", "1", "2", "3", "4", "5", "a", "b", "c"];
const INPUT_PORTS: &[&str] = &["6", "7"];
const MAX_POWER: u8 = 255;

fn output_index(port: &str) -> Option<usize> {
    port.parse::<usize>().ok().filter(|&i| i < 6)
}

// TC Logo motor pair ports "a"/"b"/"c" mirror Interface A: each maps to a
// pair of direct outputs (0+1, 2+3, 4+5); direction picks which half of the
// pair carries the duty, the other stays at zero.
fn pair_indices(port: &str) -> Option<(usize, usize)> {
    match port {
        "a" => Some((0, 1)),
        "b" => Some((2, 3)),
        "c" => Some((4, 5)),
        _ => None,
    }
}

fn input_index(port: &str) -> Option<usize> {
    match port {
        "6" => Some(0),
        "7" => Some(1),
        _ => None,
    }
}

fn entries_for(port: &str, direction: PortDirection, power: u8) -> Result<Vec<(usize, u8)>, String> {
    if let Some((even_idx, odd_idx)) = pair_indices(port) {
        let (d_even, d_odd) = match direction {
            PortDirection::Even => (power, 0),
            PortDirection::Odd => (0, power),
        };
        Ok(vec![(even_idx, d_even), (odd_idx, d_odd)])
    } else {
        let idx = output_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        Ok(vec![(idx, power)])
    }
}

fn zero_entries_for(port: &str) -> Result<Vec<(usize, u8)>, String> {
    if let Some((even_idx, odd_idx)) = pair_indices(port) {
        Ok(vec![(even_idx, 0), (odd_idx, 0)])
    } else {
        let idx = output_index(port).ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        Ok(vec![(idx, 0)])
    }
}

/// Turn `bit` on for the leading `on_slots` (0..=8) of each 8-slot half of
/// `frames` — contiguous from the start of the half, off for the rest. See
/// `duty_to_frames` for why contiguous instead of spread out.
fn set_leading_edge(on_slots: u32, frames: &mut [u8; 16], bit: u8) {
    let on_slots = (on_slots as usize).min(8);
    for half in [0usize, 8] {
        for slot in &mut frames[half..half + on_slots] {
            *slot |= bit;
        }
    }
}

/// Pure duty (0-255 per output) -> 16 one-millisecond `D`-frame masks: two
/// identical 8-slot **leading-edge** PWM periods back to back (8ms period =
/// 125 Hz), each output on for its first `on_slots` of every 8 and off for
/// the rest.
///
/// Contiguous on-time, not spread evenly across the 16 slots, because a
/// LEGO motor's coil has an electrical time constant longer than a single
/// 1ms pulse — chopping duty into alternating 1ms on/off slots (what an
/// even spread does at 50%) is effectively a 500Hz+ chop whose pulses are
/// too short for current to build up, so torque collapses even though the
/// average power delivered is correct. The BrickInterface firmware drives
/// the same physical outputs with exactly this shape natively (a 255-tick
/// counter, output held on while `counter < duty`, ~100Hz) — matching it
/// here is what makes `setpower` feel identical on both bridges.
///
/// 8 slots (125Hz) rather than one 16-slot period (62.5Hz): 62.5Hz is slow
/// enough that an LED or incandescent output would visibly flicker; 125Hz
/// is close to BrickInterface's ~100Hz while staying comfortably above the
/// flicker threshold.
///
/// `on_slots = round(8 * duty / 255)`, clamped to at least 1 whenever
/// `duty > 0` so a small nonzero `setpower` still pulses instead of
/// silently rounding down to fully off.
fn duty_to_frames(duty: &[u8; 6]) -> [u8; 16] {
    let mut frames = [0u8; 16];
    for (i, &d) in duty.iter().enumerate() {
        if d == 0 {
            continue;
        }
        let on_slots = ((d as u32 * 8 + 127) / 255).max(1);
        set_leading_edge(on_slots, &mut frames, 1 << i);
    }
    frames
}

fn mask_from_duty(duty: &[u8; 6]) -> u8 {
    let mut mask = 0u8;
    for (i, &d) in duty.iter().enumerate() {
        if d == 255 {
            mask |= 1 << i;
        }
    }
    mask
}

/// True when every output is fully off or fully on — no host-side PWM
/// needed, a single static `D` frame covers it.
fn is_static(duty: &[u8; 6]) -> bool {
    duty.iter().all(|&d| d == 0 || d == 255)
}

// ── Shared state ─────────────────────────────────

/// Input state updated by the slot every tick and read synchronously by the
/// adapter. The device is push-driven (unsolicited `Ixx` reports) rather
/// than polled, so there is nothing to request on a `read_sensor` call —
/// the slot is always the freshest source of truth.
struct TcState {
    last_input: Mutex<u8>,
    edge_counts: [AtomicU32; 2],
}

impl Default for TcState {
    fn default() -> Self {
        TcState {
            last_input: Mutex::new(0),
            edge_counts: [AtomicU32::new(0), AtomicU32::new(0)],
        }
    }
}

// ── Driver slot ─────────────────────────────────

type ReplyTx = Option<mpsc::Sender<()>>;

struct SetOutputs {
    entries: Vec<(usize, u8)>,
    reply_tx: ReplyTx,
}

struct TcSlot {
    device: Device,
    rx: mpsc::Receiver<SetOutputs>,
    alive: bool,
    duty: [u8; 6],
    state: Arc<TcState>,
}

impl DeviceSlot for TcSlot {
    fn tick(&mut self) {
        let mut replies = Vec::new();
        while let Ok(cmd) = self.rx.try_recv() {
            for (idx, value) in cmd.entries {
                self.duty[idx] = value;
            }
            if let Some(tx) = cmd.reply_tx {
                replies.push(tx);
            }
        }

        // Every tick transmits something, even with nothing new to say —
        // this is what feeds the device's 500ms watchdog. A static mask
        // covers "every output fully off or fully on"; anything in between
        // streams a 16-frame PWM batch in one write (~960fps, safely under
        // the 1kHz replay rate the device's queue can sustain).
        if is_static(&self.duty) {
            let _ = self.device.set_outputs(mask_from_duty(&self.duty));
        } else {
            let frames = duty_to_frames(&self.duty);
            let _ = self.device.write_batch(&frames);
        }

        // The device only sends `Ixx` on a state change, so a press and a
        // release can both arrive within one tick's drain (a fast tap
        // inside ~16ms). Walk every mask in arrival order and count an
        // edge per transition — comparing only the tick's final mask
        // against the previous tick's would let a press+release that
        // cancels out within one drain vanish without ever being counted.
        if let Ok(masks) = self.device.read_pending_input() {
            if !masks.is_empty() {
                let mut last = self.state.last_input.lock().unwrap();
                for mask in masks {
                    for (i, bit) in [INPUT_BIT_PORT6, INPUT_BIT_PORT7].into_iter().enumerate() {
                        let was_pressed = *last & bit != 0;
                        let now_pressed = mask & bit != 0;
                        if !was_pressed && now_pressed {
                            self.state.edge_counts[i].fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    *last = mask;
                }
            }
        }

        for tx in replies {
            let _ = tx.send(());
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

// ── Adapter ─────────────────────────────────────

pub struct TcLogoConnectAdapter {
    tx: Option<mpsc::Sender<SetOutputs>>,
    slot_id: Option<usize>,
    display_name: String,
    output_ports: Vec<String>,
    input_ports: Vec<String>,
    serial_path: String,
    state: Arc<TcState>,
}

impl TcLogoConnectAdapter {
    pub fn new(serial_path: &str) -> Self {
        TcLogoConnectAdapter {
            tx: None,
            slot_id: None,
            display_name: "TC Logo Connect".to_string(),
            output_ports: OUTPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            input_ports: INPUT_PORTS.iter().map(|s| s.to_string()).collect(),
            serial_path: serial_path.to_string(),
            state: Arc::new(TcState::default()),
        }
    }

    /// Send a duty update and block until the slot has applied it and
    /// transmitted the result on its next tick — gives callers (and tests)
    /// a deterministic point to observe the effect instead of racing the
    /// scheduler thread.
    fn send_and_wait(&self, entries: Vec<(usize, u8)>) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(SetOutputs { entries, reply_tx: Some(tx) })
            .map_err(|_| "Send failed".to_string())?;
        rx.recv_timeout(Duration::from_millis(2000))
            .map_err(|_| "Command timed out".to_string())
    }
}

impl HardwareAdapter for TcLogoConnectAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &self.output_ports }
    fn input_ports(&self) -> &[String] { &self.input_ports }
    fn connected(&self) -> bool { self.tx.is_some() }
    fn max_power(&self) -> u8 { MAX_POWER }

    fn connect(&mut self) -> Result<(), String> {
        let device = Device::open(&self.serial_path)?;
        let state = Arc::new(TcState::default());
        let (tx, rx) = mpsc::channel();
        let slot = TcSlot { device, rx, alive: true, duty: [0; 6], state: state.clone() };
        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.slot_id = Some(slot_id);
        self.state = state;
        Ok(())
    }

    fn disconnect(&mut self) {
        if self.tx.is_some() {
            let _ = self.send_and_wait(vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)]);
        }
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        if output_index(port).is_some() || matches!(port, "a" | "b" | "c") { Ok(()) }
        else { Err(format!("Unknown output port \"{}\"", port)) }
    }

    fn validate_sensor_port(&self, port: &str, _mode: Option<&str>) -> Result<(), String> {
        if INPUT_PORTS.contains(&port) { Ok(()) }
        else { Err(format!("Unknown sensor port \"{}\"", port)) }
    }

    fn start_port(&mut self, port: &str, direction: PortDirection, power: u8) -> Result<(), String> {
        self.send_and_wait(entries_for(port, direction, power)?)
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        self.send_and_wait(zero_entries_for(port)?)
    }

    // Coalesce every selected output into one duty update, so e.g.
    // `talkto [0 1] on` reaches the slot as a single message rather than
    // two separate tick round-trips.
    fn start_ports(&mut self, commands: &[PortCommand]) -> Result<(), String> {
        if commands.is_empty() {
            return Ok(());
        }
        let mut entries = Vec::new();
        for cmd in commands {
            entries.extend(entries_for(cmd.port, cmd.direction, cmd.power)?);
        }
        self.send_and_wait(entries)
    }

    fn stop_ports(&mut self, ports: &[&str]) -> Result<(), String> {
        if ports.is_empty() {
            return Ok(());
        }
        let mut entries = Vec::new();
        for port in ports {
            entries.extend(zero_entries_for(port)?);
        }
        self.send_and_wait(entries)
    }

    fn run_port_for_time(&mut self, port: &str, direction: PortDirection, power: u8, tenths: u32) -> Result<(), String> {
        self.start_port(port, direction, power)?;
        std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
        self.stop_port(port)
    }

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        self.start_ports(commands)?;
        std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
        let ports: Vec<&str> = commands.iter().map(|c| c.port).collect();
        self.stop_ports(&ports)
    }

    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("TC Logo Connect does not support rotation by degrees".to_string())
    }

    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("TC Logo Connect does not support rotation to position".to_string())
    }

    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("TC Logo Connect does not support position reset".to_string())
    }

    fn rotate_to_abs(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("TC Logo Connect does not support absolute positioning".to_string())
    }

    fn read_sensor(&mut self, port: &str, _mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let idx = input_index(port).ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        let bit = if idx == 0 { INPUT_BIT_PORT6 } else { INPUT_BIT_PORT7 };
        let mask = *self.state.last_input.lock().unwrap();
        // Pressed reads "true" — the cross-device convention shared by
        // Interface A, Control Lab, RCX, and Coral.
        let pressed = mask & bit != 0;
        Ok(Some(LogoValue::Word(if pressed { "true" } else { "false" }.to_string())))
    }

    fn read_counter(&mut self, port: &str) -> Result<u32, String> {
        let idx = input_index(port).ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        Ok(self.state.edge_counts[idx].load(Ordering::SeqCst))
    }

    fn reset_counter(&mut self, port: &str) -> Result<(), String> {
        let idx = input_index(port).ok_or_else(|| format!("Unknown sensor port \"{}\"", port))?;
        self.state.edge_counts[idx].store(0, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/tclogoconnect_adapter.rs"]
mod tests;
