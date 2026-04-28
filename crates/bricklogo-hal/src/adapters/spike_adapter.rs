//! LEGO SPIKE Prime / Robot Inventor adapter.
//!
//! Talks to the hub using the Atlantis binary protocol (COBS-framed, XOR
//! stuffed). Over both USB CDC and BLE the hub accepts the same frames — the
//! `0x02` delimiter lets the firmware distinguish Atlantis from REPL traffic
//! on USB.
//!
//! BrickLogo uploads a small MicroPython "agent" program once and leaves it
//! running. All motor and sensor commands after that are binary opcodes
//! carried in Atlantis `TunnelMessage` payloads. One code path serves both
//! transports.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bricklogo_lang::value::LogoValue;

use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use rust_spike::agent::{AGENT_SOURCE, agent_crc32};
use rust_spike::atlantis::{self, Message, RunningCrc};
use rust_spike::cobs;
use rust_spike::constants::{
    PORT_COUNT, is_absolute_motor, is_led, is_motor,
    port_index as port_index_validate, port_letter,
};
use rust_spike::protocol::{self, Event, Reply};

/// Sentinel meaning "device type unknown" — used both before the connect-time
/// snapshot resolves and as a fallback when the agent can't introspect a port.
/// `validate_output_port` treats this as permissive (defer to firmware) so
/// firmware quirks don't lock users out of working hardware.
const PORT_TYPE_UNKNOWN: u16 = 0xFFFF;

use super::spike_ble_transport::SpikeBleTransport;

const MAX_POWER: u8 = 100;
const AGENT_FILENAME: &str = "program.py";
const AGENT_SLOT: u8 = 0;
const READY_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const CHUNK_SIZE: usize = 512;
/// The agent sends `{"op":"heartbeat"}` every 2s. If no heartbeat arrives
/// for this long, assume the user stopped the program on the hub and tear
/// down the slot so subsequent commands fail fast.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);

// ── Transport trait ─────────────────────────────

/// Byte-level transport used by the adapter and the firmware uploader.
/// The real trait is defined in `rust-spike` so the uploader can operate
/// on a `&mut dyn Transport` without a dependency on `bricklogo-hal`.
pub use rust_spike::transport::Transport as SpikeTransport;

pub struct SpikeSerialTransport {
    port: Box<dyn serialport::SerialPort>,
}

impl SpikeSerialTransport {
    pub fn open(path: &str) -> Result<Self, String> {
        let port = serialport::new(path, 115200)
            .timeout(Duration::from_millis(50))
            .open()
            .map_err(|e| format!("Could not open {}: {}", path, e))?;
        Ok(SpikeSerialTransport { port })
    }
}

impl SpikeTransport for SpikeSerialTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        match Read::read(self.port.as_mut(), buf) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(e.to_string()),
        }
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        Write::write_all(self.port.as_mut(), data).map_err(|e| e.to_string())
    }
    fn flush(&mut self) -> Result<(), String> {
        Write::flush(self.port.as_mut()).map_err(|e| e.to_string())
    }
}

// ── Frame extractor ─────────────────────────────

/// Buffers incoming bytes and yields whole COBS frames (split on `0x02`).
/// XOR unstuffing and COBS decoding happen in the caller via `cobs::unpack`.
struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    fn new() -> Self { FrameReader { buf: Vec::with_capacity(1024) } }

    fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Pop the next complete frame (bytes up to and including the next `0x02`).
    /// Returns the frame body WITHOUT the trailing delimiter, or None if no
    /// complete frame is buffered yet.
    fn next_frame(&mut self) -> Option<Vec<u8>> {
        let pos = self.buf.iter().position(|&b| b == cobs::END_FRAME)?;
        let frame: Vec<u8> = self.buf.drain(..=pos).collect();
        // Strip the trailing 0x02
        Some(frame[..frame.len() - 1].to_vec())
    }
}

// ── Helpers ─────────────────────────────────────

fn to_velocity_i16(direction: PortDirection, power: u8) -> i16 {
    let vel = (power.min(MAX_POWER) as i16) * 10;
    match direction {
        PortDirection::Even => vel,
        PortDirection::Odd => -vel,
    }
}

/// Convert (direction, power) to the signed PWM value the firmware accepts on
/// `port.X.pwm()` (-100..100). Used for non-tacho devices (LEDs, passive
/// motors) that can't do PID speed control.
fn to_pwm_i8(direction: PortDirection, power: u8) -> i8 {
    let p = power.min(MAX_POWER).min(100) as i8;
    match direction {
        PortDirection::Even => p,
        PortDirection::Odd => -p,
    }
}

fn direction_code(direction: PortDirection) -> u8 {
    match direction {
        PortDirection::Even => 0, // CLOCKWISE
        PortDirection::Odd => 1,  // COUNTERCLOCKWISE
    }
}

// ── Connect-time bootstrap helpers ──────────────
//
// These run synchronously on the connecting thread, driving the transport
// directly (no scheduler slot yet). Once the agent is up and replies "ready",
// the transport moves into a SpikeSlot that handles steady-state traffic.

fn write_frame(transport: &mut dyn SpikeTransport, message: &[u8]) -> Result<(), String> {
    let framed = cobs::pack(message);
    transport.write_all(&framed)?;
    transport.flush()
}

fn read_frame(
    transport: &mut dyn SpikeTransport,
    framer: &mut FrameReader,
    deadline: Instant,
) -> Result<Vec<u8>, String> {
    let mut buf = [0u8; 1024];
    loop {
        if let Some(frame) = framer.next_frame() {
            return cobs::unpack(&frame);
        }
        if Instant::now() >= deadline {
            return Err("Atlantis read timeout".to_string());
        }
        match transport.read(&mut buf)? {
            0 => std::thread::sleep(Duration::from_millis(10)),
            n => framer.feed(&buf[..n]),
        }
    }
}

/// Read frames until a matching predicate fires. Frames not matching the
/// predicate are either retained (caller owns what to do) or discarded —
/// here we discard them. Only used during bootstrap, so we don't lose
/// reply-bearing traffic.
fn read_until<F>(
    transport: &mut dyn SpikeTransport,
    framer: &mut FrameReader,
    timeout: Duration,
    mut predicate: F,
) -> Result<Message, String>
where
    F: FnMut(&Message) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let bytes = read_frame(transport, framer, deadline)?;
        let msg = atlantis::parse(&bytes)?;
        if predicate(&msg) {
            return Ok(msg);
        }
    }
}

/// Upload the agent program, replacing whatever's in slot 0. Always performs
/// a fresh upload — a CRC check requires a file-download round-trip that not
/// all firmware revisions support, and the upload itself is only a few KB.
fn upload_agent(
    transport: &mut dyn SpikeTransport,
    framer: &mut FrameReader,
) -> Result<(), String> {
    let source = AGENT_SOURCE.as_bytes();
    let total_crc = agent_crc32();

    // Stop anything currently running in the slot so it can't race with our
    // upload. Any response / flow notification emitted by the old program's
    // shutdown is drained along with the reply.
    let _ = write_frame(transport, &atlantis::program_flow_request(true, AGENT_SLOT));
    let _ = read_until(transport, framer, Duration::from_secs(2), |m| {
        matches!(m, Message::ProgramFlowResponse { .. })
    });

    // Clear the slot first.
    write_frame(transport, &atlantis::clear_slot_request(AGENT_SLOT))?;
    let _ = read_until(transport, framer, Duration::from_secs(5), |m| {
        matches!(m, Message::ClearSlotResponse { .. })
    })?;

    // Begin upload.
    write_frame(
        transport,
        &atlantis::start_file_upload_request(AGENT_FILENAME, AGENT_SLOT, total_crc),
    )?;
    match read_until(transport, framer, Duration::from_secs(5), |m| {
        matches!(m, Message::StartFileUploadResponse { .. })
    })? {
        Message::StartFileUploadResponse { success: true } => {}
        Message::StartFileUploadResponse { success: false } => {
            return Err("Hub refused StartFileUpload".to_string());
        }
        _ => unreachable!(),
    }

    // Stream chunks.
    let mut running = RunningCrc::new();
    for chunk in source.chunks(CHUNK_SIZE) {
        running.update(chunk);
        write_frame(
            transport,
            &atlantis::transfer_chunk_request(running.finalize(), chunk),
        )?;
        match read_until(transport, framer, Duration::from_secs(5), |m| {
            matches!(m, Message::TransferChunkResponse { .. })
        })? {
            Message::TransferChunkResponse { success: true } => {}
            Message::TransferChunkResponse { success: false } => {
                return Err("Hub rejected agent chunk".to_string());
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn start_agent_and_wait_ready(
    transport: &mut dyn SpikeTransport,
    framer: &mut FrameReader,
) -> Result<(), String> {
    write_frame(
        transport,
        &atlantis::program_flow_request(false, AGENT_SLOT),
    )?;
    // Expect ProgramFlowResponse then eventually a TunnelMessage with {"op":"ready"}.
    let _ = read_until(transport, framer, Duration::from_secs(5), |m| {
        matches!(m, Message::ProgramFlowResponse { .. })
    });
    let deadline = Instant::now() + READY_TIMEOUT;
    let mut last_error: Option<String> = None;
    loop {
        let bytes = read_frame(transport, framer, deadline).map_err(|e| {
            match last_error.take() {
                Some(err) => format!("{} (last hub message: {})", e, err),
                None => e,
            }
        })?;
        let msg = atlantis::parse(&bytes)?;
        debug_spike(&format!("start: {:?}", msg));
        match msg {
            Message::Tunnel { payload } => {
                if matches!(protocol::parse_event(&payload), Ok(Event::Ready)) {
                    return Ok(());
                }
            }
            Message::ConsoleNotification { text } => {
                last_error = Some(text.clone());
                debug_spike(&format!("console: {}", text));
            }
            // Stop notifications during bootstrap are almost always the
            // *previous* agent dying (ClearSlot on a running program emits
            // one). Ignore them — if the new agent truly fails to come
            // up, the 5 s ready timeout catches it.
            Message::ProgramFlowNotification { .. } => {}
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Agent did not signal ready within 5s{}",
                last_error
                    .as_deref()
                    .map(|s| format!(" — hub console: {}", s))
                    .unwrap_or_default()
            ));
        }
    }
}

/// Print to stderr when `BRICKLOGO_DEBUG_SPIKE` is set. Low-noise diagnostic
/// channel for hardware debugging without changing the TUI output.
fn debug_spike(msg: &str) {
    if std::env::var("BRICKLOGO_DEBUG_SPIKE").is_ok() {
        eprintln!("[spike] {}", msg);
    }
}


// ── Slot commands / slot ────────────────────────

type ReplyTx = mpsc::Sender<Result<Reply, String>>;

enum SpikeCommand {
    Send { payload: Vec<u8>, rid: u16, reply_tx: ReplyTx },
    /// Write a `ProgramFlowRequest(stop, slot=0)` Atlantis frame directly
    /// and then let the slot die. Used during `disconnect` so the hub's
    /// running agent is cleanly terminated even if the agent is
    /// unresponsive.
    StopAgent { ack_tx: mpsc::Sender<()> },
}

struct SpikeSlot {
    transport: Box<dyn SpikeTransport>,
    rx: mpsc::Receiver<SpikeCommand>,
    framer: FrameReader,
    pending: HashMap<u16, ReplyTx>,
    alive: bool,
    /// Shared with the adapter so `connected()` reflects slot death.
    alive_flag: Arc<AtomicBool>,
    /// Last time a tunnel message (of any kind) arrived from the agent.
    last_heartbeat: Instant,
    /// Cached LPF2 device type per port. Updated on agent push events; read
    /// by the adapter's `validate_output_port`.
    port_types: Arc<Mutex<[u16; PORT_COUNT]>>,
}

impl DeviceSlot for SpikeSlot {
    fn tick(&mut self) {
        // Drain incoming bytes.
        let mut buf = [0u8; 1024];
        match self.transport.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => self.framer.feed(&buf[..n]),
            Err(_) => {
                self.mark_dead("transport read error");
                return;
            }
        }

        // Heartbeat watchdog.
        if self.alive && self.last_heartbeat.elapsed() > HEARTBEAT_TIMEOUT {
            self.mark_dead("agent heartbeat lost");
            return;
        }

        // Parse any complete frames.
        while let Some(frame) = self.framer.next_frame() {
            let decoded = match cobs::unpack(&frame) {
                Ok(d) => d,
                Err(_) => continue, // Noise (e.g. leftover REPL) — skip.
            };
            let msg = match atlantis::parse(&decoded) {
                Ok(m) => m,
                Err(_) => continue,
            };
            debug_spike(&format!("rx: {:?}", msg));
            match msg {
                Message::Tunnel { payload } => {
                    self.last_heartbeat = Instant::now();
                    self.route_tunnel_payload(&payload);
                }
                Message::ProgramFlowNotification { stop: true } => {
                    self.mark_dead("agent exited");
                }
                _ => {} // ignore console/info/etc
            }
        }

        // Drain outgoing command queue.
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                SpikeCommand::Send { payload, rid, reply_tx } => {
                    debug_spike(&format!("tx rid={} payload={:?}", rid, payload));
                    let message = atlantis::tunnel_message(&payload);
                    let framed = cobs::pack(&message);
                    match self
                        .transport
                        .write_all(&framed)
                        .and_then(|_| self.transport.flush())
                    {
                        Ok(()) => {
                            self.pending.insert(rid, reply_tx);
                        }
                        Err(e) => {
                            let _ = reply_tx.send(Err(e));
                        }
                    }
                }
                SpikeCommand::StopAgent { ack_tx } => {
                    let frame = cobs::pack(&atlantis::program_flow_request(true, AGENT_SLOT));
                    let _ = self
                        .transport
                        .write_all(&frame)
                        .and_then(|_| self.transport.flush());
                    let _ = ack_tx.send(());
                    self.mark_dead("disconnect requested");
                    return;
                }
            }
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

impl SpikeSlot {
    fn route_tunnel_payload(&mut self, data: &[u8]) {
        // Each tunnel message from the agent is one binary event.
        let event = match protocol::parse_event(data) {
            Ok(e) => e,
            Err(_) => return,
        };
        match event {
            Event::Reply { rid, reply } => {
                if let Some(tx) = self.pending.remove(&rid) {
                    let result: Result<Reply, String> = match reply {
                        Reply::Error(msg) => Err(msg),
                        other => Ok(other),
                    };
                    let _ = tx.send(result);
                }
            }
            Event::PortEvent { port, type_id } => {
                debug_spike(&format!("port_event: port={} type=0x{:04x}", port, type_id));
                if (port as usize) < PORT_COUNT {
                    self.port_types.lock().unwrap()[port as usize] = type_id;
                }
            }
            Event::Ready | Event::Heartbeat => {}
        }
    }

    fn fail_pending(&mut self, reason: &str) {
        for (_, tx) in self.pending.drain() {
            let result: Result<Reply, String> = Err(reason.to_string());
            let _ = tx.send(result);
        }
    }

    fn mark_dead(&mut self, reason: &str) {
        self.alive = false;
        self.alive_flag.store(false, Ordering::SeqCst);
        self.fail_pending(reason);
    }
}

// ── Discovery ───────────────────────────────────

const SPIKE_USB_VID: u16 = 0x0694;

/// Returns `Ok(Some(transport))` on success, `Ok(None)` if no SPIKE hub is
/// attached, or `Err` if hubs were found but none could be opened (so the
/// caller can surface a helpful message instead of falling through to BLE).
fn find_usb_transport() -> Result<Option<Box<dyn SpikeTransport>>, String> {
    let ports = serialport::available_ports()
        .map_err(|e| format!("USB port enumeration failed: {}", e))?;
    let mut saw_candidate = false;
    let mut last_err: Option<String> = None;
    for p in ports {
        if let serialport::SerialPortType::UsbPort(ref info) = p.port_type {
            if info.vid != SPIKE_USB_VID {
                continue;
            }
            saw_candidate = true;
            match SpikeSerialTransport::open(&p.port_name) {
                Ok(t) => return Ok(Some(Box::new(t))),
                Err(e) => last_err = Some(format!("{}: {}", p.port_name, e)),
            }
        }
    }
    if saw_candidate {
        Err(format!(
            "Found SPIKE Prime on USB but could not open it ({})",
            last_err.unwrap_or_else(|| "unknown error".to_string())
        ))
    } else {
        Ok(None)
    }
}

fn find_ble_transport() -> Result<Option<Box<dyn SpikeTransport>>, String> {
    match SpikeBleTransport::scan_and_connect(Duration::from_secs(10)) {
        Ok(Some(t)) => Ok(Some(Box::new(t))),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

// ── Adapter ─────────────────────────────────────

pub struct SpikeAdapter {
    tx: Option<mpsc::Sender<SpikeCommand>>,
    slot_id: Option<usize>,
    next_rid: u16,
    display_name: String,
    identifier: Option<String>,
    /// Set to `false` by the slot when the agent heartbeat is lost or the
    /// transport fails. Gated behind `connected()` so the REPL reports the
    /// disconnect accurately.
    alive: Arc<AtomicBool>,
    /// LPF2 device type per port — cloned into the slot, populated by the
    /// connect-time snapshot and refreshed by agent push events.
    port_types: Arc<Mutex<[u16; PORT_COUNT]>>,
}

impl SpikeAdapter {
    pub fn new(identifier: Option<&str>) -> Self {
        SpikeAdapter {
            tx: None,
            slot_id: None,
            next_rid: 1,
            display_name: "LEGO SPIKE Prime".to_string(),
            identifier: identifier.map(|s| s.to_string()),
            alive: Arc::new(AtomicBool::new(false)),
            port_types: Arc::new(Mutex::new([PORT_TYPE_UNKNOWN; PORT_COUNT])),
        }
    }

    /// Allocate the next request id and return `(rid, reply_rx)`. The caller
    /// builds the opcode payload with `rid` baked in, then forwards it to
    /// the slot via the tx channel.
    fn begin_request(&mut self) -> Result<(u16, mpsc::Receiver<Result<Reply, String>>, mpsc::Sender<Result<Reply, String>>), String> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err("Hub disconnected (agent heartbeat lost)".to_string());
        }
        if self.tx.is_none() {
            return Err("Not connected".to_string());
        }
        let rid = self.next_rid;
        // rid 0 is reserved for "uninitialised" callers; skip it on wrap.
        self.next_rid = self.next_rid.wrapping_add(1);
        if self.next_rid == 0 {
            self.next_rid = 1;
        }
        let (reply_tx, reply_rx) = mpsc::channel();
        Ok((rid, reply_rx, reply_tx))
    }

    fn dispatch(
        &self,
        payload: Vec<u8>,
        rid: u16,
        reply_tx: mpsc::Sender<Result<Reply, String>>,
        reply_rx: mpsc::Receiver<Result<Reply, String>>,
    ) -> Result<Reply, String> {
        let tx = self.tx.as_ref().ok_or("Not connected")?;
        tx.send(SpikeCommand::Send { payload, rid, reply_tx })
            .map_err(|_| "SPIKE slot channel closed".to_string())?;
        reply_rx
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|_| "SPIKE command timed out".to_string())?
    }

    /// Issue a command that expects an `Ok` reply (void).
    fn send_void<F>(&mut self, build: F) -> Result<(), String>
    where
        F: FnOnce(u16) -> Result<Vec<u8>, String>,
    {
        let (rid, reply_rx, reply_tx) = self.begin_request()?;
        let payload = build(rid)?;
        match self.dispatch(payload, rid, reply_tx, reply_rx)? {
            Reply::Ok => Ok(()),
            other => Err(format!("unexpected reply: {:?}", other)),
        }
    }

    /// Read the cached LPF2 type for a port and return true only if it's a
    /// motor SPIKE 3's `motor` module will accept — that's specifically the
    /// absolute-position motors (SPIKE-family angular: medium / large /
    /// small / grey variants). Tacho-only devices (medium linear actuator,
    /// Boost large, Technic XL) ENODEV via `motor.run`, so they take the
    /// PWM dispatch path. Unknown letters and unknown types also return
    /// false.
    fn port_is_pid_capable(&self, port: &str) -> bool {
        match port_index_validate(port) {
            Some(idx) => is_absolute_motor(self.port_types.lock().unwrap()[idx]),
            None => false,
        }
    }

    /// Gate for all rotate / reset / position operations. Empirically, on
    /// SPIKE 3 firmware (Hub OS 3.4.0), `motor.run_for_degrees`,
    /// `motor.run_to_absolute_position`, `motor.reset_relative_position`
    /// and `motor.run` all ENODEV on motors without an absolute encoder
    /// (passive, lights, tacho-only types like the medium linear actuator).
    /// `motor.run_for_time` is the only motor-module function that works
    /// on tacho-only types, but we already cover timed-run via the default
    /// PWM+sleep path on the host so we don't need it.
    ///
    /// Difference from PUP: PUP's LWP3 firmware accepts rotate-by-degrees
    /// on any tacho motor (Boost large 46, Technic XL 47, medium linear
    /// 38). SPIKE 3 firmware is stricter — only absolute motors. Surface
    /// it cleanly so users know to switch hubs for non-SPIKE motors.
    fn require_absolute_motor(&self, port: &str) -> Result<(), String> {
        let idx = port_index_validate(port)
            .ok_or_else(|| format!("Unknown port \"{}\"", port))?;
        let type_id = self.port_types.lock().unwrap()[idx];
        if type_id == PORT_TYPE_UNKNOWN {
            return Ok(()); // permissive when snapshot didn't resolve
        }
        if type_id == 0 {
            return Err(format!("No device on port \"{}\"", port_letter(idx)));
        }
        if !is_absolute_motor(type_id) {
            return Err(format!(
                "Motor on port \"{}\" has no absolute-position encoder",
                port_letter(idx)
            ));
        }
        Ok(())
    }

    /// Issue a command that expects a value reply.
    fn send_value<F>(&mut self, build: F) -> Result<Reply, String>
    where
        F: FnOnce(u16) -> Result<Vec<u8>, String>,
    {
        let (rid, reply_rx, reply_tx) = self.begin_request()?;
        let payload = build(rid)?;
        self.dispatch(payload, rid, reply_tx, reply_rx)
    }
}

impl HardwareAdapter for SpikeAdapter {
    fn display_name(&self) -> &str { &self.display_name }
    fn output_ports(&self) -> &[String] { &[] }
    fn input_ports(&self) -> &[String] { &[] }
    fn connected(&self) -> bool {
        self.tx.is_some() && self.alive.load(Ordering::SeqCst)
    }
    fn max_power(&self) -> u8 { MAX_POWER }

    fn connect(&mut self) -> Result<(), String> {
        let mut transport: Box<dyn SpikeTransport> = if let Some(ref id) = self.identifier {
            Box::new(SpikeSerialTransport::open(id)?)
        } else {
            match find_usb_transport()? {
                Some(t) => t,
                None => match find_ble_transport()? {
                    Some(t) => t,
                    None => return Err(
                        "No SPIKE Prime hub found (no USB device with VID 0x0694, no BLE \
                         advertisement with service FD02)"
                            .to_string(),
                    ),
                },
            }
        };

        let mut framer = FrameReader::new();

        // Drain any residual REPL text already buffered by the hub.
        let mut scratch = [0u8; 1024];
        let drain_deadline = Instant::now() + Duration::from_millis(100);
        while Instant::now() < drain_deadline {
            if transport.read(&mut scratch)? == 0 {
                break;
            }
        }

        // Bootstrap: upload agent, start it, wait for ready.
        upload_agent(transport.as_mut(), &mut framer)?;
        start_agent_and_wait_ready(transport.as_mut(), &mut framer)?;

        let (tx, rx) = mpsc::channel();
        self.alive.store(true, Ordering::SeqCst);
        // Reset cache to "unknown" — leftover state from a prior session
        // would otherwise leak into validate_output_port until the snapshot
        // resolves.
        *self.port_types.lock().unwrap() = [PORT_TYPE_UNKNOWN; PORT_COUNT];
        let slot = SpikeSlot {
            transport,
            rx,
            framer,
            pending: HashMap::new(),
            alive: true,
            alive_flag: self.alive.clone(),
            last_heartbeat: Instant::now(),
            port_types: self.port_types.clone(),
        };
        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.slot_id = Some(slot_id);

        // Best-effort snapshot of which devices are attached to which ports.
        // Failure leaves the cache at PORT_TYPE_UNKNOWN, which validate_output_port
        // treats permissively — so a firmware that doesn't expose port.X.info()
        // still works, just without device-type errors.
        if let Ok(Reply::TypeList(types)) = self.send_value(|rid| Ok(protocol::port_types(rid))) {
            debug_spike(&format!(
                "snapshot: a=0x{:04x} b=0x{:04x} c=0x{:04x} d=0x{:04x} e=0x{:04x} f=0x{:04x}",
                types.first().copied().unwrap_or(0),
                types.get(1).copied().unwrap_or(0),
                types.get(2).copied().unwrap_or(0),
                types.get(3).copied().unwrap_or(0),
                types.get(4).copied().unwrap_or(0),
                types.get(5).copied().unwrap_or(0),
            ));
            let mut cache = self.port_types.lock().unwrap();
            for (i, t) in types.iter().enumerate().take(PORT_COUNT) {
                cache[i] = *t;
            }
        }
        Ok(())
    }

    fn disconnect(&mut self) {
        // Ask the slot to send ProgramFlowRequest(stop) directly — this
        // kills the agent program on the hub, which in turn halts every
        // motor the user had running. Going through the slot channel (not
        // the transport directly) keeps exclusive transport ownership with
        // the scheduler thread. Short timeout: if the slot thread has
        // already died there's nothing to wait for.
        if let Some(tx) = self.tx.as_ref() {
            let (ack_tx, ack_rx) = mpsc::channel();
            if tx.send(SpikeCommand::StopAgent { ack_tx }).is_ok() {
                let _ = ack_rx.recv_timeout(Duration::from_millis(500));
            }
        }
        self.alive.store(false, Ordering::SeqCst);
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        let idx = port_index_validate(port)
            .ok_or_else(|| format!("Unknown output port \"{}\"", port))?;
        let type_id = self.port_types.lock().unwrap()[idx];
        if type_id == PORT_TYPE_UNKNOWN {
            // Snapshot didn't resolve, or firmware doesn't expose port.info().
            // Defer to firmware to surface any per-command failure.
            return Ok(());
        }
        if type_id == 0 {
            return Err(format!("No device on port \"{}\"", port_letter(idx)));
        }
        if !is_motor(type_id) && !is_led(type_id) {
            return Err(format!(
                "Port \"{}\" is not a motor or light",
                port_letter(idx)
            ));
        }
        Ok(())
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        if port_index_validate(port).is_some() {
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
        let port_owned = port.to_string();
        if self.port_is_pid_capable(port) {
            let vel = to_velocity_i16(direction, power);
            self.send_void(move |rid| protocol::motor_run(rid, &port_owned, vel))
        } else {
            // Non-tacho (LED, passive motor, or unknown firmware): direct PWM.
            // `motor.run` would fail with ENODEV on these devices.
            let pwm = to_pwm_i8(direction, power);
            self.send_void(move |rid| protocol::port_pwm(rid, &port_owned, pwm))
        }
    }

    fn stop_port(&mut self, port: &str) -> Result<(), String> {
        let port_owned = port.to_string();
        if self.port_is_pid_capable(port) {
            self.send_void(move |rid| protocol::motor_stop(rid, &port_owned))
        } else {
            self.send_void(move |rid| protocol::port_pwm(rid, &port_owned, 0))
        }
    }

    fn run_port_for_time(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        tenths: u32,
    ) -> Result<(), String> {
        let ms = tenths * 100;
        let vel = to_velocity_i16(direction, power);
        let port = port.to_string();
        self.send_void(move |rid| protocol::motor_run_for_time(rid, &port, ms, vel))
    }

    fn rotate_port_by_degrees(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        degrees: i32,
    ) -> Result<(), String> {
        self.require_absolute_motor(port)?;
        let vel = to_velocity_i16(direction, power);
        let port = port.to_string();
        let deg = degrees.abs();
        self.send_void(move |rid| protocol::motor_run_for_degrees(rid, &port, deg, vel))
    }

    fn rotate_port_to_position(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
    ) -> Result<(), String> {
        self.require_absolute_motor(port)?;
        let port_owned = port.to_string();
        let current = match self.send_value(|rid| protocol::read_sensor(rid, &port_owned, "rotation"))? {
            Reply::Int(v) => v,
            other => return Err(format!("rotation read returned {:?}", other)),
        };
        let delta = crate::adapter::rotateto_delta(current, position, direction);
        if delta == 0 {
            return Ok(());
        }
        let base = (power.min(MAX_POWER) as i16) * 10;
        let speed = if delta > 0 { base } else { -base };
        let deg = delta.abs();
        self.send_void(move |rid| protocol::motor_run_for_degrees(rid, &port_owned, deg, speed))
    }

    fn reset_port_zero(&mut self, port: &str) -> Result<(), String> {
        self.require_absolute_motor(port)?;
        let port = port.to_string();
        self.send_void(move |rid| protocol::motor_reset(rid, &port, 0))
    }

    fn rotate_to_abs(
        &mut self,
        port: &str,
        direction: PortDirection,
        power: u8,
        position: i32,
    ) -> Result<(), String> {
        self.require_absolute_motor(port)?;
        let vel = (power.min(MAX_POWER) as i16) * 10;
        let dir = direction_code(direction);
        let port = port.to_string();
        self.send_void(move |rid| protocol::motor_run_to_abs(rid, &port, position, vel, dir))
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let mode_name = mode.unwrap_or("rotation");
        let canonical = match mode_name {
            "raw" => "rotation",
            other => other,
        };
        let port = port.to_string();
        let mode_owned = canonical.to_string();
        let reply = self.send_value(move |rid| protocol::read_sensor(rid, &port, &mode_owned))?;
        Ok(Some(reply_to_logo(reply)))
    }

    // ── Batch overrides ─────────────────────────

    fn run_ports_for_time(&mut self, commands: &[PortCommand], tenths: u32) -> Result<(), String> {
        // Hardware-parallel `parallel_run_for_time` only works for tacho
        // motors (it routes through `motor.run` on the agent side, which
        // returns ENODEV on LEDs/passive motors). Fall back to the default
        // per-port path (start_port + sleep + stop_port) when the group
        // isn't homogeneously tacho — start_port already dispatches to the
        // right primitive per port.
        if !commands.iter().all(|c| self.port_is_pid_capable(c.port)) {
            for cmd in commands {
                self.start_port(cmd.port, cmd.direction, cmd.power)?;
            }
            std::thread::sleep(Duration::from_millis(tenths as u64 * 100));
            for cmd in commands {
                self.stop_port(cmd.port)?;
            }
            return Ok(());
        }
        let ms = tenths * 100;
        let entries: Vec<(String, i16)> = commands
            .iter()
            .map(|c| (c.port.to_string(), to_velocity_i16(c.direction, c.power)))
            .collect();
        self.send_void(move |rid| {
            let refs: Vec<(&str, i16)> =
                entries.iter().map(|(p, v)| (p.as_str(), *v)).collect();
            protocol::parallel_run_for_time(rid, ms, &refs)
        })
    }

    fn rotate_ports_by_degrees(
        &mut self,
        commands: &[PortCommand],
        degrees: i32,
    ) -> Result<(), String> {
        for cmd in commands {
            self.require_absolute_motor(cmd.port)?;
        }
        let entries: Vec<(String, i32, i16)> = commands
            .iter()
            .map(|c| (c.port.to_string(), degrees.abs(), to_velocity_i16(c.direction, c.power)))
            .collect();
        self.send_void(move |rid| {
            let refs: Vec<(&str, i32, i16)> =
                entries.iter().map(|(p, d, v)| (p.as_str(), *d, *v)).collect();
            protocol::parallel_run_for_degrees(rid, &refs)
        })
    }

    fn rotate_ports_to_abs(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        for cmd in commands {
            self.require_absolute_motor(cmd.port)?;
        }
        let entries: Vec<(String, i32, i16, u8)> = commands
            .iter()
            .map(|c| {
                let vel = (c.power.min(MAX_POWER) as i16) * 10;
                (c.port.to_string(), position, vel, direction_code(c.direction))
            })
            .collect();
        self.send_void(move |rid| {
            let refs: Vec<(&str, i32, i16, u8)> =
                entries.iter().map(|(p, pos, v, d)| (p.as_str(), *pos, *v, *d)).collect();
            protocol::parallel_run_to_abs(rid, &refs)
        })
    }

    fn rotate_ports_to_position(
        &mut self,
        commands: &[PortCommand],
        position: i32,
    ) -> Result<(), String> {
        for cmd in commands {
            self.require_absolute_motor(cmd.port)?;
        }
        // Reads are cheap (per-port round-trip), but the rotation itself
        // must run concurrently — so we fan out the reads, compute each
        // delta, then issue a single parallel_run_for_degrees.
        let mut entries: Vec<(String, i32, i16)> = Vec::new();
        for cmd in commands {
            let port = cmd.port.to_string();
            let current = match self.send_value(|rid| protocol::read_sensor(rid, &port, "rotation"))? {
                Reply::Int(v) => v,
                other => return Err(format!("rotation read returned {:?}", other)),
            };
            let delta = crate::adapter::rotateto_delta(current, position, cmd.direction);
            if delta == 0 {
                continue;
            }
            let base = (cmd.power.min(MAX_POWER) as i16) * 10;
            let speed = if delta > 0 { base } else { -base };
            entries.push((port, delta.abs(), speed));
        }
        if entries.is_empty() {
            return Ok(());
        }
        self.send_void(move |rid| {
            let refs: Vec<(&str, i32, i16)> =
                entries.iter().map(|(p, d, v)| (p.as_str(), *d, *v)).collect();
            protocol::parallel_run_for_degrees(rid, &refs)
        })
    }
}

impl Drop for SpikeAdapter {
    fn drop(&mut self) {
        // Belt-and-braces cleanup: if the adapter goes out of scope without
        // an explicit disconnect (panic, unexpected shutdown), still try to
        // stop the agent so the hub's motors don't keep running.
        if self.tx.is_some() {
            self.disconnect();
        }
    }
}

fn reply_to_logo(reply: Reply) -> LogoValue {
    match reply {
        Reply::Int(v) => LogoValue::Number(v as f64),
        Reply::List(values) => LogoValue::List(
            values.into_iter().map(|v| LogoValue::Number(v as f64)).collect(),
        ),
        Reply::Bool(b) => LogoValue::Word(if b { "true" } else { "false" }.to_string()),
        Reply::Ok => LogoValue::Number(0.0),
        Reply::Error(msg) => LogoValue::Word(msg),
        Reply::TypeList(values) => LogoValue::List(
            values.into_iter().map(|v| LogoValue::Number(v as f64)).collect(),
        ),
    }
}

#[cfg(test)]
#[path = "../tests/spike_adapter.rs"]
mod tests;
