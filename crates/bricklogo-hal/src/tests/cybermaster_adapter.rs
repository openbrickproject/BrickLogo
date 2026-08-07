use super::*;
use crate::scheduler;
use std::sync::{Arc, Mutex};

/// Records every frame sent. For `request`, returns a framed-and-parsed reply
/// carrying the current `value` (so sensor reads can be exercised); motor
/// commands ignore the payload.
struct MockTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    value: Arc<Mutex<i16>>,
}

impl CmTransport for MockTransport {
    fn send(&mut self, msg: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(msg.to_vec());
        Ok(())
    }
    fn request(&mut self, msg: &[u8]) -> Result<Vec<u8>, String> {
        self.sent.lock().unwrap().push(msg.to_vec());
        // Parsed reply payload: [opcode, lo, hi].
        let v = *self.value.lock().unwrap();
        let bytes = v.to_le_bytes();
        Ok(vec![OP_GET_VALUE, bytes[0], bytes[1]])
    }
}

fn make_adapter_with_mock() -> (CyberMasterAdapter, Arc<Mutex<Vec<Vec<u8>>>>, Arc<Mutex<i16>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let value = Arc::new(Mutex::new(0i16));
    let transport: Box<dyn CmTransport> = Box::new(MockTransport {
        sent: sent.clone(),
        value: value.clone(),
    });
    let (tx, rx) = mpsc::channel();
    let slot = CmSlot {
        transport,
        rx,
        alive: true,
        toggle: false,
    };
    let slot_id = scheduler::register_slot(Box::new(slot));
    let mut adapter = CyberMasterAdapter::new("/dev/null");
    adapter.tx = Some(tx);
    adapter.slot_id = Some(slot_id);
    (adapter, sent, value)
}

fn frames_with_op(sent: &[Vec<u8>], op: u8) -> Vec<Vec<u8>> {
    // Frame layout: [HEADER(4), OP, !OP, ...]; opcode at index 4, ignore toggle.
    sent.iter()
        .filter(|f| f.len() > 4 && f[4] & !TOGGLE_BIT == op)
        .cloned()
        .collect()
}

/// Motor+state byte from a `cmd_set_motor_state` frame:
/// [HEADER(4), OP, !OP, code, !code, checksum, !checksum].
fn motor_state_code(frame: &[u8]) -> u8 {
    frame[6]
}

#[test]
fn test_cm_run_ports_for_time_batches_motor_on_off() {
    let (mut adapter, sent, _v) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 4 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 4 },
    ];

    adapter.run_ports_for_time(&commands, 1).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let motor_frames = frames_with_op(&sent, OP_SET_MOTOR_ON_OFF);
    assert_eq!(motor_frames.len(), 2, "expected 1 batched ON + 1 batched OFF");
    assert_eq!(motor_state_code(&motor_frames[0]), MOTOR_A | MOTOR_B | MOTOR_ON);
    assert_eq!(motor_state_code(&motor_frames[1]), MOTOR_A | MOTOR_B | MOTOR_OFF);
}

#[test]
fn test_cm_toggle_alternates_across_commands() {
    let (mut adapter, sent, _v) = make_adapter_with_mock();

    // start_port = SetDirection + SetPower + MotorOn; stop_port = SetPower +
    // MotorOff → five logical commands, toggle 0,1,0,1,0.
    adapter.start_port("a", PortDirection::Even, 5).unwrap();
    adapter.stop_port("a").unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 5, "expected 5 frames, got {}", sent.len());
    for (i, frame) in sent.iter().enumerate() {
        let expected = if i % 2 == 1 { TOGGLE_BIT } else { 0 };
        assert_eq!(frame[4] & TOGGLE_BIT, expected, "toggle wrong on command {}", i);
        assert_eq!(frame[5], !frame[4], "complement must track the toggled opcode");
    }
    for pair in sent.windows(2) {
        assert_ne!(pair[0][4], pair[1][4], "adjacent frames must differ in opcode byte");
    }
}

#[test]
fn test_cm_touch_thresholds_raw_value() {
    let (mut adapter, _sent, value) = make_adapter_with_mock();

    *value.lock().unwrap() = 100; // low = pressed
    let pressed = adapter.read_sensor("1", Some("touch")).unwrap();
    assert_eq!(pressed, Some(LogoValue::Word("true".to_string())));

    *value.lock().unwrap() = 850; // high = released
    let released = adapter.read_sensor("1", Some("touch")).unwrap();
    assert_eq!(released, Some(LogoValue::Word("false".to_string())));

    adapter.disconnect();
}

#[test]
fn test_cm_raw_mode_returns_number() {
    let (mut adapter, _sent, value) = make_adapter_with_mock();
    *value.lock().unwrap() = 612;
    let v = adapter.read_sensor("2", Some("raw")).unwrap();
    assert_eq!(v, Some(LogoValue::Number(612.0)));
    adapter.disconnect();
}

#[test]
fn test_cm_read_sensor_sets_raw_mode_once() {
    let (mut adapter, sent, value) = make_adapter_with_mock();
    *value.lock().unwrap() = 300;
    adapter.read_sensor("1", Some("touch")).unwrap();
    adapter.read_sensor("1", Some("touch")).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let mode_frames = frames_with_op(&sent, OP_SET_SENSOR_MODE);
    assert_eq!(mode_frames.len(), 1, "raw mode should be set once and cached");
}

#[test]
fn test_cm_rejects_unsupported_sensor_mode() {
    let (adapter, _sent, _v) = make_adapter_with_mock();
    assert!(adapter.validate_sensor_port("1", Some("touch")).is_ok());
    assert!(adapter.validate_sensor_port("1", Some("raw")).is_ok());
    assert!(adapter.validate_sensor_port("1", Some("sound")).is_err());
    assert!(adapter.validate_sensor_port("9", Some("touch")).is_err());
}

#[test]
fn test_cm_rotation_validates_motor_ports_only() {
    let (adapter, _sent, _v) = make_adapter_with_mock();
    // Tachometers live on the two internal motors a/b...
    assert!(adapter.validate_sensor_port("a", Some("rotation")).is_ok());
    assert!(adapter.validate_sensor_port("b", Some("rotation")).is_ok());
    // ...not the external motor, nor the passive sensor inputs.
    assert!(adapter.validate_sensor_port("c", Some("rotation")).is_err());
    assert!(adapter.validate_sensor_port("1", Some("rotation")).is_err());
    // Conversely, touch is not valid on a motor port.
    assert!(adapter.validate_sensor_port("a", Some("touch")).is_err());
}

#[test]
fn test_cm_rotation_reads_signed_count_without_mode_set() {
    let (mut adapter, sent, value) = make_adapter_with_mock();

    *value.lock().unwrap() = -250; // reverse cumulative count
    let v = adapter.read_sensor("a", Some("rotation")).unwrap();
    assert_eq!(v, Some(LogoValue::Number(-250.0)));
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    // Rotation polls the tacho source directly; it must NOT configure a sensor mode.
    assert!(
        frames_with_op(&sent, OP_SET_SENSOR_MODE).is_empty(),
        "rotation read should not send a sensor-mode command"
    );
    // The read targets the tacho count source with motor A's index (0).
    let reads = frames_with_op(&sent, OP_GET_VALUE);
    assert_eq!(reads.len(), 1);
    // Frame: [HEADER(4), OP, !OP, source, !source, arg, !arg, checksum, !checksum].
    assert_eq!(reads[0][6], SOURCE_TACHO_COUNT);
    assert_eq!(reads[0][8], 0); // motor A → tacho index 0
}
