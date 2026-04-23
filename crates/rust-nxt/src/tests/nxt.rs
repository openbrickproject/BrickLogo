use super::*;
use crate::protocol as p;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Scripted `Transport` that records every outgoing LCP packet and replays
/// a canned reply for each `recv`. Tests push replies into the queue in
/// the order the handle is expected to read them.
struct MockTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    replies: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

type SentLog = Arc<Mutex<Vec<Vec<u8>>>>;
type ReplyQueue = Arc<Mutex<VecDeque<Vec<u8>>>>;

impl MockTransport {
    fn new() -> (Self, SentLog, ReplyQueue) {
        let sent: SentLog = Arc::new(Mutex::new(Vec::new()));
        let replies: ReplyQueue = Arc::new(Mutex::new(VecDeque::new()));
        (
            MockTransport {
                sent: sent.clone(),
                replies: replies.clone(),
            },
            sent,
            replies,
        )
    }
}

impl Transport for MockTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
    fn recv(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, String> {
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "mock transport: no more replies".to_string())
    }
}

fn push_reply(replies: &ReplyQueue, bytes: Vec<u8>) {
    replies.lock().unwrap().push_back(bytes);
}

#[test]
fn test_get_firmware_version_sends_sys_command_and_parses_reply() {
    let (mt, sent, replies) = MockTransport::new();
    push_reply(&replies, vec![
        p::TYPE_REPLY,
        p::SYS_GET_FIRMWARE_VERSION,
        0x00,
        0x7C,  // protocol minor
        0x01,  // protocol major
        0x05,  // fw minor
        0x01,  // fw major
    ]);
    let mut nxt = Nxt::new(Box::new(mt));

    let v = nxt.get_firmware_version().unwrap();
    assert_eq!(v, (1, 0x7C, 1, 0x05));
    assert_eq!(sent.lock().unwrap()[0], vec![p::TYPE_SYSTEM, p::SYS_GET_FIRMWARE_VERSION]);
}

#[test]
fn test_set_output_state_no_reply_skips_recv() {
    let (mt, sent, _replies) = MockTransport::new();
    let mut nxt = Nxt::new(Box::new(mt));
    nxt.set_output_state(&OutputStateSpec {
        port: 0,
        power: 50,
        mode: p::MODE_MOTORON,
        regulation: p::REG_IDLE,
        turn_ratio: 0,
        run_state: p::RUN_RUNNING,
        tacho_limit: 0,
        reply_required: false,
    })
    .unwrap();
    let packet = &sent.lock().unwrap()[0];
    assert_eq!(packet[0], p::TYPE_DIRECT | p::NO_REPLY_FLAG);
    assert_eq!(packet[1], p::OP_SET_OUTPUT_STATE);
}

#[test]
fn test_request_skips_stray_reply_with_wrong_opcode() {
    let (mt, _sent, replies) = MockTransport::new();
    // Stale reply for a motor command, then the real keep-alive reply.
    push_reply(&replies, vec![p::TYPE_REPLY, p::OP_SET_OUTPUT_STATE, 0x00]);
    push_reply(&replies, vec![p::TYPE_REPLY, p::OP_KEEP_ALIVE, 0x00]);
    let mut nxt = Nxt::new(Box::new(mt));
    // Should succeed — the handle drops the stale frame and reads the next.
    nxt.keep_alive().unwrap();
}

#[test]
fn test_get_input_values_round_trip() {
    let (mt, _sent, replies) = MockTransport::new();
    let mut reply = vec![p::TYPE_REPLY, p::OP_GET_INPUT_VALUES, 0x00];
    reply.push(0); // port
    reply.push(1); // valid
    reply.push(0); // calibrated
    reply.push(0x01); // SWITCH
    reply.push(0x20); // BOOLEAN
    reply.extend_from_slice(&900u16.to_le_bytes());
    reply.extend_from_slice(&900u16.to_le_bytes());
    reply.extend_from_slice(&1i16.to_le_bytes());
    reply.extend_from_slice(&1i16.to_le_bytes());
    push_reply(&replies, reply);

    let mut nxt = Nxt::new(Box::new(mt));
    let v = nxt.get_input_values(0).unwrap();
    assert_eq!(v.scaled, 1);
    assert_eq!(v.sensor_type, 0x01);
}

#[test]
fn test_stop_program_tolerates_no_active_program_status() {
    let (mt, _sent, replies) = MockTransport::new();
    push_reply(&replies, vec![p::TYPE_REPLY, p::OP_STOP_PROGRAM, 0xEC]);
    let mut nxt = Nxt::new(Box::new(mt));
    nxt.stop_program().unwrap();
}

#[test]
fn test_stop_program_surfaces_other_errors() {
    let (mt, _sent, replies) = MockTransport::new();
    push_reply(&replies, vec![p::TYPE_REPLY, p::OP_STOP_PROGRAM, 0xBF]);
    let mut nxt = Nxt::new(Box::new(mt));
    let err = nxt.stop_program().unwrap_err();
    assert!(err.contains("0xBF"));
}
