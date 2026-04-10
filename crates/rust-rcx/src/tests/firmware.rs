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
