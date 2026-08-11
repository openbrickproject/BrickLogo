use super::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// A transport that plays back a scripted reply for each command it sees
/// written, queuing the reply bytes only once that exact command has been
/// written — so two replies can never appear in the same `read_available`
/// unless the test scripts it that way. The inbox is shared so tests can
/// also inject bytes (e.g. unsolicited input reports) after the transport
/// has been moved into a `Device`.
struct MockTransport {
    written: Arc<Mutex<Vec<Vec<u8>>>>,
    responses: HashMap<Vec<u8>, Vec<u8>>,
    inbox: Arc<Mutex<VecDeque<u8>>>,
}

impl MockTransport {
    fn new(
        responses: &[(&[u8], &[u8])],
    ) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>, Arc<Mutex<VecDeque<u8>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));
        let inbox = Arc::new(Mutex::new(VecDeque::new()));
        let responses = responses
            .iter()
            .map(|(cmd, reply)| (cmd.to_vec(), reply.to_vec()))
            .collect();
        (
            MockTransport { written: written.clone(), responses, inbox: inbox.clone() },
            written,
            inbox,
        )
    }
}

impl Transport for MockTransport {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.written.lock().unwrap().push(bytes.to_vec());
        if let Some(reply) = self.responses.get(bytes) {
            self.inbox.lock().unwrap().extend(reply.iter().copied());
        }
        Ok(())
    }

    fn read_available(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.inbox.lock().unwrap().drain(..).collect())
    }
}

fn handshake_transport() -> (MockTransport, Arc<Mutex<Vec<Vec<u8>>>>, Arc<Mutex<VecDeque<u8>>>) {
    MockTransport::new(&[(CMD_PROBE, b"CF\r\n"), (CMD_VERSION, b"V1.0.1\r\n")])
}

#[test]
fn test_connect_probes_then_queries_version() {
    let (transport, written, _) = handshake_transport();
    let device = Device::connect(Box::new(transport)).unwrap();
    assert_eq!(device.firmware_version(), "V1.0.1");

    let written = written.lock().unwrap();
    assert_eq!(written[0], CMD_PROBE);
    assert_eq!(written[1], CMD_VERSION);
}

#[test]
fn test_connect_fails_on_wrong_probe_reply() {
    let (transport, _, _) = MockTransport::new(&[(CMD_PROBE, b"NO\r\n")]);
    assert!(Device::connect(Box::new(transport)).is_err());
}

#[test]
fn test_connect_times_out_with_no_reply() {
    let (transport, _, _) = MockTransport::new(&[]);
    match Device::connect(Box::new(transport)) {
        Err(e) => assert!(e.contains("Timed out")),
        Ok(_) => panic!("expected a timeout error"),
    }
}

#[test]
fn test_probe_absorbs_input_report_seen_while_waiting() {
    // An input report arriving interleaved with the probe reply must be
    // captured, not dropped or mistaken for the reply itself — but it was
    // already consumed by the handshake, so a subsequent drain finds
    // nothing new.
    let (transport, _, _) = MockTransport::new(&[
        (CMD_PROBE, b"I40\r\nCF\r\n"),
        (CMD_VERSION, b"V1.0.1\r\n"),
    ]);
    let mut device = Device::connect(Box::new(transport)).unwrap();
    assert_eq!(device.last_input(), Some(0x40));
    assert_eq!(device.read_pending_input().unwrap(), Vec::<u8>::new());
}

#[test]
fn test_set_outputs_writes_one_frame() {
    let (transport, written, _) = handshake_transport();
    let mut device = Device::connect(Box::new(transport)).unwrap();
    device.set_outputs(0x3F).unwrap();
    let written = written.lock().unwrap();
    assert_eq!(written.last().unwrap(), b"D3F\n");
}

#[test]
fn test_write_batch_writes_one_serial_write_with_all_frames() {
    let (transport, written, _) = handshake_transport();
    let mut device = Device::connect(Box::new(transport)).unwrap();
    device.write_batch(&[0x01, 0x00, 0x01]).unwrap();
    let written = written.lock().unwrap();
    assert_eq!(written.last().unwrap(), b"D01\nD00\nD01\n");
}

#[test]
fn test_read_pending_input_returns_every_mask_in_order() {
    let (transport, _, inbox) = handshake_transport();
    let mut device = Device::connect(Box::new(transport)).unwrap();
    // Two input reports that arrived back to back must both come back, in
    // arrival order — not just the latest one.
    inbox.lock().unwrap().extend(b"I40\nI80\n".iter().copied());
    assert_eq!(device.read_pending_input().unwrap(), vec![0x40, 0x80]);
    assert_eq!(device.last_input(), Some(0x80));
}

#[test]
fn test_read_pending_input_press_and_release_in_one_drain_both_reported() {
    // The device only sends `Ixx` on a state change, so a press and a
    // release that both land inside one drain (e.g. a fast tap within a
    // single ~16ms scheduler tick) produce two lines that must both be
    // surfaced — collapsing to just the final (released) mask would lose
    // the edge `counter` exists to catch.
    let (transport, _, inbox) = handshake_transport();
    let mut device = Device::connect(Box::new(transport)).unwrap();
    inbox.lock().unwrap().extend(b"I40\nI00\n".iter().copied());
    assert_eq!(device.read_pending_input().unwrap(), vec![0x40, 0x00]);
    assert_eq!(device.last_input(), Some(0x00));
}

#[test]
fn test_read_pending_input_empty_before_any_report() {
    let (transport, _, _) = handshake_transport();
    let mut device = Device::connect(Box::new(transport)).unwrap();
    assert_eq!(device.read_pending_input().unwrap(), Vec::<u8>::new());
    assert_eq!(device.last_input(), None);
}

#[test]
fn test_read_pending_input_empty_when_nothing_new_but_last_input_persists() {
    let (transport, _, inbox) = handshake_transport();
    let mut device = Device::connect(Box::new(transport)).unwrap();
    inbox.lock().unwrap().extend(b"I40\n".iter().copied());
    assert_eq!(device.read_pending_input().unwrap(), vec![0x40]);
    // No new bytes queued — nothing to drain, but the last known mask
    // (queried via `last_input`) still reflects it.
    assert_eq!(device.read_pending_input().unwrap(), Vec::<u8>::new());
    assert_eq!(device.last_input(), Some(0x40));
}
