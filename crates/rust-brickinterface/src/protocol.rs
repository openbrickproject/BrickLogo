/// Build a BrickInterface framed command.
///
/// Frame layout: SOF(1) LEN(1) SEQ(1) CMD(1) PAYLOAD(LEN-2) CHK(1)
/// LEN = 2 + payload.len()
/// CHK = XOR of LEN, SEQ, CMD, and all payload bytes.
pub fn build_frame(seq: u8, cmd: u8, payload: &[u8]) -> Vec<u8> {
    let len = (2 + payload.len()) as u8;
    let mut chk = len ^ seq ^ cmd;
    for &b in payload {
        chk ^= b;
    }
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.push(0xAA);
    frame.push(len);
    frame.push(seq);
    frame.push(cmd);
    frame.extend_from_slice(payload);
    frame.push(chk);
    frame
}

/// Try to parse one complete, valid frame from `buf`.
/// Returns `(reply_cmd, payload, consumed_bytes)` on success, `None` if
/// the buffer does not yet contain a complete valid frame.
/// Silently discards bytes before the SOF and frames with bad checksums,
/// matching the firmware's receiver state machine.
pub fn try_parse_frame(buf: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    let mut i = 0;
    while i < buf.len() {
        if buf[i] != 0xAA {
            i += 1;
            continue;
        }
        if buf.len() < i + 3 {
            return None;
        }
        let len = buf[i + 1] as usize;
        if len < 2 || len > 34 {
            i += 1;
            continue;
        }
        // Total bytes: SOF(1) + LEN(1) + len + CHK(1)
        let total = i + 3 + len;
        if buf.len() < total {
            return None;
        }
        let mut chk = 0u8;
        for &b in &buf[i + 1..i + 2 + len] {
            chk ^= b;
        }
        if chk != buf[i + 2 + len] {
            i += 1;
            continue;
        }
        let cmd = buf[i + 3];
        let payload = buf[i + 4..i + 2 + len].to_vec();
        return Some((cmd, payload, total));
    }
    None
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
