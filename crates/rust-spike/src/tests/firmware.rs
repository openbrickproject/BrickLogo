use super::*;
use crate::atlantis;
use crate::cobs;
use std::collections::VecDeque;

/// Scripted transport that replays one of several canned hub reply sequences.
struct ScriptedTransport {
    outgoing: VecDeque<u8>,
    writes: Vec<Vec<u8>>,
    /// Called after each write to prepare the next reply. Returns the bytes
    /// (already framed) to append to `outgoing`.
    on_write: Box<dyn FnMut(&[u8]) -> Vec<u8> + Send>,
}

impl Transport for ScriptedTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        if self.outgoing.is_empty() {
            return Ok(0);
        }
        let n = buf.len().min(self.outgoing.len());
        for slot in buf.iter_mut().take(n) {
            *slot = self.outgoing.pop_front().unwrap();
        }
        Ok(n)
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.writes.push(data.to_vec());
        let reply = (self.on_write)(data);
        self.outgoing.extend(reply);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> { Ok(()) }
}

fn frame(msg: &[u8]) -> Vec<u8> {
    cobs::pack(msg)
}

fn info_with_chunk(size: u16) -> atlantis::InfoResponse {
    atlantis::InfoResponse {
        rpc_major: 0, rpc_minor: 0, rpc_build: 0,
        firmware_major: 0, firmware_minor: 0, firmware_build: 0,
        max_packet_size: 0, max_message_size: 0,
        max_chunk_size: size,
        product_group_device: 0,
    }
}

#[test]
fn test_upload_firmware_happy_path() {
    let dfu = vec![0xAAu8; 200]; // will split into 2 chunks at max_chunk_size=128
    let info = info_with_chunk(128);

    let mut transport = ScriptedTransport {
        outgoing: VecDeque::new(),
        writes: Vec::new(),
        on_write: Box::new(|data: &[u8]| {
            // Unpack so we can peek at the request id and synthesise the
            // matching response.
            if data.len() < 2 {
                return Vec::new();
            }
            let body = cobs::unpack(&data[..data.len() - 1]).unwrap_or_default();
            let id = body.first().copied().unwrap_or(0);
            match id {
                atlantis::ID_START_FIRMWARE_UPLOAD_REQUEST => {
                    let mut reply = vec![atlantis::ID_START_FIRMWARE_UPLOAD_RESPONSE, 0x00];
                    reply.extend_from_slice(&0u32.to_le_bytes());
                    frame(&reply)
                }
                atlantis::ID_TRANSFER_CHUNK_REQUEST => {
                    frame(&[atlantis::ID_TRANSFER_CHUNK_RESPONSE, 0x00])
                }
                atlantis::ID_BEGIN_FIRMWARE_UPDATE_REQUEST => {
                    frame(&[atlantis::ID_BEGIN_FIRMWARE_UPDATE_RESPONSE, 0x00])
                }
                _ => Vec::new(),
            }
        }),
    };

    let progress: ProgressFn = Box::new(|_, _, _| {});
    upload_firmware(&mut transport, &info, &dfu, &progress).unwrap();

    // Expected: 1 StartFirmwareUpload + 2 TransferChunk + 1 BeginFirmwareUpdate
    let ids: Vec<u8> = transport
        .writes
        .iter()
        .filter_map(|w| cobs::unpack(&w[..w.len() - 1]).ok())
        .filter_map(|b| b.first().copied())
        .collect();
    assert_eq!(
        ids,
        vec![
            atlantis::ID_START_FIRMWARE_UPLOAD_REQUEST,
            atlantis::ID_TRANSFER_CHUNK_REQUEST,
            atlantis::ID_TRANSFER_CHUNK_REQUEST,
            atlantis::ID_BEGIN_FIRMWARE_UPDATE_REQUEST,
        ]
    );
}

#[test]
fn test_upload_firmware_rejected_start() {
    let dfu = vec![0u8; 64];
    let info = info_with_chunk(64);
    let mut transport = ScriptedTransport {
        outgoing: VecDeque::new(),
        writes: Vec::new(),
        on_write: Box::new(|data: &[u8]| {
            let body = cobs::unpack(&data[..data.len() - 1]).unwrap_or_default();
            if body.first() == Some(&atlantis::ID_START_FIRMWARE_UPLOAD_REQUEST) {
                let mut reply = vec![atlantis::ID_START_FIRMWARE_UPLOAD_RESPONSE, 0x01];
                reply.extend_from_slice(&0u32.to_le_bytes());
                frame(&reply)
            } else {
                Vec::new()
            }
        }),
    };
    let progress: ProgressFn = Box::new(|_, _, _| {});
    let err = upload_firmware(&mut transport, &info, &dfu, &progress).unwrap_err();
    assert!(err.contains("refused"), "got {:?}", err);
}

#[test]
fn test_upload_firmware_resume_skips_already_uploaded() {
    let dfu = vec![0u8; 256];
    let info = info_with_chunk(64);
    let resume_from = 128u32;

    let mut transport = ScriptedTransport {
        outgoing: VecDeque::new(),
        writes: Vec::new(),
        on_write: Box::new(move |data: &[u8]| {
            let body = cobs::unpack(&data[..data.len() - 1]).unwrap_or_default();
            let id = body.first().copied().unwrap_or(0);
            match id {
                atlantis::ID_START_FIRMWARE_UPLOAD_REQUEST => {
                    let mut reply = vec![atlantis::ID_START_FIRMWARE_UPLOAD_RESPONSE, 0x00];
                    reply.extend_from_slice(&resume_from.to_le_bytes());
                    frame(&reply)
                }
                atlantis::ID_TRANSFER_CHUNK_REQUEST => {
                    frame(&[atlantis::ID_TRANSFER_CHUNK_RESPONSE, 0x00])
                }
                atlantis::ID_BEGIN_FIRMWARE_UPDATE_REQUEST => {
                    frame(&[atlantis::ID_BEGIN_FIRMWARE_UPDATE_RESPONSE, 0x00])
                }
                _ => Vec::new(),
            }
        }),
    };
    let progress: ProgressFn = Box::new(|_, _, _| {});
    upload_firmware(&mut transport, &info, &dfu, &progress).unwrap();

    // 256 bytes total, 128 resumed, chunks of 64 → expect 2 TransferChunk.
    let chunk_count = transport
        .writes
        .iter()
        .filter(|w| {
            cobs::unpack(&w[..w.len() - 1])
                .map(|b| b.first().copied() == Some(atlantis::ID_TRANSFER_CHUNK_REQUEST))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(chunk_count, 2);
}
