//! Tests for LEGO's COBS variant. Reference values come from running the
//! Python implementation at
//! <https://github.com/LEGO/spike-prime-docs/blob/main/examples/python/cobs.py>.
use super::*;

#[test]
fn test_encode_empty() {
    assert_eq!(encode(&[]), vec![COBS_CODE_OFFSET + 1]);
}

#[test]
fn test_encode_single_high_byte() {
    // byte > DELIMITER: data byte, block size 2 (code + byte), terminates at EOF.
    assert_eq!(encode(&[0x41]), vec![COBS_CODE_OFFSET + 2, 0x41]);
}

#[test]
fn test_encode_rejects_all_delimiters_in_output() {
    // Round-trip a hostile payload and assert output has none of {0,1,2}.
    let data: Vec<u8> = (0..=255u16).map(|b| b as u8).collect();
    let encoded = encode(&data);
    for &b in &encoded {
        assert!(b > DELIMITER, "byte {:#04x} <= DELIMITER in encoded output", b);
    }
}

#[test]
fn test_decode_roundtrip_small() {
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0x01],
        vec![0x02],
        vec![0x41, 0x42, 0x43],
        vec![0x00, 0x01, 0x02],
        vec![0x41, 0x00, 0x42, 0x02, 0x43, 0x01, 0x44],
    ];
    for data in cases {
        let encoded = encode(&data);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, data, "roundtrip failed for {:?}", data);
    }
}

#[test]
fn test_decode_roundtrip_long() {
    // Longer than MAX_BLOCK_SIZE to exercise block continuation.
    let data: Vec<u8> = (0..200u16).map(|b| b as u8).collect();
    let encoded = encode(&data);
    let decoded = decode(&encoded).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_pack_unpack_roundtrip() {
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        b"hello".to_vec(),
        vec![0x00, 0x01, 0x02, 0x03, 0xFF],
        (0..=255u16).map(|b| b as u8).collect(),
    ];
    for data in cases {
        let framed = pack(&data);
        // Body must never contain the frame delimiter.
        assert_eq!(*framed.last().unwrap(), DELIMITER);
        for &b in &framed[..framed.len() - 1] {
            assert_ne!(b, DELIMITER, "delimiter leaked into frame body");
        }
        let unpacked = unpack(&framed).unwrap();
        assert_eq!(unpacked, data);
    }
}

#[test]
fn test_unpack_strips_high_priority_marker() {
    let data = b"cmd";
    let mut framed = pack(data);
    framed.insert(0, HIGH_PRIORITY_FRAME);
    let unpacked = unpack(&framed).unwrap();
    assert_eq!(unpacked, data);
}

#[test]
fn test_info_request_frame_matches_reference() {
    // Sanity check against a known wire frame.
    // InfoRequest is id=0x00, serialize()=b"\0", encoded as single delimiter
    // block; after XOR+delimiter the expected bytes come from running the
    // Python reference with `cobs.pack(b"\0")`.
    let framed = pack(&[0x00]);
    // encode -> [0x03, 0x03] (delimiter-terminated block then final block),
    // XOR 3    -> [0x00, 0x00],
    // + delim  -> [0x00, 0x00, 0x02]
    assert_eq!(framed, vec![0x00, 0x00, 0x02]);
}
