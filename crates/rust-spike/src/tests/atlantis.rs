use super::*;

#[test]
fn test_info_request() {
    assert_eq!(info_request(), vec![ID_INFO_REQUEST]);
}

#[test]
fn test_clear_slot_request() {
    assert_eq!(clear_slot_request(3), vec![ID_CLEAR_SLOT_REQUEST, 3]);
}

#[test]
fn test_start_file_upload_request_layout() {
    let msg = start_file_upload_request("program.py", 0, 0xDEADBEEF);
    assert_eq!(msg[0], ID_START_FILE_UPLOAD_REQUEST);
    assert_eq!(&msg[1..11], b"program.py");
    assert_eq!(msg[11], 0x00);
    assert_eq!(msg[12], 0);
    assert_eq!(&msg[13..17], &0xDEADBEEFu32.to_le_bytes());
    assert_eq!(msg.len(), 17);
}

#[test]
fn test_transfer_chunk_request_layout() {
    let msg = transfer_chunk_request(0x12345678, &[0x41, 0x42, 0x43]);
    assert_eq!(msg[0], ID_TRANSFER_CHUNK_REQUEST);
    assert_eq!(&msg[1..5], &0x12345678u32.to_le_bytes());
    assert_eq!(&msg[5..7], &3u16.to_le_bytes());
    assert_eq!(&msg[7..10], &[0x41, 0x42, 0x43]);
}

#[test]
fn test_program_flow_request() {
    assert_eq!(program_flow_request(false, 0), vec![ID_PROGRAM_FLOW_REQUEST, 0, 0]);
    assert_eq!(program_flow_request(true, 2), vec![ID_PROGRAM_FLOW_REQUEST, 1, 2]);
}

#[test]
fn test_device_notification_request() {
    let msg = device_notification_request(5000);
    assert_eq!(msg[0], ID_DEVICE_NOTIFICATION_REQUEST);
    assert_eq!(&msg[1..3], &5000u16.to_le_bytes());
}

#[test]
fn test_parse_info_response_full_format() {
    // id 0x01, rpc 1.2 build 0x0003, fw 3.4 build 0x0005,
    // max_packet 0x00FF, max_msg 0x0200, max_chunk 0x0100, product 0x0007
    let bytes: Vec<u8> = vec![
        0x01, 0x01, 0x02, 0x03, 0x00, 0x03, 0x04, 0x05, 0x00, 0xFF, 0x00, 0x00, 0x02,
        0x00, 0x01, 0x07, 0x00,
    ];
    match parse(&bytes).unwrap() {
        Message::InfoResponse(info) => {
            assert_eq!(info.rpc_major, 1);
            assert_eq!(info.rpc_minor, 2);
            assert_eq!(info.rpc_build, 3);
            assert_eq!(info.firmware_major, 3);
            assert_eq!(info.firmware_minor, 4);
            assert_eq!(info.firmware_build, 5);
            assert_eq!(info.max_packet_size, 0x00FF);
            assert_eq!(info.max_message_size, 0x0200);
            assert_eq!(info.max_chunk_size, 0x0100);
            assert_eq!(info.product_group_device, 0x0007);
        }
        other => panic!("wrong variant: {:?}", other),
    }
}

#[test]
fn test_parse_status_responses() {
    assert_eq!(
        parse(&[ID_START_FILE_UPLOAD_RESPONSE, 0x00]).unwrap(),
        Message::StartFileUploadResponse { success: true }
    );
    assert_eq!(
        parse(&[ID_START_FILE_UPLOAD_RESPONSE, 0x01]).unwrap(),
        Message::StartFileUploadResponse { success: false }
    );
    assert_eq!(
        parse(&[ID_TRANSFER_CHUNK_RESPONSE, 0x00]).unwrap(),
        Message::TransferChunkResponse { success: true }
    );
    assert_eq!(
        parse(&[ID_CLEAR_SLOT_RESPONSE, 0x00]).unwrap(),
        Message::ClearSlotResponse { success: true }
    );
    assert_eq!(
        parse(&[ID_DEVICE_NOTIFICATION_RESPONSE, 0x00]).unwrap(),
        Message::DeviceNotificationResponse { success: true }
    );
}

#[test]
fn test_parse_program_flow_notification() {
    assert_eq!(
        parse(&[ID_PROGRAM_FLOW_NOTIFICATION, 0x01]).unwrap(),
        Message::ProgramFlowNotification { stop: true }
    );
    assert_eq!(
        parse(&[ID_PROGRAM_FLOW_NOTIFICATION, 0x00]).unwrap(),
        Message::ProgramFlowNotification { stop: false }
    );
}

#[test]
fn test_parse_console_notification_trims_nuls() {
    let mut bytes = vec![ID_CONSOLE_NOTIFICATION];
    bytes.extend_from_slice(b"hello\0\0\0");
    match parse(&bytes).unwrap() {
        Message::ConsoleNotification { text } => assert_eq!(text, "hello"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_parse_device_notification() {
    let mut bytes = vec![ID_DEVICE_NOTIFICATION, 0x05, 0x00];
    bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    match parse(&bytes).unwrap() {
        Message::DeviceNotification { payload } => {
            assert_eq!(payload, vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_parse_tunnel_plain() {
    // Hub→host format observed on-wire: [0x32, size_u16_LE, payload]
    let mut bytes = vec![ID_TUNNEL_MESSAGE, 0x06, 0x00];
    bytes.extend_from_slice(b"hello\n");
    match parse(&bytes).unwrap() {
        Message::Tunnel { payload } => assert_eq!(payload, b"hello\n"),
        other => panic!("wrong variant: {:?}", other),
    }
}

#[test]
fn test_tunnel_message_plain_layout() {
    let data = b"{\"op\":\"ping\"}\n";
    let msg = tunnel_message(data);
    assert_eq!(msg[0], ID_TUNNEL_MESSAGE);
    let size = u16::from_le_bytes([msg[1], msg[2]]) as usize;
    assert_eq!(size, data.len());
    assert_eq!(&msg[3..], data);
}

#[test]
fn test_parse_unknown() {
    match parse(&[0xAB, 0x01, 0x02]).unwrap() {
        Message::Unknown { id, payload } => {
            assert_eq!(id, 0xAB);
            assert_eq!(payload, vec![0x01, 0x02]);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_crc32_padded_matches_padding() {
    // Unpadded zlib CRC over 'hi' would differ from the hub's padded one.
    let padded = crc32_padded(b"hi", 0);
    let expected = {
        let mut h = crc32fast::Hasher::new();
        h.update(b"hi");
        h.update(&[0u8, 0u8]); // pad to 4 bytes
        h.finalize()
    };
    assert_eq!(padded, expected);
}

#[test]
fn test_running_crc_aligns_per_chunk() {
    let mut rc = RunningCrc::new();
    rc.update(b"hi");
    let after_first = rc.finalize();

    let mut rc2 = RunningCrc::new();
    rc2.update(b"hi");
    rc2.update(b"there");
    let after_second = rc2.finalize();

    assert_ne!(after_first, after_second);
    // Different from plain crc32 without padding.
    let plain = crc32_padded(b"hithere", 0);
    assert_ne!(after_second, plain);
}
