//! ToyPad frame encoding and decoding.
//!
//! Ported from `node-toypad` (`src/protocol.ts`). The transport is a stream of
//! fixed 32-byte HID packets that multiplexes command *responses* (first byte
//! `0x55`, correlated by a one-byte request id) and unsolicited tag *events*
//! (first byte `0x56`).

use crate::constants::{
    Action, Panel, CMD_ACTION, MSG_EVENT, MSG_RESPONSE, PACKET_LENGTH, REQ_FADE, REQ_FADE_ALL,
    REQ_LIST_TAGS, REQ_READ_TAG, REQ_SET_COLOR, REQ_SET_COLOR_ALL,
};

/// An outgoing command: a request id byte plus parameter bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub id: u8,
    pub params: Vec<u8>,
}

/// A decoded tag add/remove event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEvent {
    pub panel: Panel,
    pub action: Action,
    pub index: u8,
    pub tag_type: u8,
    pub uid: [u8; 7],
}

/// A decoded incoming frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    Event(TagEvent),
    Response { request_id: u8, payload: Vec<u8> },
}

/// One entry of a `list_tags` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    pub panel: Panel,
    pub index: u8,
    pub ok: bool,
}

/// Encode a command into a padded 32-byte ToyPad packet:
/// `[0x55, len+2, id, request_id, params..., checksum, 0-pad..]`.
pub fn encode_command(command: &Command, request_id: u8) -> [u8; PACKET_LENGTH] {
    let mut bytes: Vec<u8> = Vec::with_capacity(PACKET_LENGTH);
    bytes.push(MSG_RESPONSE);
    bytes.push(((command.params.len() + 2) & 0xff) as u8);
    bytes.push(command.id);
    bytes.push(request_id);
    bytes.extend_from_slice(&command.params);
    let checksum = bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b));
    bytes.push(checksum);

    let mut packet = [0u8; PACKET_LENGTH];
    let n = bytes.len().min(PACKET_LENGTH);
    packet[..n].copy_from_slice(&bytes[..n]);
    packet
}

/// Decode an incoming HID frame. Returns `None` for empty, truncated, or
/// unrecognized packets. Strips a leading HID report-id byte if present.
pub fn decode_message(data: &[u8]) -> Option<Incoming> {
    let data = normalize_packet(data);
    let &kind = data.first()?;
    match kind {
        MSG_EVENT => decode_action_event(data).map(Incoming::Event),
        MSG_RESPONSE => decode_response(data),
        _ => None,
    }
}

fn normalize_packet(data: &[u8]) -> &[u8] {
    if data.len() > 1 && data[0] == 0x00 {
        &data[1..]
    } else {
        data
    }
}

fn decode_action_event(data: &[u8]) -> Option<TagEvent> {
    if data.len() < 14 || data[1] != CMD_ACTION {
        return None;
    }
    let mut uid = [0u8; 7];
    uid.copy_from_slice(&data[6..13]);
    Some(TagEvent {
        panel: Panel::from_byte(data[2]),
        tag_type: data[3],
        index: data[4],
        action: Action::from_byte(data[5]),
        uid,
    })
}

fn decode_response(data: &[u8]) -> Option<Incoming> {
    if data.len() < 3 {
        return None;
    }
    let length = data[1] as usize;
    let payload_start = 2usize;
    let payload_end = (data.len() - 1).min(payload_start + length);
    if payload_end <= payload_start {
        return None;
    }
    let with_request_id = &data[payload_start..payload_end];
    let (&request_id, payload) = with_request_id.split_first()?;
    Some(Incoming::Response {
        request_id,
        payload: payload.to_vec(),
    })
}

/// Format a 7-byte UID as space-separated lowercase hex (the canonical tag key).
pub fn format_uid(uid: &[u8; 7]) -> String {
    uid.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Command builders ─────────────────────────────

/// Set a single pad to an RGB color.
pub fn create_set_color(panel: Panel, rgb: (u8, u8, u8)) -> Command {
    Command {
        id: REQ_SET_COLOR,
        params: vec![panel as u8, rgb.0, rgb.1, rgb.2],
    }
}

/// Set all three pads in one command. `None` leaves a pad unchanged.
/// Order on the wire is center, left, right.
pub fn create_set_color_all(
    center: Option<(u8, u8, u8)>,
    left: Option<(u8, u8, u8)>,
    right: Option<(u8, u8, u8)>,
) -> Command {
    let mut params = Vec::with_capacity(12);
    for pad in [center, left, right] {
        match pad {
            Some((r, g, b)) => params.extend_from_slice(&[1, r, g, b]),
            None => params.extend_from_slice(&[0, 0, 0, 0]),
        }
    }
    Command {
        id: REQ_SET_COLOR_ALL,
        params,
    }
}

/// Per-pad fade parameters: ramp to `rgb` at `speed`, repeated `cycles` times.
/// `speed` and `cycles` are passed to the ToyPad verbatim (their units are not
/// yet calibrated — see `dev/SOURCES.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FadeSpec {
    pub speed: u8,
    pub cycles: u8,
    pub rgb: (u8, u8, u8),
}

/// Fade a single pad toward an RGB color.
pub fn create_fade(panel: Panel, spec: FadeSpec) -> Command {
    Command {
        id: REQ_FADE,
        params: vec![panel as u8, spec.speed, spec.cycles, spec.rgb.0, spec.rgb.1, spec.rgb.2],
    }
}

/// Fade all three pads in one command. `None` leaves a pad unchanged.
/// Order on the wire is center, left, right; each pad is 6 bytes:
/// `[enabled, speed, cycles, r, g, b]`. (The leading enable byte is required —
/// node-toypad's `FadeAll` omits it, which shifts every field by one.)
pub fn create_fade_all(
    center: Option<FadeSpec>,
    left: Option<FadeSpec>,
    right: Option<FadeSpec>,
) -> Command {
    let mut params = Vec::with_capacity(18);
    for pad in [center, left, right] {
        match pad {
            Some(s) => params.extend_from_slice(&[1, s.speed, s.cycles, s.rgb.0, s.rgb.1, s.rgb.2]),
            None => params.extend_from_slice(&[0, 0, 0, 0, 0, 0]),
        }
    }
    Command {
        id: REQ_FADE_ALL,
        params,
    }
}

/// Read a 16-byte NFC page from the tag at the given pad index.
pub fn create_read_tag(index: u8, page: u8) -> Command {
    Command {
        id: REQ_READ_TAG,
        params: vec![index, page],
    }
}

/// Ask the ToyPad to report all currently-present tags.
pub fn create_list_tags() -> Command {
    Command {
        id: REQ_LIST_TAGS,
        params: vec![],
    }
}

/// Decode a `list_tags` response into per-pad entries. Each entry is two bytes:
/// `[panel<<4 | index, status]`. Entries with an out-of-range panel are skipped.
pub fn decode_list_tags(payload: &[u8]) -> Vec<ListEntry> {
    let mut entries = Vec::new();
    let mut i = 0;
    while i + 1 < payload.len() {
        let byte0 = payload[i];
        let byte1 = payload[i + 1];
        let panel = (byte0 >> 4) & 0x0f;
        let index = byte0 & 0x0f;
        if (1..=3).contains(&panel) {
            entries.push(ListEntry {
                panel: Panel::from_byte(panel),
                index,
                ok: byte1 == 0x00,
            });
        }
        i += 2;
    }
    entries
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
