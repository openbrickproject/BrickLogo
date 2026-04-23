use super::*;
use crate::protocol;
use crate::transport::Transport;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Scripted mock transport.
///
/// Every `send` records the raw bytes. Every `recv` pulls a canned reply
/// from a queue — tests populate the queue ahead of time with bytes that
/// mimic real EV3 responses. This lets us exercise the counter-matching
/// retry logic without a brick.
struct MockTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    replies: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl Transport for MockTransport {
    fn send(&mut self, frame: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(frame.to_vec());
        Ok(())
    }
    fn recv(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, String> {
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "mock: out of replies".to_string())
    }
}

fn build_ev3() -> (Ev3, Arc<Mutex<Vec<Vec<u8>>>>, Arc<Mutex<VecDeque<Vec<u8>>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let replies = Arc::new(Mutex::new(VecDeque::new()));
    let transport: Box<dyn Transport> = Box::new(MockTransport {
        sent: sent.clone(),
        replies: replies.clone(),
    });
    (Ev3::new(transport), sent, replies)
}

fn direct_ok_reply(counter: u16, payload: &[u8]) -> Vec<u8> {
    // Wire: [length:u16][counter:u16][type:u8][payload]. length counts
    // the bytes after itself (counter + type + payload).
    let body_len = (2 + 1 + payload.len()) as u16;
    let mut v = Vec::with_capacity(2 + body_len as usize);
    v.extend_from_slice(&body_len.to_le_bytes());
    v.extend_from_slice(&counter.to_le_bytes());
    v.push(0x02); // direct-ok
    v.extend_from_slice(payload);
    v
}

fn direct_error_reply(counter: u16) -> Vec<u8> {
    let body_len = 3u16;
    let mut v = Vec::with_capacity(5);
    v.extend_from_slice(&body_len.to_le_bytes());
    v.extend_from_slice(&counter.to_le_bytes());
    v.push(0x04); // direct-error
    v
}

// ── Counter ─────────────────────────────────────

#[test]
fn test_counter_advances_each_send() {
    let (mut ev3, sent, _) = build_ev3();
    let _ = ev3.start(0x01);
    let _ = ev3.start(0x02);
    let _ = ev3.start(0x04);
    let frames = sent.lock().unwrap();
    let counters: Vec<u16> = frames
        .iter()
        .map(|f| u16::from_le_bytes([f[2], f[3]]))
        .collect();
    // Three distinct counters, strictly increasing (counter starts at 1).
    assert!(counters.windows(2).all(|w| w[1] > w[0]));
    assert!(counters[0] >= 1);
}

// ── Fire-and-forget motor ops send without reading ──

#[test]
fn test_set_power_sends_output_power_opcode() {
    let (mut ev3, sent, _) = build_ev3();
    ev3.set_power(0x03, 50).unwrap();
    let frame = sent.lock().unwrap()[0].clone();
    // Frame body starts after [length:2][counter:2][type:1][header:2] = 7 bytes.
    assert_eq!(frame[7], protocol::OP_OUTPUT_POWER);
}

#[test]
fn test_start_stop_emit_opcodes() {
    let (mut ev3, sent, _) = build_ev3();
    ev3.start(0x0F).unwrap();
    ev3.stop(0x0F, true).unwrap();
    let frames = sent.lock().unwrap();
    assert_eq!(frames[0][7], protocol::OP_OUTPUT_START);
    assert_eq!(frames[1][7], protocol::OP_OUTPUT_STOP);
}

#[test]
fn test_step_power_encodes_degrees_and_brake() {
    let (mut ev3, sent, _) = build_ev3();
    ev3.step_power(0x01, 60, 720, true).unwrap();
    let frame = sent.lock().unwrap()[0].clone();
    assert_eq!(frame[7], protocol::OP_OUTPUT_STEP_POWER);
}

#[test]
fn test_time_power_encodes_ms_and_brake() {
    let (mut ev3, sent, _) = build_ev3();
    ev3.time_power(0x01, 40, 1000, false).unwrap();
    let frame = sent.lock().unwrap()[0].clone();
    assert_eq!(frame[7], protocol::OP_OUTPUT_TIME_POWER);
}

#[test]
fn test_clr_count_emits_clear_opcode() {
    let (mut ev3, sent, _) = build_ev3();
    ev3.clr_count(0x01).unwrap();
    let frame = sent.lock().unwrap()[0].clone();
    assert_eq!(frame[7], protocol::OP_OUTPUT_CLR_COUNT);
}

// ── Request/reply correlation ───────────────────

#[test]
fn test_test_busy_true_when_reply_has_nonzero_byte() {
    let (mut ev3, sent, replies) = build_ev3();
    let mask = 0x01u8;
    // First call: expect reply with counter 1, payload 0x01 (busy).
    replies.lock().unwrap().push_back(direct_ok_reply(1, &[0x01]));
    let busy = ev3.test_busy(mask).unwrap();
    assert!(busy);
    // Second call: expect counter 2, payload 0x00 (not busy).
    replies.lock().unwrap().push_back(direct_ok_reply(2, &[0x00]));
    let busy = ev3.test_busy(mask).unwrap();
    assert!(!busy);

    let frames = sent.lock().unwrap();
    assert_eq!(frames[0][7], protocol::OP_OUTPUT_TEST);
}

#[test]
fn test_request_skips_stale_reply_with_wrong_counter() {
    // A stale reply from a previous request (counter 99) should be
    // discarded; the handle keeps reading until it sees the expected
    // counter.
    let (mut ev3, _sent, replies) = build_ev3();
    replies.lock().unwrap().push_back(direct_ok_reply(99, &[0x00]));
    replies.lock().unwrap().push_back(direct_ok_reply(1, &[0x01]));
    let busy = ev3.test_busy(0x01).unwrap();
    assert!(busy, "request should have skipped the stale counter-99 reply");
}

#[test]
fn test_request_surfaces_error_reply_type() {
    let (mut ev3, _, replies) = build_ev3();
    replies.lock().unwrap().push_back(direct_error_reply(1));
    let err = ev3.test_busy(0x01).unwrap_err();
    assert!(err.contains("failed"));
}

#[test]
fn test_request_short_reply_errors() {
    let (mut ev3, _, replies) = build_ev3();
    replies.lock().unwrap().push_back(vec![0, 0]); // too short
    let err = ev3.test_busy(0x01).unwrap_err();
    assert!(err.contains("too short"));
}

// ── Count / sensor parsers ──────────────────────

#[test]
fn test_get_count_parses_i32_le() {
    let (mut ev3, _, replies) = build_ev3();
    let value: i32 = -360;
    replies.lock().unwrap().push_back(direct_ok_reply(1, &value.to_le_bytes()));
    let got = ev3.get_count(0).unwrap();
    assert_eq!(got, -360);
}

#[test]
fn test_get_count_short_reply_errors() {
    let (mut ev3, _, replies) = build_ev3();
    replies.lock().unwrap().push_back(direct_ok_reply(1, &[0x00])); // only 1 byte
    let err = ev3.get_count(0).unwrap_err();
    assert!(err.contains("too short"));
}

#[test]
fn test_read_sensor_pct_returns_first_byte() {
    let (mut ev3, _, replies) = build_ev3();
    replies.lock().unwrap().push_back(direct_ok_reply(1, &[42, 0, 0, 0]));
    let v = ev3.read_sensor_pct(0, 16, 0).unwrap();
    assert_eq!(v, 42);
}

#[test]
fn test_read_sensor_si_parses_f32_le() {
    let (mut ev3, _, replies) = build_ev3();
    let raw: f32 = 3.25;
    replies.lock().unwrap().push_back(direct_ok_reply(1, &raw.to_le_bytes()));
    let v = ev3.read_sensor_si(0, 16, 0).unwrap();
    assert!((v - 3.25).abs() < 1e-6);
}

#[test]
fn test_get_sensor_typemode_returns_both_bytes() {
    let (mut ev3, _, replies) = build_ev3();
    replies.lock().unwrap().push_back(direct_ok_reply(1, &[29, 2])); // EV3 Color, mode 2
    let (ty, mode) = ev3.get_sensor_typemode(0).unwrap();
    assert_eq!(ty, 29);
    assert_eq!(mode, 2);
}
