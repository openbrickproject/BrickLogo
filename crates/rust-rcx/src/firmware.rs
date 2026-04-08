use crate::constants::*;
use crate::protocol;
use crate::srec::FirmwareImage;

/// Progress callback: (current_block, total_blocks, phase_description)
pub type ProgressFn = Box<dyn Fn(usize, usize, &str) + Send>;

/// Upload firmware to the RCX.
///
/// `send_request` is a closure that sends a framed message and returns the reply payload.
/// This decouples the upload logic from the transport implementation.
pub fn upload_firmware<F>(
    image: &FirmwareImage,
    send_request: &mut F,
    progress: &ProgressFn,
) -> Result<(), String>
where
    F: FnMut(&[u8]) -> Result<Vec<u8>, String>,
{
    let total_blocks = (image.data.len() + FIRMWARE_BLOCK_SIZE - 1) / FIRMWARE_BLOCK_SIZE;

    // Phase 1: Delete firmware
    progress(0, total_blocks, "Deleting old firmware");
    let msg = protocol::cmd_delete_firmware();
    send_with_retry(&msg, send_request, FIRMWARE_MAX_RETRIES)
        .map_err(|e| format!("Delete firmware failed: {}", e))?;

    // Phase 2: Start firmware download
    progress(0, total_blocks, "Starting firmware download");
    let msg = protocol::cmd_start_firmware_download(image.base_address, image.checksum);
    let reply = send_with_retry(&msg, send_request, FIRMWARE_MAX_RETRIES)
        .map_err(|e| format!("Start download failed: {}", e))?;
    // Check for error code in reply (byte 1, 0 = success)
    if reply.len() >= 2 && reply[1] != 0 {
        return Err(format!("Start download rejected (error code {})", reply[1]));
    }

    // Phase 3: Transfer data blocks
    let mut offset = 0;
    let mut block_index: u16 = 1;

    while offset < image.data.len() {
        let end = (offset + FIRMWARE_BLOCK_SIZE).min(image.data.len());
        let block_data = &image.data[offset..end];
        let is_last = end >= image.data.len();

        // Last block uses index 0
        let index = if is_last { 0 } else { block_index };

        progress(block_index as usize, total_blocks, "Uploading firmware");

        let msg = protocol::cmd_transfer_data(index, block_data);
        let reply = send_with_retry(&msg, send_request, FIRMWARE_MAX_RETRIES)
            .map_err(|e| format!("Transfer block {} failed: {}", block_index, e))?;

        // Check for error code in reply
        if reply.len() >= 2 && reply[1] != 0 {
            return Err(format!("Transfer block {} rejected (error code {})", block_index, reply[1]));
        }

        offset = end;
        block_index += 1;
    }

    // Phase 4: Unlock firmware
    progress(total_blocks, total_blocks, "Unlocking firmware");
    let msg = protocol::cmd_unlock_firmware();
    // Unlock takes longer — RCX verifies ROM checksum
    send_with_retry(&msg, send_request, FIRMWARE_MAX_RETRIES)
        .map_err(|e| format!("Unlock firmware failed: {}", e))?;

    progress(total_blocks, total_blocks, "Firmware upload complete");
    Ok(())
}

fn send_with_retry<F>(
    msg: &[u8],
    send_request: &mut F,
    max_retries: usize,
) -> Result<Vec<u8>, String>
where
    F: FnMut(&[u8]) -> Result<Vec<u8>, String>,
{
    let mut last_err = String::new();
    for _ in 0..max_retries {
        match send_request(msg) {
            Ok(reply) => return Ok(reply),
            Err(e) => {
                last_err = e;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_sequence() {
        // Create a small firmware image (400 bytes = 2 blocks)
        let image = FirmwareImage {
            data: vec![0xAA; 400],
            base_address: 0x8000,
            entry_point: 0x8000,
            checksum: (0xAAu16).wrapping_mul(400),
        };

        let mut calls: Vec<Vec<u8>> = Vec::new();
        let mut send_request = |msg: &[u8]| -> Result<Vec<u8>, String> {
            calls.push(msg.to_vec());
            // Return a success reply (opcode complement + status 0)
            Ok(vec![0x00, 0x00])
        };

        let progress_calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let pc = progress_calls.clone();
        let progress: ProgressFn = Box::new(move |current, total, phase| {
            pc.lock().unwrap().push((current, total, phase.to_string()));
        });

        let result = upload_firmware(&image, &mut send_request, &progress);
        assert!(result.is_ok(), "Upload failed: {:?}", result);

        // Should have: delete + start + 2 data blocks + unlock = 5 calls
        assert_eq!(calls.len(), 5);

        // Verify opcodes in the framed messages (opcode is at index 3, after 55 FF 00)
        assert_eq!(calls[0][3], OP_DELETE_FIRMWARE);
        assert_eq!(calls[1][3], OP_START_FIRMWARE_DOWNLOAD);
        assert_eq!(calls[2][3], OP_TRANSFER_DATA); // block 1
        assert_eq!(calls[3][3], OP_TRANSFER_DATA); // block 2 (last, index=0)
        assert_eq!(calls[4][3], OP_UNLOCK_FIRMWARE);

        // Block 1 should have index=1, block 2 (last) should have index=0
        // Index is at bytes 5,7 (after opcode complement pair)
        // In framed message: [55 FF 00] [opcode ~opcode] [index_lo ~index_lo] ...
        assert_eq!(calls[2][5], 1); // block 1 index low byte
        assert_eq!(calls[3][5], 0); // last block index = 0
    }

    #[test]
    fn test_upload_retry_on_failure() {
        let image = FirmwareImage {
            data: vec![0xBB; 100],
            base_address: 0x8000,
            entry_point: 0x8000,
            checksum: (0xBBu16).wrapping_mul(100),
        };

        let call_count = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let cc = call_count.clone();
        let mut send_request = move |_msg: &[u8]| -> Result<Vec<u8>, String> {
            let mut count = cc.lock().unwrap();
            *count += 1;
            // Fail first 2 calls (delete retries), succeed on 3rd
            if *count <= 2 {
                Err("timeout".to_string())
            } else {
                Ok(vec![0x00, 0x00])
            }
        };

        let progress: ProgressFn = Box::new(|_, _, _| {});
        let result = upload_firmware(&image, &mut send_request, &progress);
        assert!(result.is_ok());
        // 2 failed delete + 1 success delete + start + 1 block + unlock = 6
        // But retries happen per phase, so delete took 3 attempts
        assert!(*call_count.lock().unwrap() >= 5);
    }

    #[test]
    fn test_upload_gives_up_after_max_retries() {
        let image = FirmwareImage {
            data: vec![0xCC; 100],
            base_address: 0x8000,
            entry_point: 0x8000,
            checksum: (0xCCu16).wrapping_mul(100),
        };

        let mut send_request = |_msg: &[u8]| -> Result<Vec<u8>, String> {
            Err("always fails".to_string())
        };

        let progress: ProgressFn = Box::new(|_, _, _| {});
        let result = upload_firmware(&image, &mut send_request, &progress);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Delete firmware failed"));
    }
}
