use super::*;
use crate::constants::*;
use crate::protocol::build_frame;
use crate::transport::BrickInterfaceTransport;
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

// Command replies echo the request's (non-zero) SEQ on the wire; the driver
// only distinguishes events (SEQ 0x00) from replies, so any non-zero value
// works here.
fn enqueue(responses: &Arc<Mutex<VecDeque<u8>>>, cmd: u8, payload: &[u8]) {
    let frame = build_frame(0x5A, cmd, payload);
    responses.lock().unwrap().extend(frame);
}

// Async events (IR_DONE) use SEQ 0x00.
fn enqueue_event(responses: &Arc<Mutex<VecDeque<u8>>>, cmd: u8, payload: &[u8]) {
    let frame = build_frame(0x00, cmd, payload);
    responses.lock().unwrap().extend(frame);
}

fn enqueue_ok(responses: &Arc<Mutex<VecDeque<u8>>>) {
    enqueue(responses, REPLY_OK, &[]);
}

fn make_device() -> (BrickInterface, Arc<Mutex<Vec<u8>>>, Arc<Mutex<VecDeque<u8>>>) {
    let (transport, written, responses) = MockTransport::new();
    let device = BrickInterface::from_transport(Box::new(transport));
    (device, written, responses)
}

// Pull the CMD byte out of a raw written frame (byte index 3).
fn written_cmd(written: &[u8]) -> u8 {
    written[3]
}

// Pull the payload bytes out of a raw written frame.
fn written_payload(written: &[u8]) -> Vec<u8> {
    let len = written[1] as usize;
    written[4..2 + len].to_vec()
}

// ── set_outputs_masked ────────────────────────────────────────────────────────

#[test]
fn test_set_outputs_masked_single_output() {
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.set_outputs_masked(0x01, &[200]).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_IFACE_SET_OUTPUTS);
    assert_eq!(written_payload(&w), &[0x01, 200]);
}

#[test]
fn test_set_outputs_masked_two_outputs() {
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.set_outputs_masked(0x03, &[100, 0]).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w), &[0x03, 100, 0]);
}

#[test]
fn test_set_outputs_masked_all_outputs() {
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.set_outputs_masked(0x3F, &[10, 20, 30, 40, 50, 60]).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w), &[0x3F, 10, 20, 30, 40, 50, 60]);
}

#[test]
fn test_set_outputs_masked_non_contiguous_outputs() {
    // mask=0x05 = bits 0 and 2; duties=[a, b] sets output 0 to a, output 2 to b
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.set_outputs_masked(0x05, &[0xFF, 0x80]).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w), &[0x05, 0xFF, 0x80]);
}

#[test]
fn test_set_outputs_masked_strips_high_bits() {
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.set_outputs_masked(0xFF, &[1, 2, 3, 4, 5, 6]).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w)[0], 0x3F, "high bits should be masked off");
}

// ── get_inputs ────────────────────────────────────────────────────────────────

#[test]
fn test_get_inputs_returns_state_byte() {
    let (mut dev, written, responses) = make_device();
    enqueue(&responses, REPLY_IFACE_INPUTS, &[0x02]);

    let state = dev.get_inputs().unwrap();
    assert_eq!(state, 0x02);
    assert_eq!(written_cmd(&written.lock().unwrap()), CMD_IFACE_GET_INPUTS);
}

#[test]
fn test_get_inputs_unexpected_reply_errors() {
    let (mut dev, _, responses) = make_device();
    enqueue_ok(&responses); // wrong reply for get_inputs
    assert!(dev.get_inputs().is_err());
}

// ── get_counts ────────────────────────────────────────────────────────────────

#[test]
fn test_get_counts_parses_both_u32s() {
    let (mut dev, written, responses) = make_device();
    let payload = [42u8, 0, 0, 0, 99, 0, 0, 0];
    enqueue(&responses, REPLY_IFACE_COUNTS, &payload);

    let (c6, c7) = dev.get_counts().unwrap();
    assert_eq!(c6, 42);
    assert_eq!(c7, 99);
    assert_eq!(written_cmd(&written.lock().unwrap()), CMD_IFACE_GET_COUNTS);
}

#[test]
fn test_get_counts_large_value() {
    let (mut dev, _, responses) = make_device();
    let payload = [0x00u8, 0x00, 0x01, 0x00, 0xFF, 0xFF, 0xFF, 0xFF];
    enqueue(&responses, REPLY_IFACE_COUNTS, &payload);

    let (c6, c7) = dev.get_counts().unwrap();
    assert_eq!(c6, 65536);
    assert_eq!(c7, u32::MAX);
}

// ── reset_count ───────────────────────────────────────────────────────────────

#[test]
fn test_reset_count_sends_port_6() {
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.reset_count(6).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_IFACE_RESET_COUNT);
    assert_eq!(written_payload(&w), &[6u8]);
}

#[test]
fn test_reset_count_sends_port_7() {
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.reset_count(7).unwrap();

    assert_eq!(written_payload(&written.lock().unwrap()), &[7u8]);
}

#[test]
fn test_reset_count_rejects_invalid_input() {
    let (mut dev, _, _) = make_device();
    assert!(dev.reset_count(5).is_err());
    assert!(dev.reset_count(8).is_err());
}

// ── ping ──────────────────────────────────────────────────────────────────────

#[test]
fn test_ping_sends_correct_command() {
    let (mut dev, written, responses) = make_device();
    enqueue(&responses, REPLY_PONG, &[]);

    dev.ping().unwrap();
    assert_eq!(written_cmd(&written.lock().unwrap()), CMD_PING);
}

// ── get_version ───────────────────────────────────────────────────────────────

#[test]
fn test_get_version_parses_reply() {
    let (mut dev, _, responses) = make_device();
    // proto 1.0, fw 0.3
    enqueue(&responses, REPLY_VERSION, &[0x01, 0x00, 0x00, 0x03]);

    let (pmaj, pmin, fmaj, fmin) = dev.get_version().unwrap();
    assert_eq!((pmaj, pmin, fmaj, fmin), (1, 0, 0, 3));
}

// ── pf_send_single_pwm ────────────────────────────────────────────────────────

// What a real device produces per accepted send: the IR_ACCEPTED reply, then
// (after the burst) the async IR_DONE event. The driver consumes the IR_DONE
// lazily — on the next PF send or on abort.
fn enqueue_pf_ok(responses: &Arc<Mutex<VecDeque<u8>>>) {
    enqueue(responses, REPLY_IR_ACCEPTED, &[]);
    enqueue_event(responses, REPLY_IR_DONE, &[]);
}

#[test]
fn test_pf_send_single_pwm_output_a_forward() {
    let (mut dev, written, responses) = make_device();
    enqueue_pf_ok(&responses);

    dev.pf_send_single_pwm(0, false, 5).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_PF_SEND);
    assert_eq!(written_payload(&w), &[0x00, PF_MODE_SINGLE_PWM, 0x05, 0x00]);
}

#[test]
fn test_pf_send_single_pwm_output_b_forward() {
    let (mut dev, written, responses) = make_device();
    enqueue_pf_ok(&responses);

    dev.pf_send_single_pwm(0, true, 5).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w), &[0x00, PF_MODE_SINGLE_PWM, 0x15, 0x00]);
}

#[test]
fn test_pf_send_single_pwm_channel_and_reverse_step() {
    let (mut dev, written, responses) = make_device();
    enqueue_pf_ok(&responses);

    dev.pf_send_single_pwm(2, false, 9).unwrap(); // step 9 = reverse 7/7

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w), &[0x02, PF_MODE_SINGLE_PWM, 0x09, 0x00]);
}

#[test]
fn test_pf_send_single_pwm_rejects_bad_channel() {
    let (mut dev, _, _) = make_device();
    assert!(dev.pf_send_single_pwm(4, false, 0).is_err());
}

#[test]
fn test_pf_send_single_pwm_rejects_bad_step() {
    let (mut dev, _, _) = make_device();
    assert!(dev.pf_send_single_pwm(0, false, 16).is_err());
}

#[test]
fn test_pf_send_returns_after_accept_without_ir_done() {
    // The burst (~0.4-0.6 s) runs in the background: the send must return on
    // IR_ACCEPTED alone, not wait for IR_DONE.
    let (mut dev, _, responses) = make_device();
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);

    dev.pf_send_single_pwm(0, false, 5).unwrap();
}

#[test]
fn test_pf_send_consumes_previous_burst_ir_done_first() {
    let (mut dev, written, responses) = make_device();

    // First send: accepted, burst in flight.
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);
    dev.pf_send_single_pwm(0, false, 5).unwrap();

    // Device finishes the burst, then accepts the second send.
    enqueue_event(&responses, REPLY_IR_DONE, &[0x01, 0x01]);
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);
    dev.pf_send_single_pwm(0, false, 0).unwrap();

    // Both PF_SEND frames went out.
    let all = written.lock().unwrap();
    assert_eq!(written_cmd(&all), CMD_PF_SEND);
    let first_len = 2 + all[1] as usize + 1;
    assert_eq!(written_cmd(&all[first_len..]), CMD_PF_SEND);
}

#[test]
fn test_pf_send_times_out_when_previous_burst_never_completes() {
    // With a burst in flight and no IR_DONE forthcoming, the next send must
    // fail with a timeout rather than transmit into the firmware's busy slot.
    let (mut dev, written, responses) = make_device();
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);
    dev.pf_send_single_pwm(0, false, 5).unwrap();

    let err = dev.pf_send_single_pwm(0, false, 0).unwrap_err();
    assert!(err.contains("Timed out"), "got: {}", err);

    // Only the first PF_SEND was written — the second never went out.
    let all = written.lock().unwrap();
    let first_len = 2 + all[1] as usize + 1;
    assert_eq!(all.len(), first_len);
}

// ── pf_send_combo_pwm ────────────────────────────────────────────────────────

#[test]
fn test_pf_send_combo_pwm_encodes_both_steps() {
    let (mut dev, written, responses) = make_device();
    enqueue_pf_ok(&responses);

    dev.pf_send_combo_pwm(0, 5, 3).unwrap(); // step_a=5, step_b=3

    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_PF_SEND);
    // data = (step_b << 4) | step_a = (3 << 4) | 5 = 0x35
    assert_eq!(written_payload(&w), &[0x00, PF_MODE_COMBO_PWM, 0x35, 0x00]);
}

#[test]
fn test_pf_send_combo_pwm_zero_is_float() {
    let (mut dev, written, responses) = make_device();
    enqueue_pf_ok(&responses);

    dev.pf_send_combo_pwm(1, 0, 0).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w), &[0x01, PF_MODE_COMBO_PWM, 0x00, 0x00]);
}

#[test]
fn test_pf_send_combo_pwm_rejects_bad_step() {
    let (mut dev, _, _) = make_device();
    assert!(dev.pf_send_combo_pwm(0, 16, 0).is_err());
    assert!(dev.pf_send_combo_pwm(0, 0, 16).is_err());
}

// ── pf_wait_idle ─────────────────────────────────────────────────────────────

#[test]
fn test_pf_wait_idle_true_when_no_burst() {
    let (mut dev, _, _) = make_device();
    assert!(dev.pf_wait_idle(0).unwrap());
}

#[test]
fn test_pf_wait_idle_false_while_burst_in_flight() {
    let (mut dev, _, responses) = make_device();
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);
    dev.pf_send_single_pwm(0, false, 5).unwrap();

    // No IR_DONE forthcoming: a short bounded wait reports busy, not error.
    assert!(!dev.pf_wait_idle(10).unwrap());
}

#[test]
fn test_pf_wait_idle_consumes_ir_done_then_send_is_immediate() {
    let (mut dev, _, responses) = make_device();
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);
    dev.pf_send_single_pwm(0, false, 5).unwrap();

    enqueue_event(&responses, REPLY_IR_DONE, &[0x01, 0x01]);
    assert!(dev.pf_wait_idle(100).unwrap());

    // The burst flag is cleared, so the next send needs no further wait.
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);
    dev.pf_send_single_pwm(0, false, 0).unwrap();
}

// ── ir_abort_all ─────────────────────────────────────────────────────────────

#[test]
fn test_ir_abort_all_sends_correct_command() {
    let (mut dev, written, responses) = make_device();
    enqueue_ok(&responses);

    dev.ir_abort_all().unwrap();
    assert_eq!(written_cmd(&written.lock().unwrap()), CMD_IR_ABORT_ALL);
}

#[test]
fn test_ir_abort_all_drains_aborted_burst_ir_done() {
    let (mut dev, _, responses) = make_device();

    // Burst in flight…
    enqueue(&responses, REPLY_IR_ACCEPTED, &[]);
    dev.pf_send_single_pwm(0, false, 5).unwrap();

    // …aborted: firmware replies OK, then emits the aborted token's IR_DONE.
    enqueue_ok(&responses);
    enqueue_event(&responses, REPLY_IR_DONE, &[0x01, 0x01]);
    dev.ir_abort_all().unwrap();

    // Connection is idle and the event was consumed — the next command's
    // reply parses cleanly.
    enqueue(&responses, REPLY_PONG, &[]);
    dev.ping().unwrap();
}

// ── async event filtering ────────────────────────────────────────────────────

#[test]
fn test_stray_ir_done_event_not_mistaken_for_reply() {
    // An IR_DONE (SEQ 0x00) sitting in the receive buffer ahead of a command
    // reply must be skipped, not returned as the reply.
    let (mut dev, _, responses) = make_device();
    enqueue_event(&responses, REPLY_IR_DONE, &[0x01, 0x01]);
    enqueue(&responses, REPLY_PONG, &[]);

    dev.ping().unwrap();
}

// ── get_capabilities ─────────────────────────────────────────────────────────

#[test]
fn test_get_capabilities_brickinterface_bitmap() {
    let (mut dev, _, responses) = make_device();
    // BrickInterface reports 0x0037
    enqueue(&responses, REPLY_CAPABILITIES, &[0x37, 0x00]);

    let caps = dev.get_capabilities().unwrap();
    assert_eq!(caps, 0x0037);
    assert!(caps & crate::constants::CAP_INTERFACE_A != 0);
    assert!(caps & crate::constants::CAP_PF_IR != 0);
    assert!(caps & crate::constants::CAP_RCX_IR != 0);
}
