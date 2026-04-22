//! LEGO SPIKE Prime Atlantis binary protocol messages.
//!
//! Each message is `[id: u8, ...payload]`. On the wire the message is
//! COBS-framed with XOR stuffing (see `cobs.rs`). Layouts match LEGO's
//! reference implementation at
//! <https://github.com/LEGO/spike-prime-docs/blob/main/examples/python/messages.py>.

use crc32fast::Hasher;

// ── Message IDs ─────────────────────────────────

pub const ID_INFO_REQUEST: u8 = 0x00;
pub const ID_INFO_RESPONSE: u8 = 0x01;
pub const ID_START_FILE_UPLOAD_REQUEST: u8 = 0x0C;
pub const ID_START_FILE_UPLOAD_RESPONSE: u8 = 0x0D;
pub const ID_TRANSFER_CHUNK_REQUEST: u8 = 0x10;
pub const ID_TRANSFER_CHUNK_RESPONSE: u8 = 0x11;
pub const ID_PROGRAM_FLOW_REQUEST: u8 = 0x1E;
pub const ID_PROGRAM_FLOW_RESPONSE: u8 = 0x1F;
pub const ID_PROGRAM_FLOW_NOTIFICATION: u8 = 0x20;
pub const ID_CONSOLE_NOTIFICATION: u8 = 0x21;
pub const ID_DEVICE_NOTIFICATION_REQUEST: u8 = 0x28;
pub const ID_DEVICE_NOTIFICATION_RESPONSE: u8 = 0x29;
pub const ID_DEVICE_NOTIFICATION: u8 = 0x3C;
pub const ID_TUNNEL_MESSAGE: u8 = 0x32;
pub const ID_CLEAR_SLOT_REQUEST: u8 = 0x46;
pub const ID_CLEAR_SLOT_RESPONSE: u8 = 0x47;

/// Max filename length accepted by `StartFileUploadRequest` (incl. trailing
/// NUL); hub rejects longer names.
pub const MAX_FILENAME_LEN: usize = 31;

// ── Parsed message representation ───────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InfoResponse {
    pub rpc_major: u8,
    pub rpc_minor: u8,
    pub rpc_build: u16,
    pub firmware_major: u8,
    pub firmware_minor: u8,
    pub firmware_build: u16,
    pub max_packet_size: u16,
    pub max_message_size: u16,
    pub max_chunk_size: u16,
    pub product_group_device: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    InfoResponse(InfoResponse),
    StartFileUploadResponse { success: bool },
    TransferChunkResponse { success: bool },
    ProgramFlowResponse { success: bool },
    ProgramFlowNotification { stop: bool },
    ClearSlotResponse { success: bool },
    DeviceNotificationResponse { success: bool },
    DeviceNotification { payload: Vec<u8> },
    ConsoleNotification { text: String },
    /// A `TunnelMessage` from the hub. Observed wire layout is plain
    /// `[0x32, size_u16_LE, payload]`, where `payload` is the raw bytes the
    /// running program passed to `hub.config["module_tunnel"].send(...)`.
    /// (Host→hub uses a chunked variant with `frame_id`/`frame_total` — see
    /// `tunnel_chunks` — which the firmware reassembles before delivering
    /// to the Python callback.)
    Tunnel { payload: Vec<u8> },
    Unknown { id: u8, payload: Vec<u8> },
}

// ── Outbound message builders ───────────────────

pub fn info_request() -> Vec<u8> {
    // Python: `return b"\0"`. Note: single zero byte, not empty.
    vec![ID_INFO_REQUEST]
}

pub fn clear_slot_request(slot: u8) -> Vec<u8> {
    vec![ID_CLEAR_SLOT_REQUEST, slot]
}

pub fn start_file_upload_request(filename: &str, slot: u8, crc32: u32) -> Vec<u8> {
    // Python: <B{len(name)+1}sBI — id, utf8_bytes + NUL terminator, slot, crc (u32 LE).
    // The `+1` padding byte is the NUL terminator appended to the UTF-8 bytes.
    let name_bytes = filename.as_bytes();
    assert!(name_bytes.len() < MAX_FILENAME_LEN, "filename too long");
    let mut buf = Vec::with_capacity(2 + name_bytes.len() + 1 + 4);
    buf.push(ID_START_FILE_UPLOAD_REQUEST);
    buf.extend_from_slice(name_bytes);
    buf.push(0);
    buf.push(slot);
    buf.extend_from_slice(&crc32.to_le_bytes());
    buf
}

pub fn transfer_chunk_request(running_crc32: u32, chunk: &[u8]) -> Vec<u8> {
    // Python: <BIH{size}s — id, running_crc (u32 LE), size (u16 LE), payload.
    let mut buf = Vec::with_capacity(1 + 4 + 2 + chunk.len());
    buf.push(ID_TRANSFER_CHUNK_REQUEST);
    buf.extend_from_slice(&running_crc32.to_le_bytes());
    buf.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
    buf.extend_from_slice(chunk);
    buf
}

pub fn program_flow_request(stop: bool, slot: u8) -> Vec<u8> {
    // Python: <BBB — id, stop (u8), slot.
    vec![ID_PROGRAM_FLOW_REQUEST, if stop { 1 } else { 0 }, slot]
}

pub fn device_notification_request(interval_ms: u16) -> Vec<u8> {
    // Python: <BH — id, interval_ms.
    let mut buf = Vec::with_capacity(3);
    buf.push(ID_DEVICE_NOTIFICATION_REQUEST);
    buf.extend_from_slice(&interval_ms.to_le_bytes());
    buf
}

/// Build a TunnelMessage with the plain LEGO-spec wire layout:
///
/// ```text
///   [0x32, size_LE_u16, payload...]
/// ```
///
/// Used in both directions. The community "chunked" variant with a
/// `[frame_id, frame_total]` header is application-level encoding carried
/// inside `payload`, not a separate wire format.
pub fn tunnel_message(payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(3 + payload.len());
    buf.push(ID_TUNNEL_MESSAGE);
    buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

// ── Inbound message parser ──────────────────────

pub fn parse(data: &[u8]) -> Result<Message, String> {
    if data.is_empty() {
        return Err("Atlantis parse: empty message".to_string());
    }
    let id = data[0];
    match id {
        ID_INFO_RESPONSE => parse_info_response(data),
        ID_START_FILE_UPLOAD_RESPONSE => Ok(Message::StartFileUploadResponse {
            success: status_byte(data) == 0,
        }),
        ID_TRANSFER_CHUNK_RESPONSE => Ok(Message::TransferChunkResponse {
            success: status_byte(data) == 0,
        }),
        ID_PROGRAM_FLOW_RESPONSE => Ok(Message::ProgramFlowResponse {
            success: status_byte(data) == 0,
        }),
        ID_PROGRAM_FLOW_NOTIFICATION => Ok(Message::ProgramFlowNotification {
            stop: status_byte(data) != 0,
        }),
        ID_CLEAR_SLOT_RESPONSE => Ok(Message::ClearSlotResponse {
            success: status_byte(data) == 0,
        }),
        ID_DEVICE_NOTIFICATION_RESPONSE => Ok(Message::DeviceNotificationResponse {
            success: status_byte(data) == 0,
        }),
        ID_DEVICE_NOTIFICATION => Ok(Message::DeviceNotification {
            payload: if data.len() > 3 { data[3..].to_vec() } else { Vec::new() },
        }),
        ID_CONSOLE_NOTIFICATION => {
            // Trim trailing NUL bytes (the hub pads to a fixed size).
            let text_bytes = data[1..].split(|&b| b == 0).next().unwrap_or(&[]);
            Ok(Message::ConsoleNotification {
                text: String::from_utf8_lossy(text_bytes).into_owned(),
            })
        }
        ID_TUNNEL_MESSAGE => {
            // Plain wire layout: [id, size_u16_LE, payload...].
            if data.len() < 3 {
                return Err(format!("TunnelMessage too short: {} bytes", data.len()));
            }
            let size = u16::from_le_bytes([data[1], data[2]]) as usize;
            let end = (3 + size).min(data.len());
            Ok(Message::Tunnel { payload: data[3..end].to_vec() })
        }
        _ => Ok(Message::Unknown { id, payload: data[1..].to_vec() }),
    }
}

fn status_byte(data: &[u8]) -> u8 {
    data.get(1).copied().unwrap_or(0xFF)
}

fn parse_info_response(data: &[u8]) -> Result<Message, String> {
    // Python unpack string: <BBBHBBHHHHH
    // id u8, rpc_major u8, rpc_minor u8, rpc_build u16, fw_major u8,
    // fw_minor u8, fw_build u16, max_packet u16, max_msg u16,
    // max_chunk u16, product_group_device u16
    if data.len() < 17 {
        return Err(format!("InfoResponse too short: {} bytes", data.len()));
    }
    let rpc_major = data[1];
    let rpc_minor = data[2];
    let rpc_build = u16::from_le_bytes([data[3], data[4]]);
    let firmware_major = data[5];
    let firmware_minor = data[6];
    let firmware_build = u16::from_le_bytes([data[7], data[8]]);
    let max_packet_size = u16::from_le_bytes([data[9], data[10]]);
    let max_message_size = u16::from_le_bytes([data[11], data[12]]);
    let max_chunk_size = u16::from_le_bytes([data[13], data[14]]);
    let product_group_device = u16::from_le_bytes([data[15], data[16]]);
    Ok(Message::InfoResponse(InfoResponse {
        rpc_major,
        rpc_minor,
        rpc_build,
        firmware_major,
        firmware_minor,
        firmware_build,
        max_packet_size,
        max_message_size,
        max_chunk_size,
        product_group_device,
    }))
}

// ── CRC32 helpers (for file upload) ─────────────
//
// LEGO's `crc.py` pads to 4-byte alignment with zeros before hashing. This
// matters for file transfer — the hub's running CRC is computed against the
// padded bytes.

/// CRC32 with padding to a 4-byte boundary.
pub fn crc32_padded(data: &[u8], seed: u32) -> u32 {
    let mut hasher = Hasher::new_with_initial(seed);
    hasher.update(data);
    let remainder = data.len() % 4;
    if remainder != 0 {
        let pad = vec![0u8; 4 - remainder];
        hasher.update(&pad);
    }
    hasher.finalize()
}

pub struct RunningCrc {
    current: u32,
}

impl RunningCrc {
    pub fn new() -> Self { RunningCrc { current: 0 } }
    /// Feed a chunk. Pads each chunk to 4-byte boundary to match the hub.
    pub fn update(&mut self, data: &[u8]) {
        self.current = crc32_padded(data, self.current);
    }
    pub fn finalize(&self) -> u32 { self.current }
}

impl Default for RunningCrc {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
#[path = "tests/atlantis.rs"]
mod tests;
