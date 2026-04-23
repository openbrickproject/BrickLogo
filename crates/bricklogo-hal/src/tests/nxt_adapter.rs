use super::*;
use rust_nxt::protocol as p;
use std::sync::{Arc, Mutex};

/// Records every sent LCP packet and fabricates a generic success reply for
/// any reply-required command. The handle's `request()` loop matches
/// replies by echoed opcode, so we just echo whatever was last sent.
///
/// For `GetOutputState` we fake motor progress: if the port has never been
/// fired yet, `rotation_count` reads as `0` (so `start_rotation` snapshots
/// are stable). After a fire, subsequent reads return `100_000` — a value
/// larger than any reasonable target, so the slot's tacho-delta check
/// trips on the first poll and the step completes. This lets tests verify
/// the adapter's wire ordering without modelling motion over time.
struct MockTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl rust_nxt::transport::Transport for MockTransport {
    fn send(&mut self, packet: &[u8]) -> Result<(), String> {
        self.sent.lock().unwrap().push(packet.to_vec());
        Ok(())
    }

    fn recv(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, String> {
        // Find the most recent reply-required packet and build a reply for it.
        let sent = self.sent.lock().unwrap();
        let last = sent
            .iter()
            .rev()
            .find(|pkt| pkt.len() >= 2 && (pkt[0] & p::NO_REPLY_FLAG) == 0)
            .cloned()
            .ok_or("mock: no reply-required packet to respond to")?;
        drop(sent);
        let op = last[1];
        let mut reply = vec![p::TYPE_REPLY, op, 0x00];
        match op {
            p::OP_GET_OUTPUT_STATE => {
                let port = last.get(2).copied().unwrap_or(0);
                // Rotation count comes back zero until the port has been
                // fired at least once; after that, jump to a value large
                // enough that the slot sees "target reached" on its first
                // poll.
                let sent = self.sent.lock().unwrap();
                let has_fired = sent.iter().any(|pkt| {
                    pkt.len() >= 8
                        && pkt[1] == p::OP_SET_OUTPUT_STATE
                        && pkt[2] == port
                        && pkt[7] == p::RUN_RUNNING
                        && (pkt[3] as i8) != 0
                });
                drop(sent);
                let moved: i32 = if has_fired { 100_000 } else { 0 };
                reply.push(port);                  // port
                reply.push(0);                     // power
                reply.push(0);                     // mode
                reply.push(0);                     // regulation
                reply.push(0);                     // turn ratio
                reply.push(p::RUN_RUNNING);        // run_state
                reply.extend_from_slice(&0u32.to_le_bytes());  // tacho_limit
                reply.extend_from_slice(&moved.to_le_bytes());   // tacho_count
                reply.extend_from_slice(&moved.to_le_bytes());   // block_tacho_count
                reply.extend_from_slice(&moved.to_le_bytes());   // rotation_count
            }
            p::OP_GET_INPUT_VALUES => {
                let port = last.get(2).copied().unwrap_or(0);
                reply.push(port);
                reply.push(1);      // valid
                reply.push(0);      // calibrated
                reply.push(0);      // sensor type
                reply.push(0);      // sensor mode
                reply.extend_from_slice(&0u16.to_le_bytes());
                reply.extend_from_slice(&0u16.to_le_bytes());
                reply.extend_from_slice(&1i16.to_le_bytes());  // scaled (e.g. touch pressed)
                reply.extend_from_slice(&1i16.to_le_bytes());
            }
            p::OP_GET_BATTERY_LEVEL => {
                reply.extend_from_slice(&8000u16.to_le_bytes());
            }
            p::SYS_GET_FIRMWARE_VERSION => {
                reply.extend_from_slice(&[0x7C, 0x01, 0x05, 0x01]);
            }
            _ => {}
        }
        Ok(reply)
    }
}

fn make_adapter_with_mock() -> (NxtAdapter, Arc<Mutex<Vec<Vec<u8>>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let transport: Box<dyn rust_nxt::transport::Transport> = Box::new(MockTransport {
        sent: sent.clone(),
    });
    let nxt = rust_nxt::nxt::Nxt::new(transport);
    let (adapter, _alive) = NxtAdapter::with_connected_nxt(nxt);
    (adapter, sent)
}

fn packets_with_opcode(sent: &[Vec<u8>], op: u8) -> Vec<Vec<u8>> {
    sent.iter().filter(|pkt| pkt.len() >= 2 && pkt[1] == op).cloned().collect()
}

fn is_fire(pkt: &[u8]) -> bool {
    // A motor "fire" is a RUN_RUNNING SetOutputState with non-zero power;
    // a zero-power RUN_RUNNING is an active brake, not a start.
    pkt.len() >= 8
        && pkt[1] == p::OP_SET_OUTPUT_STATE
        && (pkt[0] & p::NO_REPLY_FLAG) != 0
        && pkt[7] == p::RUN_RUNNING
        && (pkt[3] as i8) != 0
}

fn is_poll(pkt: &[u8]) -> bool {
    pkt.len() >= 2 && pkt[1] == p::OP_GET_OUTPUT_STATE
}

/// Assert that the batch fired both motors adjacently — no completion poll
/// was sent between them. Regressing the batch to a sequential loop (e.g.
/// by reverting to the default trait impl) would insert GetOutputState
/// polls between the two fires; this guard catches that.
fn assert_both_motors_fired_in_parallel(sent: &[Vec<u8>]) {
    let first_fire = sent
        .iter()
        .position(|pkt| is_fire(pkt))
        .expect("expected at least one motor fire in the wire log");
    let after_first = &sent[first_fire + 1..];
    let next_non_disconnect = after_first
        .iter()
        .find(|pkt| is_fire(pkt) || is_poll(pkt))
        .expect("expected either another fire or a completion poll after first fire");
    assert!(
        is_fire(next_non_disconnect),
        "batch fire regressed to sequential: first motor fire was followed by \
         {:#04x?} instead of a second motor fire. Either the override was lost \
         and the default trait impl is serialising per port, or the batch \
         stopped using NO_REPLY_FLAG.",
        next_non_disconnect
    );
    // And there should be exactly two motor fires overall.
    let fires = sent.iter().filter(|pkt| is_fire(pkt)).count();
    assert_eq!(
        fires, 2,
        "expected exactly 2 motor fires, got {}",
        fires
    );
}

#[test]
fn test_nxt_rotate_ports_by_degrees_fires_both_motors_in_parallel() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    adapter.rotate_ports_by_degrees(&commands, 90).unwrap();
    let snapshot = sent.lock().unwrap().clone();
    adapter.disconnect();
    assert_both_motors_fired_in_parallel(&snapshot);
}

#[test]
fn test_nxt_rotate_ports_to_position_fires_both_motors_in_parallel() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    // Mock tacho=0, target=90 → delta=90 for both ports.
    adapter.rotate_ports_to_position(&commands, 90).unwrap();
    let snapshot = sent.lock().unwrap().clone();
    adapter.disconnect();
    assert_both_motors_fired_in_parallel(&snapshot);
}

#[test]
fn test_nxt_run_ports_for_time_fires_both_motors_in_parallel() {
    let (mut adapter, sent) = make_adapter_with_mock();
    let commands = vec![
        PortCommand { port: "a", direction: PortDirection::Even, power: 50 },
        PortCommand { port: "b", direction: PortDirection::Even, power: 50 },
    ];
    // tenths=1 → 100ms. Deadline-based stop fires inside the slot, so by
    // the time this call returns both motors have already been started and
    // stopped.
    adapter.run_ports_for_time(&commands, 1).unwrap();
    let snapshot = sent.lock().unwrap().clone();
    adapter.disconnect();
    assert_both_motors_fired_in_parallel(&snapshot);
}

#[test]
fn test_nxt_rotate_to_abs_errors() {
    let (mut adapter, _sent) = make_adapter_with_mock();
    let r = adapter.rotate_to_abs("a", PortDirection::Even, 50, 0);
    adapter.disconnect();
    assert!(r.is_err(), "NXT should reject rotate_to_abs");
    let msg = r.unwrap_err();
    assert!(msg.contains("absolute"));
}

#[test]
fn test_nxt_rotate_ports_to_abs_errors() {
    let (mut adapter, _sent) = make_adapter_with_mock();
    let commands = vec![PortCommand {
        port: "a",
        direction: PortDirection::Even,
        power: 50,
    }];
    let r = adapter.rotate_ports_to_abs(&commands, 0);
    adapter.disconnect();
    assert!(r.is_err(), "NXT should reject rotate_ports_to_abs");
}

#[test]
fn test_nxt_rotate_to_position_skips_wire_when_delta_is_zero() {
    let (mut adapter, sent) = make_adapter_with_mock();
    // Mock tacho=0 and target=0 → delta=0 → should not send another
    // SET_OUTPUT_STATE, only the initial GET_OUTPUT_STATE to read position.
    adapter
        .rotate_port_to_position("a", PortDirection::Even, 50, 0)
        .unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let motor_fires: Vec<_> = sent.iter().filter(|pkt| is_fire(pkt)).collect();
    assert!(
        motor_fires.is_empty(),
        "expected no motor start fires when delta is 0; saw {}",
        motor_fires.len()
    );
}

#[test]
fn test_nxt_read_sensor_caches_input_mode() {
    let (mut adapter, sent) = make_adapter_with_mock();
    adapter.read_sensor("1", Some("touch")).unwrap();
    adapter.read_sensor("1", Some("touch")).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let set_mode = packets_with_opcode(&sent, p::OP_SET_INPUT_MODE);
    assert_eq!(
        set_mode.len(),
        1,
        "repeated read of the same sensor mode should not re-configure the port"
    );
}

#[test]
fn test_nxt_read_sensor_reconfigures_when_mode_changes() {
    let (mut adapter, sent) = make_adapter_with_mock();
    adapter.read_sensor("1", Some("touch")).unwrap();
    adapter.read_sensor("1", Some("light")).unwrap();
    adapter.disconnect();

    let sent = sent.lock().unwrap();
    let set_mode = packets_with_opcode(&sent, p::OP_SET_INPUT_MODE);
    assert_eq!(set_mode.len(), 2, "changing mode should trigger re-configure");
}

#[test]
fn test_nxt_validate_sensor_port_rejects_unknown_mode() {
    let (adapter, _sent) = make_adapter_with_mock();
    let r = adapter.validate_sensor_port("1", Some("gyro"));
    assert!(r.is_err());
}

#[test]
fn test_nxt_output_ports_are_three() {
    let (adapter, _sent) = make_adapter_with_mock();
    assert_eq!(adapter.output_ports(), &["a", "b", "c"]);
}

#[test]
fn test_nxt_input_ports_are_four() {
    let (adapter, _sent) = make_adapter_with_mock();
    assert_eq!(adapter.input_ports(), &["1", "2", "3", "4"]);
}
