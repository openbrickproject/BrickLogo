use super::*;

#[test]
fn test_agent_source_nonempty() {
    assert!(!AGENT_SOURCE.is_empty());
    assert!(AGENT_SOURCE.contains("module_tunnel"));
    assert!(AGENT_SOURCE.contains("runloop.run(_main(), _heartbeat())"));
    // Must match the Rust-side opcodes exactly.
    assert!(AGENT_SOURCE.contains("OP_MOTOR_RUN = 0x01"));
    assert!(AGENT_SOURCE.contains("R_READY = 0x10"));
}

#[test]
fn test_agent_crc32_stable() {
    assert_eq!(agent_crc32(), agent_crc32());
}
