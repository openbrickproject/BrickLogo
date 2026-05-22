use super::*;

#[test]
fn test_build_frame_ping_matches_spec() {
    // From the protocol spec worked example:
    // PING: AA 02 5A 01 59
    let frame = build_frame(0x5A, 0x01, &[]);
    assert_eq!(frame, &[0xAA, 0x02, 0x5A, 0x01, 0x59]);
}

#[test]
fn test_build_frame_set_outputs_matches_spec() {
    // From the spec: set outputs 0 and 2 full-on, others off (6-byte form)
    // AA 08 10 10 FF 00 FF 00 00 00 08
    let payload = [0xFF, 0x00, 0xFF, 0x00, 0x00, 0x00];
    let frame = build_frame(0x10, 0x10, &payload);
    assert_eq!(
        frame,
        &[0xAA, 0x08, 0x10, 0x10, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0x00, 0x08]
    );
}

#[test]
fn test_build_frame_checksum_changes_with_payload() {
    let f1 = build_frame(0x01, 0x10, &[0xFF, 0x00, 0x00, 0x00, 0x00, 0x00]);
    let f2 = build_frame(0x01, 0x10, &[0xFE, 0x00, 0x00, 0x00, 0x00, 0x00]);
    assert_ne!(f1.last(), f2.last(), "CHK should differ when payload byte changes");
}

#[test]
fn test_parse_frame_valid_roundtrip() {
    let frame = build_frame(0x5A, 0x81, &[]); // PONG reply
    let (cmd, payload, consumed) = try_parse_frame(&frame).unwrap();
    assert_eq!(cmd, 0x81);
    assert!(payload.is_empty());
    assert_eq!(consumed, frame.len());
}

#[test]
fn test_parse_frame_with_payload() {
    let counts = [5u8, 0, 0, 0, 10, 0, 0, 0];
    let frame = build_frame(0x01, 0x91, &counts);
    let (cmd, payload, consumed) = try_parse_frame(&frame).unwrap();
    assert_eq!(cmd, 0x91);
    assert_eq!(payload, counts);
    assert_eq!(consumed, frame.len());
}

#[test]
fn test_parse_frame_skips_leading_garbage() {
    let mut buf = vec![0x00, 0xFF, 0x55];
    buf.extend(build_frame(0x01, 0x84, &[]));
    let (cmd, _, consumed) = try_parse_frame(&buf).unwrap();
    assert_eq!(cmd, 0x84);
    assert_eq!(consumed, buf.len());
}

#[test]
fn test_parse_frame_bad_checksum_skipped() {
    let mut frame = build_frame(0x01, 0x84, &[]);
    *frame.last_mut().unwrap() ^= 0xFF; // corrupt CHK
    frame.extend(build_frame(0x02, 0x81, &[]));
    let (cmd, _, _) = try_parse_frame(&frame).unwrap();
    assert_eq!(cmd, 0x81, "should skip bad-checksum frame and parse the valid one");
}

#[test]
fn test_parse_frame_incomplete_returns_none() {
    let frame = build_frame(0x01, 0x84, &[]);
    assert!(try_parse_frame(&frame[..frame.len() - 1]).is_none());
}

#[test]
fn test_parse_frame_two_consecutive_parses_first() {
    let f1 = build_frame(0x01, 0x81, &[]);
    let f2 = build_frame(0x02, 0x84, &[]);
    let mut buf = f1.clone();
    buf.extend(&f2);
    let (cmd, _, consumed) = try_parse_frame(&buf).unwrap();
    assert_eq!(cmd, 0x81);
    assert_eq!(consumed, f1.len());
}

#[test]
fn test_parse_frame_invalid_len_skipped() {
    let mut buf = vec![0xAA, 0x01, 0xFF, 0xFF]; // LEN=1, below minimum
    buf.extend(build_frame(0x02, 0x84, &[]));
    let (cmd, _, _) = try_parse_frame(&buf).unwrap();
    assert_eq!(cmd, 0x84);
}
