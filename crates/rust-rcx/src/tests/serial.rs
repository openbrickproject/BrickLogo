use super::*;

#[test]
fn test_find_reply_after_echo() {
    // Simulate: sent 7 bytes (alive), echo comes back, then reply
    let sent = protocol::cmd_alive();
    let reply_op: u8 = !OP_ALIVE;
    let checksum = reply_op;
    let mut data = sent.clone();
    data.extend_from_slice(&[0x55, 0xFF, 0x00, reply_op, !reply_op, checksum, !checksum]);

    let payload = find_reply_after_echo(&data, sent.len());
    assert!(payload.is_some());
    assert_eq!(payload.unwrap()[0], reply_op);
}

#[test]
fn test_find_reply_no_reply_yet() {
    let sent = protocol::cmd_alive();
    assert!(find_reply_after_echo(&sent, sent.len()).is_none());
}
