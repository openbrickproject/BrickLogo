use super::*;
use crate::scheduler;
use rust_tclogoconnect::constants::*;
use rust_tclogoconnect::device::Device;
use rust_tclogoconnect::transport::Transport;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A transport that plays back a scripted reply for each command it sees
/// written (used for the `R`/`V` handshake `Device::connect` performs), and
/// exposes its inbound byte queue so tests can inject unsolicited `Ixx`
/// input reports after the device is up and running.
struct MockTransport {
    written: Arc<Mutex<Vec<Vec<u8>>>>,
    responses: HashMap<Vec<u8>, Vec<u8>>,
    inbox: Arc<Mutex<VecDeque<u8>>>,
}

impl MockTransport {
    fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>, Arc<Mutex<VecDeque<u8>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));
        let inbox = Arc::new(Mutex::new(VecDeque::new()));
        let mut responses = HashMap::new();
        responses.insert(CMD_PROBE.to_vec(), b"CF\r\n".to_vec());
        responses.insert(CMD_VERSION.to_vec(), b"V1.0.1\r\n".to_vec());
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

#[allow(clippy::type_complexity)]
fn make_adapter_with_mock() -> (
    TcLogoConnectAdapter,
    Arc<Mutex<Vec<Vec<u8>>>>,
    Arc<Mutex<VecDeque<u8>>>,
) {
    let (transport, written, inbox) = MockTransport::new();
    let device = Device::connect(Box::new(transport)).unwrap();
    let state = Arc::new(TcState::default());
    let (tx, rx) = mpsc::channel();
    let slot = TcSlot { device, rx, alive: true, duty: [0; 6], state: state.clone() };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = TcLogoConnectAdapter::new("/dev/null");
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    adapter.state = state;
    (adapter, written, inbox)
}

fn wait_until<F: FnMut() -> bool>(mut cond: F, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

// ── Port validation ─────────────────────────────

#[test]
fn test_validate_output_port_accepts_direct_and_paired() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    for port in ["0", "1", "2", "3", "4", "5", "a", "b", "c"] {
        assert!(adapter.validate_output_port(port).is_ok(), "port {} should validate", port);
    }
    assert!(adapter.validate_output_port("6").is_err());
    assert!(adapter.validate_output_port("d").is_err());
    adapter.disconnect();
}

#[test]
fn test_validate_sensor_port_ignores_mode() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    assert!(adapter.validate_sensor_port("6", Some("touch")).is_ok());
    assert!(adapter.validate_sensor_port("7", None).is_ok());
    assert!(adapter.validate_sensor_port("7", Some("anything")).is_ok());
    assert!(adapter.validate_sensor_port("0", Some("touch")).is_err());
    adapter.disconnect();
}

// ── On/off masks ────────────────────────────────

#[test]
fn test_start_port_full_power_sends_static_mask() {
    let (mut adapter, written, _) = make_adapter_with_mock();
    adapter.start_port("0", PortDirection::Even, 255).unwrap();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D01\n");
    adapter.stop_port("0").unwrap();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D00\n");
    adapter.disconnect();
}

#[test]
fn test_start_ports_combines_into_one_mask() {
    let (mut adapter, written, _) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "0", direction: PortDirection::Even, power: 255 },
        PortCommand { port: "2", direction: PortDirection::Even, power: 255 },
    ];
    adapter.start_ports(&commands).unwrap();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D05\n"); // bits 0 and 2
    adapter.disconnect();
}

// ── Paired-port Even/Odd semantics ──────────────

#[test]
fn test_paired_port_even_drives_first_half() {
    let (mut adapter, written, _) = make_adapter_with_mock();
    adapter.start_port("a", PortDirection::Even, 255).unwrap();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D01\n"); // output 0
    adapter.disconnect();
}

#[test]
fn test_paired_port_odd_drives_second_half() {
    let (mut adapter, written, _) = make_adapter_with_mock();
    adapter.start_port("a", PortDirection::Odd, 255).unwrap();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D02\n"); // output 1
    adapter.disconnect();
}

#[test]
fn test_paired_port_b_and_c_map_to_their_own_outputs() {
    let (mut adapter, written, _) = make_adapter_with_mock();
    adapter.start_port("b", PortDirection::Even, 255).unwrap();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D04\n"); // output 2
    adapter.start_port("c", PortDirection::Odd, 255).unwrap();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D24\n"); // output 2 stays + output 5
    adapter.disconnect();
}

// ── PWM batch duty distribution (pure function) ─

#[test]
fn test_duty_to_frames_half_duty_is_leading_edge_four_on_four_off() {
    let frames = duty_to_frames(&[128, 0, 0, 0, 0, 0]);
    let period: Vec<bool> = frames[0..8].iter().map(|&f| f & 0x01 != 0).collect();
    assert_eq!(
        period,
        vec![true, true, true, true, false, false, false, false],
        "round(8*128/255) = 4 leading on-slots, contiguous"
    );
    // The two 8-slot halves of the batch must be identical periods.
    assert_eq!(&frames[0..8], &frames[8..16]);
}

#[test]
fn test_duty_to_frames_small_nonzero_duty_clamps_to_one_on_slot() {
    let frames = duty_to_frames(&[1, 0, 0, 0, 0, 0]);
    let period: Vec<bool> = frames[0..8].iter().map(|&f| f & 0x01 != 0).collect();
    assert_eq!(
        period,
        vec![true, false, false, false, false, false, false, false],
        "duty 1 rounds to 0 on-slots but must clamp to at least 1 — a small nonzero setpower must still pulse"
    );
    assert_eq!(&frames[0..8], &frames[8..16]);
}

#[test]
fn test_duty_to_frames_zero_and_full_are_all_off_or_all_on() {
    let frames = duty_to_frames(&[0, 255, 0, 0, 0, 0]);
    assert!(frames.iter().all(|&f| f & 0x01 == 0), "duty 0 must never turn the bit on");
    assert!(frames.iter().all(|&f| f & 0x02 != 0), "duty 255 must be on every frame");
}

#[test]
fn test_duty_to_frames_multiple_outputs_are_independent() {
    let frames = duty_to_frames(&[128, 64, 0, 0, 0, 0]);
    let out0: Vec<bool> = frames[0..8].iter().map(|&f| f & 0x01 != 0).collect();
    let out1: Vec<bool> = frames[0..8].iter().map(|&f| f & 0x02 != 0).collect();
    assert_eq!(out0, vec![true, true, true, true, false, false, false, false]); // duty 128 -> 4/8
    assert_eq!(out1, vec![true, true, false, false, false, false, false, false]); // duty 64 -> round(8*64/255) = 2/8
    assert_eq!(&frames[0..8], &frames[8..16]);
}

#[test]
fn test_mask_from_duty_and_is_static() {
    assert!(is_static(&[0, 255, 0, 255, 0, 0]));
    assert!(!is_static(&[128, 0, 0, 0, 0, 0]));
    assert_eq!(mask_from_duty(&[0, 255, 0, 255, 0, 0]), 0b0000_1010);
}

#[test]
fn test_partial_duty_streams_a_16_frame_batch() {
    let (mut adapter, written, _) = make_adapter_with_mock();
    // 128 is neither 0 nor 255 — must fall into the PWM batch path.
    adapter.start_port("0", PortDirection::Even, 128).unwrap();
    let last = written.lock().unwrap().last().unwrap().clone();
    // 16 frames of "Dxx\n" (4 bytes each) concatenated in one write.
    assert_eq!(last.len(), 16 * 4);
    adapter.disconnect();
}

// ── Touch true/false mapping ─────────────────────

#[test]
fn test_touch_reads_true_when_bit_set_false_otherwise() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    *adapter.state.last_input.lock().unwrap() = INPUT_BIT_PORT6;
    assert_eq!(adapter.read_sensor("6", Some("touch")).unwrap(), Some(LogoValue::Word("true".to_string())));
    assert_eq!(adapter.read_sensor("7", Some("touch")).unwrap(), Some(LogoValue::Word("false".to_string())));

    *adapter.state.last_input.lock().unwrap() = INPUT_BIT_PORT7;
    assert_eq!(adapter.read_sensor("6", Some("touch")).unwrap(), Some(LogoValue::Word("false".to_string())));
    assert_eq!(adapter.read_sensor("7", Some("touch")).unwrap(), Some(LogoValue::Word("true".to_string())));
    adapter.disconnect();
}

#[test]
fn test_touch_unknown_port_errors() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    assert!(adapter.read_sensor("0", Some("touch")).is_err());
    adapter.disconnect();
}

// ── Edge counting and reset ──────────────────────

#[test]
fn test_edge_counting_counts_rising_edges_only() {
    let (mut adapter, _written, inbox) = make_adapter_with_mock();

    // Press then release port 6 twice — two rising edges expected.
    inbox.lock().unwrap().extend(b"I40\n".iter().copied());
    assert!(wait_until(|| adapter.read_counter("6").unwrap() == 1, Duration::from_millis(500)));

    inbox.lock().unwrap().extend(b"I00\n".iter().copied());
    assert!(wait_until(|| *adapter.state.last_input.lock().unwrap() == 0x00, Duration::from_millis(500)));
    assert_eq!(adapter.read_counter("6").unwrap(), 1, "release must not add an edge");

    inbox.lock().unwrap().extend(b"I40\n".iter().copied());
    assert!(wait_until(|| adapter.read_counter("6").unwrap() == 2, Duration::from_millis(500)));

    // Port 7 never toggled — its counter stays at zero.
    assert_eq!(adapter.read_counter("7").unwrap(), 0);

    adapter.disconnect();
}

#[test]
fn test_edge_counting_catches_press_and_release_within_one_tick() {
    let (mut adapter, _written, inbox) = make_adapter_with_mock();

    // Both lines are queued under one lock acquisition, so the scheduler's
    // next drain sees them together — a fast tap that lands inside a
    // single ~16ms tick. Comparing only the tick's final mask against the
    // previous tick's would miss this: I40 then I00 nets out to "still
    // released" tick-to-tick even though a real press happened.
    inbox.lock().unwrap().extend(b"I40\nI00\n".iter().copied());
    assert!(wait_until(|| adapter.read_counter("6").unwrap() == 1, Duration::from_millis(500)));
    assert_eq!(adapter.read_counter("6").unwrap(), 1, "press+release in one tick must count exactly once");
    assert_eq!(adapter.read_sensor("6", Some("touch")).unwrap(), Some(LogoValue::Word("false".to_string())), "sensor must read released afterwards");
    assert_eq!(adapter.read_counter("7").unwrap(), 0);

    adapter.disconnect();
}

#[test]
fn test_reset_counter_clears_only_its_own_port() {
    let (mut adapter, _written, inbox) = make_adapter_with_mock();

    inbox.lock().unwrap().extend(b"I40\n".iter().copied());
    assert!(wait_until(|| adapter.read_counter("6").unwrap() == 1, Duration::from_millis(500)));
    inbox.lock().unwrap().extend(b"Ic0\n".iter().copied()); // port 6 stays pressed, port 7 presses
    assert!(wait_until(|| adapter.read_counter("7").unwrap() == 1, Duration::from_millis(500)));

    adapter.reset_counter("6").unwrap();
    assert_eq!(adapter.read_counter("6").unwrap(), 0);
    assert_eq!(adapter.read_counter("7").unwrap(), 1);

    adapter.disconnect();
}

#[test]
fn test_counter_unknown_port_errors() {
    let (mut adapter, _, _) = make_adapter_with_mock();
    assert!(adapter.read_counter("0").is_err());
    assert!(adapter.reset_counter("a").is_err());
    adapter.disconnect();
}

// ── Disconnect ───────────────────────────────────

#[test]
fn test_disconnect_sends_all_off() {
    let (mut adapter, written, _) = make_adapter_with_mock();
    adapter.start_port("0", PortDirection::Even, 255).unwrap();
    adapter.disconnect();
    assert_eq!(written.lock().unwrap().last().unwrap(), b"D00\n");
}
