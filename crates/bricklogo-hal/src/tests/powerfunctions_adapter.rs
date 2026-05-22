use super::*;
use crate::adapter::{HardwareAdapter, PortCommand, PortDirection};
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

fn enqueue_pf_ok(responses: &Arc<Mutex<VecDeque<u8>>>) {
    enqueue(responses, REPLY_IR_ACCEPTED, &[]);
    enqueue(responses, REPLY_IR_DONE, &[]);
}

fn make_adapter() -> (PowerFunctionsAdapter, Arc<Mutex<Vec<u8>>>, Arc<Mutex<VecDeque<u8>>>) {
    let (transport, written, responses) = MockTransport::new();
    let shared = SharedBrickInterface::new(BrickInterface::from_transport(Box::new(transport)));
    (PowerFunctionsAdapter::new_with_shared(shared), written, responses)
}

fn written_cmd(written: &[u8]) -> u8 { written[3] }
fn written_payload(written: &[u8]) -> Vec<u8> {
    let len = written[1] as usize;
    written[4..2 + len].to_vec()
}

// ── Single-port output behaviour ──────────────────────────────────────────────

#[test]
fn test_start_port_red1_forward_sends_single_pwm() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses);

    adapter.start_port("red1", PortDirection::Even, 5).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_cmd(&w), CMD_PF_SEND);
    // channel=0, mode=SINGLE_PWM, data=0x05 (output A, step 5), flags=0
    assert_eq!(written_payload(&w), &[0x00, PF_MODE_SINGLE_PWM, 0x05, 0x00]);
}

#[test]
fn test_start_port_blue1_forward_sets_output_b_bit() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses);

    adapter.start_port("blue1", PortDirection::Even, 5).unwrap();

    let w = written.lock().unwrap();
    // output B: data = 0x10 | step = 0x15
    assert_eq!(written_payload(&w), &[0x00, PF_MODE_SINGLE_PWM, 0x15, 0x00]);
}

#[test]
fn test_start_port_channel_4_uses_channel_3() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses);

    adapter.start_port("red4", PortDirection::Even, 7).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w), &[0x03, PF_MODE_SINGLE_PWM, 0x07, 0x00]);
}

#[test]
fn test_start_port_odd_direction_encodes_reverse_step() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses);

    adapter.start_port("red1", PortDirection::Odd, 5).unwrap();

    let w = written.lock().unwrap();
    // reverse step: 16 - 5 = 11
    assert_eq!(written_payload(&w), &[0x00, PF_MODE_SINGLE_PWM, 0x0B, 0x00]);
}

#[test]
fn test_start_port_power_capped_at_7() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses);

    adapter.start_port("red1", PortDirection::Even, 7).unwrap();

    let w = written.lock().unwrap();
    assert_eq!(written_payload(&w)[2], 0x07);
}

#[test]
fn test_stop_port_sends_float_step() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses); // start
    enqueue_pf_ok(&responses); // stop

    adapter.start_port("red1", PortDirection::Even, 5).unwrap();
    adapter.stop_port("red1").unwrap();

    let all = written.lock().unwrap();
    let first_len = 2 + all[1] as usize + 1;
    let p_stop = written_payload(&all[first_len..]);
    assert_eq!(p_stop, &[0x00, PF_MODE_SINGLE_PWM, 0x00, 0x00]);
}

// ── Combo (multi-port) behaviour ──────────────────────────────────────────────

#[test]
fn test_start_ports_same_channel_sends_two_single_pwm() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses); // red1
    enqueue_pf_ok(&responses); // blue1

    let cmds = [
        PortCommand { port: "red1", direction: PortDirection::Even, power: 5 },
        PortCommand { port: "blue1", direction: PortDirection::Even, power: 3 },
    ];
    adapter.start_ports(&cmds).unwrap();

    let all = written.lock().unwrap();
    assert_eq!(written_cmd(&all), CMD_PF_SEND);
    assert_eq!(written_payload(&all), &[0x00, PF_MODE_SINGLE_PWM, 0x05, 0x00]);
    let first_len = 2 + all[1] as usize + 1;
    // blue1: output B, step 3 → 0x10 | 3 = 0x13
    assert_eq!(written_payload(&all[first_len..]), &[0x00, PF_MODE_SINGLE_PWM, 0x13, 0x00]);
}

#[test]
fn test_start_ports_combo_reverse_direction() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses); // red2
    enqueue_pf_ok(&responses); // blue2

    let cmds = [
        PortCommand { port: "red2", direction: PortDirection::Odd, power: 7 },
        PortCommand { port: "blue2", direction: PortDirection::Even, power: 7 },
    ];
    adapter.start_ports(&cmds).unwrap();

    let all = written.lock().unwrap();
    // red2: channel 1, output A, reverse step = 16-7 = 9
    assert_eq!(written_payload(&all), &[0x01, PF_MODE_SINGLE_PWM, 0x09, 0x00]);
    let first_len = 2 + all[1] as usize + 1;
    // blue2: channel 1, output B, forward step 7 → 0x10 | 7 = 0x17
    assert_eq!(written_payload(&all[first_len..]), &[0x01, PF_MODE_SINGLE_PWM, 0x17, 0x00]);
}

#[test]
fn test_start_ports_cross_channel_succeeds() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses); // red1
    enqueue_pf_ok(&responses); // blue3

    let cmds = [
        PortCommand { port: "red1", direction: PortDirection::Even, power: 5 },
        PortCommand { port: "blue3", direction: PortDirection::Even, power: 3 },
    ];
    assert!(adapter.start_ports(&cmds).is_ok());

    let all = written.lock().unwrap();
    // red1: channel 0, output A, step 5
    assert_eq!(written_payload(&all), &[0x00, PF_MODE_SINGLE_PWM, 0x05, 0x00]);
    let first_len = 2 + all[1] as usize + 1;
    // blue3: channel 2, output B, step 3 → 0x10 | 3 = 0x13
    assert_eq!(written_payload(&all[first_len..]), &[0x02, PF_MODE_SINGLE_PWM, 0x13, 0x00]);
}

#[test]
fn test_stop_ports_sends_two_single_pwm_float() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses); // red1
    enqueue_pf_ok(&responses); // blue1

    adapter.stop_ports(&["red1", "blue1"]).unwrap();

    let all = written.lock().unwrap();
    // red1: output A, step 0 (float)
    assert_eq!(written_payload(&all), &[0x00, PF_MODE_SINGLE_PWM, 0x00, 0x00]);
    let first_len = 2 + all[1] as usize + 1;
    // blue1: output B, step 0 (float) → 0x10 | 0 = 0x10
    assert_eq!(written_payload(&all[first_len..]), &[0x00, PF_MODE_SINGLE_PWM, 0x10, 0x00]);
}

#[test]
fn test_stop_ports_cross_channel_succeeds() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_pf_ok(&responses); // red1
    enqueue_pf_ok(&responses); // blue3

    assert!(adapter.stop_ports(&["red1", "blue3"]).is_ok());

    let all = written.lock().unwrap();
    assert_eq!(written_payload(&all), &[0x00, PF_MODE_SINGLE_PWM, 0x00, 0x00]);
    let first_len = 2 + all[1] as usize + 1;
    assert_eq!(written_payload(&all[first_len..]), &[0x02, PF_MODE_SINGLE_PWM, 0x10, 0x00]);
}

// ── Disconnect ────────────────────────────────────────────────────────────────

#[test]
fn test_disconnect_calls_ir_abort_all() {
    let (mut adapter, written, responses) = make_adapter();
    enqueue_ok(&responses);

    adapter.disconnect();

    assert_eq!(written_cmd(&written.lock().unwrap()), CMD_IR_ABORT_ALL);
}

// ── Validation ────────────────────────────────────────────────────────────────

#[test]
fn test_validate_output_port_accepts_all_valid() {
    let (adapter, _, _) = make_adapter();
    for p in &["red1","blue1","red2","blue2","red3","blue3","red4","blue4"] {
        assert!(adapter.validate_output_port(p).is_ok(), "port {} should be valid", p);
    }
}

#[test]
fn test_validate_output_port_rejects_out_of_range() {
    let (adapter, _, _) = make_adapter();
    assert!(adapter.validate_output_port("red0").is_err());
    assert!(adapter.validate_output_port("blue5").is_err());
    assert!(adapter.validate_output_port("red").is_err());
    assert!(adapter.validate_output_port("0").is_err());
}

#[test]
fn test_validate_sensor_port_always_errors() {
    let (adapter, _, _) = make_adapter();
    assert!(adapter.validate_sensor_port("red1", None).is_err());
}

#[test]
fn test_max_power_is_7() {
    let (adapter, _, _) = make_adapter();
    assert_eq!(adapter.max_power(), 7);
}

// ── Unsupported operations ────────────────────────────────────────────────────

#[test]
fn test_rotate_by_degrees_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.rotate_port_by_degrees("red1", PortDirection::Even, 5, 90).is_err());
}

#[test]
fn test_rotate_to_position_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.rotate_port_to_position("red1", PortDirection::Even, 5, 180).is_err());
}

#[test]
fn test_reset_port_zero_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.reset_port_zero("red1").is_err());
}

#[test]
fn test_rotate_to_abs_unsupported() {
    let (mut adapter, _, _) = make_adapter();
    assert!(adapter.rotate_to_abs("red1", PortDirection::Even, 5, 0).is_err());
}
