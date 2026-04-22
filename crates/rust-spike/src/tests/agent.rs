use super::*;

#[test]
fn test_agent_source_nonempty() {
    assert!(!AGENT_SOURCE.is_empty());
    assert!(AGENT_SOURCE.contains("module_tunnel"));
    assert!(AGENT_SOURCE.contains("runloop.run(_main())"));
}

#[test]
fn test_agent_crc32_stable() {
    assert_eq!(agent_crc32(), agent_crc32());
}
