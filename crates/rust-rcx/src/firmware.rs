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
#[path = "tests/firmware.rs"]
mod tests;
