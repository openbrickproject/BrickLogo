use super::*;

#[test]
fn test_agent_source_nonempty() {
    assert!(!AGENT_SOURCE.is_empty());
    assert!(AGENT_SOURCE.contains("module_tunnel"));
    assert!(AGENT_SOURCE.contains("runloop.run(_main(), _heartbeat(), _port_watcher())"));
    // Must match the Rust-side opcodes exactly.
    assert!(AGENT_SOURCE.contains("OP_MOTOR_RUN = 0x01"));
    assert!(AGENT_SOURCE.contains("R_READY = 0x10"));
}

#[test]
fn test_agent_source_has_port_type_wiring() {
    // Pin the port-type protocol surface — if any of these go missing the
    // adapter's device-type validation silently breaks for SPIKE.
    assert!(AGENT_SOURCE.contains("OP_PORT_TYPES = 0x0D"));
    assert!(AGENT_SOURCE.contains("R_TYPE_LIST = 0x05"));
    assert!(AGENT_SOURCE.contains("R_PORT_EVENT = 0x12"));
    // Background poller pushes diffs only — no idle traffic.
    assert!(AGENT_SOURCE.contains("_port_watcher"));
    assert!(AGENT_SOURCE.contains("_port_types_cache"));
    // Snapshot handler must be present.
    assert!(AGENT_SOURCE.contains("elif op == OP_PORT_TYPES:"));
    // SPIKE 3 device introspection — `device.id(port)` returns the LPF2 type
    // ID and raises when nothing is attached.
    assert!(AGENT_SOURCE.contains("import hub, motor, device"));
    assert!(AGENT_SOURCE.contains("device.id"));
    // PWM dispatch path for non-tacho devices (LEDs, passive motors).
    // Lights and passive motors share `device.set_duty_cycle` — the `motor`
    // module ENODEVs on passive motors so we can't use it here.
    assert!(AGENT_SOURCE.contains("OP_PORT_PWM = 0x0E"));
    assert!(AGENT_SOURCE.contains("elif op == OP_PORT_PWM:"));
    assert!(AGENT_SOURCE.contains("device.set_duty_cycle"));
}

#[test]
fn test_agent_crc32_stable() {
    assert_eq!(agent_crc32(), agent_crc32());
}
