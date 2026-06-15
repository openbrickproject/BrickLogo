//! LEGO Dimensions ToyPad adapter (USB HID, slot-based).
//!
//! The ToyPad pushes tag add/remove *events* and answers *commands* on the same
//! HID stream. A scheduler slot ([`ToyPadSlot`]) runs the read loop: it routes
//! events into a per-pad tag-state map and correlates command responses (read
//! identity pages, list tags) by request id. Tag UIDs become visible the moment
//! a figure is placed; character/vehicle ids are decoded eagerly via a follow-up
//! page read and back-filled (eventually consistent). Decrypt failures are
//! silent — the tag stays in `tags` only.
//!
//! The three pads (`left`/`center`/`right`) are both input ports (`listento` →
//! tags) and output ports (`talkto` → `setrgb`). See `dev/SOURCES.md`.

use crate::adapter::{HardwareAdapter, PortDirection};
use crate::scheduler::{self, DeviceSlot};
use bricklogo_lang::value::LogoValue;
use hidapi::{HidApi, HidDevice};
use rust_toypad::constants::{Action, Panel, PACKET_LENGTH, PAGE_IDENTITY, PRODUCT_ID, VENDOR_ID};
use rust_toypad::protocol::{
    create_list_tags, create_read_tag, create_set_color, create_set_color_all, decode_list_tags,
    decode_message, encode_command, format_uid, Command, Incoming,
};
use rust_toypad::tag::{identify, Identity};
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

/// HID read/write abstraction so tests can inject a mock without real hardware.
pub trait ToyPadTransport: Send {
    fn read_frame(&mut self) -> Result<Option<[u8; PACKET_LENGTH]>, String>;
    fn write(&mut self, data: &[u8]) -> Result<(), String>;
}

impl ToyPadTransport for HidDevice {
    fn read_frame(&mut self) -> Result<Option<[u8; PACKET_LENGTH]>, String> {
        let mut buf = [0u8; PACKET_LENGTH + 1];
        match HidDevice::read(self, &mut buf).map_err(|e| e.to_string())? {
            0 => Ok(None),
            n => {
                let mut frame = [0u8; PACKET_LENGTH];
                let m = n.min(PACKET_LENGTH);
                frame[..m].copy_from_slice(&buf[..m]);
                Ok(Some(frame))
            }
        }
    }
    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        HidDevice::write(self, data).map(|_| ()).map_err(|e| e.to_string())
    }
}

/// Resolved identity of a present tag. `Pending` means the identity read is in
/// flight; `Unidentified` means it completed but the tag isn't a valid
/// Dimensions figure (decrypt failed / foreign tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagIdentity {
    Pending,
    Character(u16),
    Vehicle(u16),
    Unidentified,
}

#[derive(Debug, Clone)]
struct TagEntry {
    uid: [u8; 7],
    index: u8,
    identity: TagIdentity,
}

/// Per-pad tag state, shared between the slot (writer) and the adapter (reader).
#[derive(Default)]
pub struct ToyPadShared {
    tags: HashMap<Panel, HashMap<String, TagEntry>>,
}

/// What an outstanding command response will be used for.
enum Pending {
    /// `list_tags` response — enumerate present tags to seed state.
    List,
    /// page 0x00 read — recover a seeded tag's UID, then read its identity.
    Uid { panel: Panel, index: u8 },
    /// page 0x24 read — decode the tag's character/vehicle identity.
    Identity { panel: Panel, uid: [u8; 7], index: u8 },
}

struct OutCommand {
    command: Command,
    reply_tx: Option<mpsc::Sender<Result<(), String>>>,
}

// ── Slot ────────────────────────────────────────

struct ToyPadSlot {
    device: Box<dyn ToyPadTransport>,
    rx: mpsc::Receiver<OutCommand>,
    shared: Arc<Mutex<ToyPadShared>>,
    pending: HashMap<u8, Pending>,
    request_id: u8,
    seeded: bool,
    alive: bool,
}

impl ToyPadSlot {
    fn next_id(&mut self) -> u8 {
        self.request_id = self.request_id.wrapping_add(1);
        if self.request_id == 0 {
            self.request_id = 1;
        }
        self.request_id
    }

    fn write_command(&mut self, command: &Command, request_id: u8) -> Result<(), String> {
        let packet = encode_command(command, request_id);
        self.device.write(&packet)
    }

    /// Send a command and record what its response means.
    fn dispatch(&mut self, command: Command, pending: Pending) {
        let id = self.next_id();
        self.pending.insert(id, pending);
        let _ = self.write_command(&command, id);
    }

    fn handle_message(&mut self, msg: Incoming) {
        match msg {
            Incoming::Event(ev) => {
                if ev.panel == Panel::All {
                    return; // "all" events carry no specific pad — ignore for state
                }
                let key = format_uid(&ev.uid);
                match ev.action {
                    Action::Add => {
                        {
                            let mut shared = self.shared.lock().unwrap();
                            shared.tags.entry(ev.panel).or_default().insert(
                                key,
                                TagEntry { uid: ev.uid, index: ev.index, identity: TagIdentity::Pending },
                            );
                        }
                        // Eagerly read the identity page (UID is already visible).
                        self.dispatch(
                            create_read_tag(ev.index, PAGE_IDENTITY),
                            Pending::Identity { panel: ev.panel, uid: ev.uid, index: ev.index },
                        );
                    }
                    Action::Remove => {
                        let mut shared = self.shared.lock().unwrap();
                        if let Some(panel_tags) = shared.tags.get_mut(&ev.panel) {
                            panel_tags.remove(&key);
                        }
                    }
                }
            }
            Incoming::Response { request_id, payload } => {
                if let Some(pending) = self.pending.remove(&request_id) {
                    self.handle_response(pending, &payload);
                }
            }
        }
    }

    fn handle_response(&mut self, pending: Pending, payload: &[u8]) {
        match pending {
            Pending::List => {
                for entry in decode_list_tags(payload) {
                    // We know the pad+index but not the UID; read page 0 for it.
                    self.dispatch(
                        create_read_tag(entry.index, 0x00),
                        Pending::Uid { panel: entry.panel, index: entry.index },
                    );
                }
            }
            Pending::Uid { panel, index } => {
                if let Some(page) = read_tag_page(payload) {
                    // UID layout in page 0: bytes 0,1,2 then 4,5,6,7 (byte 3 is a
                    // check byte) — matches node-toypad's resolveTagByIndex.
                    let uid = [page[0], page[1], page[2], page[4], page[5], page[6], page[7]];
                    let key = format_uid(&uid);
                    {
                        let mut shared = self.shared.lock().unwrap();
                        shared
                            .tags
                            .entry(panel)
                            .or_default()
                            .entry(key)
                            .or_insert(TagEntry { uid, index, identity: TagIdentity::Pending });
                    }
                    self.dispatch(
                        create_read_tag(index, PAGE_IDENTITY),
                        Pending::Identity { panel, uid, index },
                    );
                }
            }
            Pending::Identity { panel, uid, index } => {
                if let Some(page) = read_tag_page(payload) {
                    let identity = match identify(&uid, &page) {
                        Some(Identity::Character(id)) => TagIdentity::Character(id),
                        Some(Identity::Vehicle(id)) => TagIdentity::Vehicle(id),
                        None => TagIdentity::Unidentified, // silent decrypt failure
                    };
                    let key = format_uid(&uid);
                    let mut shared = self.shared.lock().unwrap();
                    if let Some(entry) = shared.tags.get_mut(&panel).and_then(|m| m.get_mut(&key)) {
                        // Guard against a tag that was lifted before the read returned
                        // (and its index possibly reused).
                        if entry.index == index {
                            entry.identity = identity;
                        }
                    }
                }
            }
        }
    }
}

impl DeviceSlot for ToyPadSlot {
    fn tick(&mut self) {
        // Seed present tags on the first tick.
        if !self.seeded {
            self.seeded = true;
            self.dispatch(create_list_tags(), Pending::List);
        }

        // Drain all available frames, then process (avoids borrow tangles).
        let mut frames: Vec<[u8; PACKET_LENGTH]> = Vec::new();
        loop {
            match self.device.read_frame() {
                Ok(Some(frame)) => frames.push(frame),
                Ok(None) | Err(_) => break,
            }
        }
        for frame in frames {
            if let Some(msg) = decode_message(&frame) {
                self.handle_message(msg);
            }
        }

        // Drain outbound color commands.
        while let Ok(out) = self.rx.try_recv() {
            let id = self.next_id();
            let res = self.write_command(&out.command, id);
            if let Some(tx) = out.reply_tx {
                let _ = tx.send(res);
            }
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

fn read_tag_page(payload: &[u8]) -> Option<[u8; 16]> {
    // ReadTag response: [error_code, 16 bytes of card data, ...].
    if payload.len() < 17 || payload[0] != 0 {
        return None;
    }
    let mut page = [0u8; 16];
    page.copy_from_slice(&payload[1..17]);
    Some(page)
}

fn panel_for_port(port: &str) -> Result<Panel, String> {
    match port.to_lowercase().as_str() {
        "center" => Ok(Panel::Center),
        "left" => Ok(Panel::Left),
        "right" => Ok(Panel::Right),
        _ => Err(format!("Unknown ToyPad pad \"{}\" (use left, center, or right)", port)),
    }
}

// ── Adapter ─────────────────────────────────────

pub struct ToyPadAdapter {
    tx: Option<mpsc::Sender<OutCommand>>,
    shared: Arc<Mutex<ToyPadShared>>,
    slot_id: Option<usize>,
    display_name: String,
    ports: Vec<String>,
    identifier: Option<String>,
}

impl ToyPadAdapter {
    pub fn new(identifier: Option<&str>) -> Self {
        ToyPadAdapter {
            tx: None,
            shared: Arc::new(Mutex::new(ToyPadShared::default())),
            slot_id: None,
            display_name: "LEGO Dimensions ToyPad".to_string(),
            ports: vec!["left".to_string(), "center".to_string(), "right".to_string()],
            identifier: identifier.map(|s| s.to_string()),
        }
    }

    fn send_color_command(&self, command: Command) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("Not connected")?
            .send(OutCommand { command, reply_tx: Some(tx) })
            .map_err(|_| "Send failed".to_string())?;
        rx.recv_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| "Command timed out".to_string())?
    }
}

impl HardwareAdapter for ToyPadAdapter {
    fn display_name(&self) -> &str {
        &self.display_name
    }
    fn output_ports(&self) -> &[String] {
        &self.ports
    }
    fn input_ports(&self) -> &[String] {
        &self.ports
    }
    fn connected(&self) -> bool {
        self.tx.is_some()
    }

    fn connect(&mut self) -> Result<(), String> {
        let api = HidApi::new().map_err(|e| format!("Failed to init HID: {}", e))?;
        let device = if let Some(ref id) = self.identifier {
            let c_path = std::ffi::CString::new(id.as_str()).map_err(|e| e.to_string())?;
            api.open_path(&c_path)
                .map_err(|e| format!("Failed to open ToyPad at {}: {}", id, e))?
        } else {
            let info = api
                .device_list()
                .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
                .ok_or("No ToyPad device found")?;
            api.open_path(info.path())
                .map_err(|e| format!("Failed to open ToyPad: {}", e))?
        };
        device
            .set_blocking_mode(false)
            .map_err(|e| format!("Failed to set non-blocking: {}", e))?;
        // Wake the ToyPad — it ignores all commands until this is written.
        device
            .write(&rust_toypad::constants::WAKE_SEQUENCE)
            .map_err(|e| format!("Failed to wake ToyPad: {}", e))?;

        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Mutex::new(ToyPadShared::default()));
        let slot = ToyPadSlot {
            device: Box::new(device),
            rx,
            shared: shared.clone(),
            pending: HashMap::new(),
            request_id: 0,
            seeded: false,
            alive: true,
        };
        let slot_id = scheduler::register_slot(Box::new(slot));
        self.tx = Some(tx);
        self.shared = shared;
        self.slot_id = Some(slot_id);
        Ok(())
    }

    fn disconnect(&mut self) {
        if let Some(id) = self.slot_id.take() {
            scheduler::deregister_slot(id);
        }
        self.tx = None;
    }

    fn max_power(&self) -> u8 {
        255 // unused: the ToyPad has no power-controlled outputs
    }

    fn validate_output_port(&self, port: &str) -> Result<(), String> {
        panel_for_port(port).map(|_| ())
    }

    fn validate_sensor_port(&self, port: &str, mode: Option<&str>) -> Result<(), String> {
        panel_for_port(port)?;
        match mode {
            None | Some("tags") | Some("characters") | Some("vehicles") => Ok(()),
            Some(m) => Err(format!("Unsupported ToyPad mode \"{}\"", m)),
        }
    }

    fn read_sensor(&mut self, port: &str, mode: Option<&str>) -> Result<Option<LogoValue>, String> {
        let panel = panel_for_port(port)?;
        let shared = self.shared.lock().unwrap();
        let empty = HashMap::new();
        let panel_tags = shared.tags.get(&panel).unwrap_or(&empty);
        match mode {
            // Presence: a scalar count, so sensor? collapses correctly (an empty
            // list would read as truthy).
            None => Ok(Some(LogoValue::Number(panel_tags.len() as f64))),
            Some("tags") => {
                let uids: Vec<LogoValue> = panel_tags
                    .values()
                    .map(|e| LogoValue::Word(format_uid(&e.uid)))
                    .collect();
                Ok(Some(LogoValue::List(uids)))
            }
            Some("characters") => {
                let ids: Vec<LogoValue> = panel_tags
                    .values()
                    .filter_map(|e| match e.identity {
                        TagIdentity::Character(id) => Some(LogoValue::Number(id as f64)),
                        _ => None,
                    })
                    .collect();
                Ok(Some(LogoValue::List(ids)))
            }
            Some("vehicles") => {
                let ids: Vec<LogoValue> = panel_tags
                    .values()
                    .filter_map(|e| match e.identity {
                        TagIdentity::Vehicle(id) => Some(LogoValue::Number(id as f64)),
                        _ => None,
                    })
                    .collect();
                Ok(Some(LogoValue::List(ids)))
            }
            Some(m) => Err(format!("Unsupported ToyPad mode \"{}\"", m)),
        }
    }

    // ── Color output ────────────────────────────

    fn set_color(&mut self, _port: &str, _id: u8) -> Result<(), String> {
        Err("The ToyPad has no color palette — use setrgb [r g b] instead".to_string())
    }

    fn set_rgb(&mut self, port: &str, rgb: (u8, u8, u8)) -> Result<(), String> {
        let panel = panel_for_port(port)?;
        self.send_color_command(create_set_color(panel, rgb))
    }

    fn set_rgb_ports(&mut self, commands: &[(&str, (u8, u8, u8))]) -> Result<(), String> {
        if commands.is_empty() {
            return Ok(());
        }
        if commands.len() == 1 {
            let (port, rgb) = commands[0];
            return self.set_rgb(port, rgb);
        }
        // Several pads at once → one atomic SetColorAll.
        let (mut center, mut left, mut right) = (None, None, None);
        for (port, rgb) in commands {
            match panel_for_port(port)? {
                Panel::Center => center = Some(*rgb),
                Panel::Left => left = Some(*rgb),
                Panel::Right => right = Some(*rgb),
                Panel::All => {}
            }
        }
        self.send_color_command(create_set_color_all(center, left, right))
    }

    // ── Motor methods: not applicable ───────────

    fn start_port(&mut self, _port: &str, _direction: PortDirection, _power: u8) -> Result<(), String> {
        Err("The ToyPad has no motors".to_string())
    }
    fn stop_port(&mut self, _port: &str) -> Result<(), String> {
        Err("The ToyPad has no motors".to_string())
    }
    fn run_port_for_time(&mut self, _port: &str, _direction: PortDirection, _power: u8, _tenths: u32) -> Result<(), String> {
        Err("The ToyPad has no motors".to_string())
    }
    fn rotate_port_by_degrees(&mut self, _port: &str, _direction: PortDirection, _power: u8, _degrees: i32) -> Result<(), String> {
        Err("The ToyPad has no motors".to_string())
    }
    fn rotate_port_to_position(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("The ToyPad has no motors".to_string())
    }
    fn reset_port_zero(&mut self, _port: &str) -> Result<(), String> {
        Err("The ToyPad has no motors".to_string())
    }
    fn rotate_to_abs(&mut self, _port: &str, _direction: PortDirection, _power: u8, _position: i32) -> Result<(), String> {
        Err("The ToyPad has no motors".to_string())
    }
}

#[cfg(test)]
#[path = "../tests/toypad_adapter.rs"]
mod tests;
