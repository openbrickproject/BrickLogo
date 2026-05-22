use super::*;
use crate::adapter::{HardwareAdapter, PortDirection};
use crate::shared_brick_interface::SharedBrickInterface;
use rust_brickinterface::{BrickInterface, BrickInterfaceTransport};
use rust_brickinterface::protocol::build_frame;
use rust_brickinterface::constants::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// ── Mock transport ────────────────────────────────────────────────────────────

struct MockTransport {
    written: Arc<Mutex<Vec<u8>>>,
    responses: Arc<Mutex<VecDeque<u8>>>,
}

impl MockTransport {
    fn new() -> (Self, Arc<Mutex<Vec<u8>>>, Arc<Mutex<VecDeque<u8>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        (
            MockTransport { written: written.clone(), responses: responses.clone() },
            written,
            responses,
        )
    }
}

impl BrickInterfaceTransport for MockTransport {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        let mut q = self.responses.lock().unwrap();
        let n = buf.len().min(q.len());
        for b in buf[..n].iter_mut() {
            *b = q.pop_front().unwrap();
        }
        Ok(n)
    }
    fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.written.lock().unwrap().extend_from_slice(data);
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> { Ok(()) }
}

fn enqueue(responses: &Arc<Mutex<VecDeque<u8>>>, cmd: u8, payload: &[u8]) {
    let frame = build_frame(0x00, cmd, payload);
    responses.lock().unwrap().extend(frame);
}

fn enqueue_ok(responses: &Arc<Mutex<VecDeque<u8>>>) {
    enqueue(responses, REPLY_OK, &[]);
}

fn make_adapter() -> (InterfaceAAdapter, Arc<Mutex<Vec<u8>>>, Arc<Mutex<VecDeque<u8>>>) {
    let (transport, written, responses) = MockTransport::new();
    let shared = SharedBrickInterface::new(BrickInterface::from_transport(Box::new(transport)));
    (InterfaceAAdapter::new_with_shared(shared), written, responses)
}

// Pull CMD and payload out of the raw bytes the adapter wrote.
fn written_cmd(written: &[u8]) -> u8 { written[3] }
fn written_payload(written: &[u8]) -> Vec<u8> {
    let len = written[1] as usize;
    written[4..2 + len].to_vec()
}

// ── Output port behaviour ─────────────────────────────────────────────────────

#[test]
fn test_start_port_sends_masked_set_outputs() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);

    adapter.start_port("0", PortDirection::Even, 200).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_IFACE_SET_OUTPUTS);
    let p = written_payload(&w);
    assert_eq!(p, &[0x01, 200], "mask=0x01 then single duty byte");
}

#[test]
fn test_start_port_direction_is_ignored() {
    let (mut a1, w1, r1) = make_adapter();
    let (mut a2, w2, r2) = make_adapter();
    enqueue_ok(&r1);
    enqueue_ok(&r2);

    a1.start_port("3", PortDirection::Even, 128).unwrap();
    a2.start_port("3", PortDirection::Odd, 128).unwrap();

    let b1 = w1.lock().unwrap();
    let b2 = w2.lock().unwrap();
    // Compare payloads (skip SEQ byte at index 2).
    assert_eq!(written_payload(&b1), written_payload(&b2));
}

#[test]
fn test_start_port_sends_only_changed_output() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);
    enqueue_ok(&responses);

    adapter.start_port("0", PortDirection::Even, 100).unwrap();
    adapter.start_port("2", PortDirection::Even, 50).unwrap();

    let all = written.lock().unwrap();
    let first_len = 2 + all[1] as usize + 1;
    let p1 = written_payload(&all);
    let p2 = written_payload(&all[first_len..]);
    assert_eq!(p1, &[0x01, 100], "first frame: mask=0x01, duty=100");
    assert_eq!(p2, &[0x04, 50],  "second frame: mask=0x04, duty=50");
}

#[test]
fn test_stop_port_sends_zero_duty() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses); // start
    enqueue_ok(&responses); // stop

    adapter.start_port("1", PortDirection::Even, 150).unwrap();
    adapter.stop_port("1").unwrap();

    let all = written.lock().unwrap();
    let first_len = 2 + all[1] as usize + 1;
    let p_stop = written_payload(&all[first_len..]);
    assert_eq!(p_stop, &[0x02, 0], "mask=0x02, duty=0");
}

#[test]
fn test_disconnect_zeroes_all_outputs() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses); // start
    enqueue_ok(&responses); // disconnect zero-all

    adapter.start_port("5", PortDirection::Even, 255).unwrap();
    adapter.disconnect();

    let all = written.lock().unwrap();
    let first_len = 2 + all[1] as usize + 1;
    let p_disc = written_payload(&all[first_len..]);
    assert_eq!(p_disc[0], 0x3F, "mask should be all 6 bits");
    assert!(p_disc[1..].iter().all(|&b| b == 0), "all duties should be zero");
}

// ── Sensor (input) behaviour ──────────────────────────────────────────────────

#[test]
fn test_read_sensor_input6_closed_returns_true() {
    let (mut adapter, _, responses) = make_adapter();
    enqueue(&responses, REPLY_IFACE_INPUTS, &[0x00]);
    let val = adapter.read_sensor("6", None).unwrap().unwrap();
    assert_eq!(val.as_string(), "true");
}

#[test]
fn test_read_sensor_input6_open_returns_false() {
    let (mut adapter, _, responses) = make_adapter();
    enqueue(&responses, REPLY_IFACE_INPUTS, &[0x01]); // bit 0 set = open
    let val = adapter.read_sensor("6", None).unwrap().unwrap();
    assert_eq!(val.as_string(), "false");
}

#[test]
fn test_read_sensor_input7_closed_returns_true() {
    let (mut adapter, _, responses) = make_adapter();
    enqueue(&responses, REPLY_IFACE_INPUTS, &[0x00]);
    let val = adapter.read_sensor("7", None).unwrap().unwrap();
    assert_eq!(val.as_string(), "true");
}

#[test]
fn test_read_sensor_input7_open_returns_false() {
    let (mut adapter, _, responses) = make_adapter();
    enqueue(&responses, REPLY_IFACE_INPUTS, &[0x02]); // bit 1 set = open
    let val = adapter.read_sensor("7", None).unwrap().unwrap();
    assert_eq!(val.as_string(), "false");
}

#[test]
fn test_read_sensor_inputs_are_independent() {
    // 0x01: bit0 set (input 6 open), bit1 clear (input 7 closed)
    let (mut adapter, _, responses) = make_adapter();
    enqueue(&responses, REPLY_IFACE_INPUTS, &[0x01]);
    enqueue(&responses, REPLY_IFACE_INPUTS, &[0x01]);
    let v6 = adapter.read_sensor("6", None).unwrap().unwrap();
    let v7 = adapter.read_sensor("7", None).unwrap().unwrap();
    assert_eq!(v6.as_string(), "false");
    assert_eq!(v7.as_string(), "true");
}

// ── Counter behaviour ─────────────────────────────────────────────────────────

#[test]
fn test_read_counter_input6() {
    let (mut adapter, _, responses) = make_adapter();
    enqueue(&responses, REPLY_IFACE_COUNTS, &[42, 0, 0, 0, 99, 0, 0, 0]);
    assert_eq!(adapter.read_counter("6").unwrap(), 42);
}

#[test]
fn test_read_counter_input7() {
    let (mut adapter, _, responses) = make_adapter();
    enqueue(&responses, REPLY_IFACE_COUNTS, &[42, 0, 0, 0, 99, 0, 0, 0]);
    assert_eq!(adapter.read_counter("7").unwrap(), 99);
}

#[test]
fn test_reset_counter_sends_port_6() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);
    adapter.reset_counter("6").unwrap();
    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_IFACE_RESET_COUNT);
    assert_eq!(written_payload(&w), &[6u8]);
}

#[test]
fn test_reset_counter_sends_port_7() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);
    adapter.reset_counter("7").unwrap();
    assert_eq!(written_payload(&written.lock().unwrap()), &[7u8]);
}

// ── Validation ────────────────────────────────────────────────────────────────

#[test]
fn test_validate_output_port_accepts_0_to_5() {
    let (adapter, _, _) = make_adapter();
    for p in &["0", "1", "2", "3", "4", "5"] {
        assert!(adapter.validate_output_port(p).is_ok(), "port {} should be valid", p);
    }
}

#[test]
fn test_validate_output_port_rejects_input_ports() {
    let (adapter, _, _) = make_adapter();
    assert!(adapter.validate_output_port("6").is_err());
    assert!(adapter.validate_output_port("7").is_err());
}

#[test]
fn test_validate_output_port_accepts_pair_ports() {
    let (adapter, _, _) = make_adapter();
    assert!(adapter.validate_output_port("a").is_ok());
    assert!(adapter.validate_output_port("b").is_ok());
    assert!(adapter.validate_output_port("c").is_ok());
}

#[test]
fn test_pair_port_a_even_sets_output0_clears_output1() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);

    adapter.start_port("a", PortDirection::Even, 200).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_IFACE_SET_OUTPUTS);
    // mask=0x03 (bits 0+1), duties=[200, 0]
    assert_eq!(written_payload(&w), &[0x03, 200, 0]);
}

#[test]
fn test_pair_port_a_odd_sets_output1_clears_output0() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);

    adapter.start_port("a", PortDirection::Odd, 200).unwrap();

    // mask=0x03, duties=[0, 200]
    assert_eq!(written_payload(&written.lock().unwrap()), &[0x03, 0, 200]);
}

#[test]
fn test_stop_pair_port_a_clears_both_outputs() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses); // start
    enqueue_ok(&responses); // stop

    adapter.start_port("a", PortDirection::Even, 200).unwrap();
    adapter.stop_port("a").unwrap();

    let w = written.lock().unwrap();
    let first_len = 2 + w[1] as usize + 1;
    // mask=0x03, duties=[0, 0]
    assert_eq!(written_payload(&w[first_len..]), &[0x03, 0, 0]);
}

#[test]
fn test_pair_port_b_even_sets_output2_clears_output3() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);

    adapter.start_port("b", PortDirection::Even, 128).unwrap();

    // mask=0x0C (bits 2+3), duties=[128, 0]
    assert_eq!(written_payload(&written.lock().unwrap()), &[0x0C, 128, 0]);
}

#[test]
fn test_pair_port_direction_change_swaps_active_output() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses); // even
    enqueue_ok(&responses); // odd

    adapter.start_port("a", PortDirection::Even, 150).unwrap();
    adapter.start_port("a", PortDirection::Odd, 150).unwrap();

    let w = written.lock().unwrap();
    let first_len = 2 + w[1] as usize + 1;
    // second frame: mask=0x03, duties=[0, 150]
    assert_eq!(written_payload(&w[first_len..]), &[0x03, 0, 150]);
}

#[test]
fn test_validate_sensor_port_accepts_6_and_7() {
    let (adapter, _, _) = make_adapter();
    assert!(adapter.validate_sensor_port("6", None).is_ok());
    assert!(adapter.validate_sensor_port("7", None).is_ok());
}

#[test]
fn test_validate_sensor_port_rejects_output_ports() {
    let (adapter, _, _) = make_adapter();
    assert!(adapter.validate_sensor_port("0", None).is_err());
    assert!(adapter.validate_sensor_port("5", None).is_err());
}

// ── Unsupported operations ────────────────────────────────────────────────────

#[test]
fn test_rotate_by_degrees_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.rotate_port_by_degrees("0", PortDirection::Even, 100, 90).is_err());
}

#[test]
fn test_rotate_to_position_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.rotate_port_to_position("0", PortDirection::Even, 100, 180).is_err());
}

#[test]
fn test_reset_port_zero_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.reset_port_zero("0").is_err());
}

#[test]
fn test_rotate_to_abs_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.rotate_to_abs("0", PortDirection::Even, 100, 0).is_err());
}

#[test]
fn test_max_power_is_255() {
    let (adapter, _, _) = make_adapter();
    assert_eq!(adapter.max_power(), 255);
}
