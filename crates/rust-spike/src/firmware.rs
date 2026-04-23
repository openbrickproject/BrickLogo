//! In-band SPIKE Prime firmware upload over the Atlantis protocol.
//!
//! Sequence (per LEGO's `spike-prime-docs/messages.rst`):
//!
//!   1. `InfoRequest` / `InfoResponse` — caller passes the `InfoResponse`
//!      so we know `max_chunk_size`.
//!   2. `StartFirmwareUploadRequest(sha1, crc32)` — announce the upload. The
//!      hub replies with how many bytes it already has for this SHA (resume).
//!   3. `TransferChunkRequest(running_crc32, chunk)` stream.
//!   4. `BeginFirmwareUpdateRequest(sha1, crc32)` — finalize; hub reboots.
//!
//! Every outbound message is COBS-framed (see `crate::cobs`). Inbound frames
//! are split on the `0x02` delimiter, un-XORed, COBS-decoded, and parsed as
//! Atlantis messages.

use std::time::{Duration, Instant};

use sha1::{Digest, Sha1};

use crate::atlantis::{self, InfoResponse, Message};
use crate::cobs;
use crate::transport::Transport;

/// Progress callback — `(bytes_done, bytes_total, phase)`.
pub type ProgressFn = Box<dyn Fn(usize, usize, &str) + Send>;

const CHUNK_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

/// Upload a SPIKE Prime firmware image in-band via the Atlantis protocol.
///
/// `info` is the `InfoResponse` from a prior `InfoRequest` — only
/// `info.max_chunk_size` is consulted, so the caller can stub the rest in
/// tests.
pub fn upload_firmware(
    transport: &mut dyn Transport,
    info: &InfoResponse,
    dfu_bytes: &[u8],
    progress: &ProgressFn,
) -> Result<(), String> {
    if dfu_bytes.is_empty() {
        return Err("firmware image is empty".into());
    }

    let mut sha = Sha1::new();
    sha.update(dfu_bytes);
    let sha1: [u8; 20] = sha.finalize().into();
    let whole_crc = atlantis::crc32_padded(dfu_bytes, 0);

    if std::env::var("BRICKLOGO_DEBUG_SPIKE").is_ok() {
        let raw_crc = crc32fast::hash(dfu_bytes);
        eprintln!(
            "[spike] firmware sha1={} crc32_padded={:08x} raw_crc32={:08x} size={}",
            hex_encode(&sha1),
            whole_crc,
            raw_crc,
            dfu_bytes.len(),
        );
    }

    write_framed(transport, &atlantis::start_firmware_upload_request(&sha1, whole_crc))?;
    let resume_from = match read_until(transport, |m| {
        matches!(m, Message::StartFirmwareUploadResponse { .. })
    })
    .map_err(|e| format!("{} (during StartFirmwareUploadRequest)", e))?
    {
        Message::StartFirmwareUploadResponse { success: false, .. } => {
            return Err("hub refused firmware upload".into());
        }
        Message::StartFirmwareUploadResponse { bytes_already_uploaded, .. } => {
            if std::env::var("BRICKLOGO_DEBUG_SPIKE").is_ok() {
                eprintln!(
                    "[spike] hub already has {} bytes for this sha1",
                    bytes_already_uploaded,
                );
            }
            bytes_already_uploaded as usize
        }
        _ => unreachable!(),
    };

    let chunk_size = info.max_chunk_size.max(64) as usize;
    let total = dfu_bytes.len();
    let mut running = 0u32;

    // Bring the running CRC forward over the resumed prefix so the hub's
    // per-chunk CRC check keeps agreeing with us.
    if resume_from > 0 {
        running = atlantis::crc32_padded(&dfu_bytes[..resume_from], 0);
    }

    let mut pos = resume_from;
    while pos < total {
        let end = (pos + chunk_size).min(total);
        let chunk = &dfu_bytes[pos..end];
        running = atlantis::crc32_padded(chunk, running);
        write_framed(transport, &atlantis::transfer_chunk_request(running, chunk))?;
        match read_until(transport, |m| matches!(m, Message::TransferChunkResponse { .. }))
            .map_err(|e| format!("{} (after chunk at offset {})", e, pos))?
        {
            Message::TransferChunkResponse { success: true } => {}
            Message::TransferChunkResponse { success: false } => {
                return Err(format!("hub rejected chunk at offset {}", pos));
            }
            _ => unreachable!(),
        }
        pos = end;
        progress(pos, total, "writing");
    }

    write_framed(transport, &atlantis::begin_firmware_update_request(&sha1, whole_crc))?;
    match read_until(transport, |m| matches!(m, Message::BeginFirmwareUpdateResponse { .. }))
        .map_err(|e| format!("{} (during BeginFirmwareUpdateRequest)", e))?
    {
        Message::BeginFirmwareUpdateResponse { success: true } => Ok(()),
        Message::BeginFirmwareUpdateResponse { success: false } => {
            Err("hub rejected BeginFirmwareUpdate".into())
        }
        _ => unreachable!(),
    }
}

// ── Wire helpers ─────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn write_framed(transport: &mut dyn Transport, message: &[u8]) -> Result<(), String> {
    let framed = cobs::pack(message);
    transport.write_all(&framed)?;
    transport.flush()
}

fn read_until<F>(transport: &mut dyn Transport, mut predicate: F) -> Result<Message, String>
where
    F: FnMut(&Message) -> bool,
{
    let deadline = Instant::now() + CHUNK_RESPONSE_TIMEOUT;
    let mut buf = [0u8; 1024];
    let mut frame_buf: Vec<u8> = Vec::new();
    loop {
        // Pop any complete frames we've buffered.
        while let Some(pos) = frame_buf.iter().position(|&b| b == cobs::END_FRAME) {
            let frame: Vec<u8> = frame_buf.drain(..=pos).collect();
            let body = match cobs::unpack(&frame[..frame.len() - 1]) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let msg = match atlantis::parse(&body) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if predicate(&msg) {
                return Ok(msg);
            }
            // Non-matching message (e.g. heartbeat, console noise) — drop.
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for hub reply".into());
        }
        match transport.read(&mut buf)? {
            0 => std::thread::sleep(Duration::from_millis(5)),
            n => frame_buf.extend_from_slice(&buf[..n]),
        }
    }
}

#[cfg(test)]
#[path = "tests/firmware.rs"]
mod tests;
