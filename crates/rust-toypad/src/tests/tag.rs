use super::*;

// Vectors lifted from node-toypad/tests/tag.test.ts — these validate the Rust
// TEA port against the working JS reference.

#[test]
fn detects_vehicles_from_marker_bytes() {
    let marker = [0x00, 0x01, 0x00, 0x00];
    assert!(is_vehicle(&marker));
    assert_eq!(detect_tag_type(&marker), TagType::Vehicle);
}

#[test]
fn treats_unknown_markers_as_characters() {
    let marker = [0x10, 0x20, 0x30, 0x40];
    assert!(!is_vehicle(&marker));
    assert_eq!(detect_tag_type(&marker), TagType::Character);
}

#[test]
fn reads_vehicle_ids_from_nfc_data() {
    let block = [
        0x63, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    assert_eq!(get_vehicle_id(&block), 0x0463);
}

#[test]
fn decrypts_character_ids() {
    let encrypted = [0x5c, 0xf7, 0x1c, 0xde, 0x29, 0xad, 0xea, 0x08];
    let uid = [0x04, 0x47, 0x37, 0xe2, 0x48, 0x3f, 0x80];
    assert_eq!(get_character_id(&uid, &encrypted), Some(16));
}

#[test]
fn rejects_undecryptable_character_data() {
    // Garbage that won't satisfy the v0 == v1 self-check → None (silent fail).
    let encrypted = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
    let uid = [0x04, 0x47, 0x37, 0xe2, 0x48, 0x3f, 0x80];
    assert_eq!(get_character_id(&uid, &encrypted), None);
}

#[test]
fn identify_resolves_character() {
    // 16-byte page 0x24: bytes [0..8] = encrypted character record, [8..12] is
    // a non-vehicle marker, so it's classified as a character.
    let uid = [0x04, 0x47, 0x37, 0xe2, 0x48, 0x3f, 0x80];
    let mut block = [0u8; 16];
    block[..8].copy_from_slice(&[0x5c, 0xf7, 0x1c, 0xde, 0x29, 0xad, 0xea, 0x08]);
    block[8..12].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    assert_eq!(identify(&uid, &block), Some(Identity::Character(16)));
}

#[test]
fn identify_resolves_vehicle() {
    let uid = [0x04, 0x47, 0x37, 0xe2, 0x48, 0x3f, 0x80];
    let mut block = [0u8; 16];
    block[0] = 0x63;
    block[1] = 0x04;
    block[8..12].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // vehicle marker
    assert_eq!(identify(&uid, &block), Some(Identity::Vehicle(0x0463)));
}

#[test]
fn identify_returns_none_for_unidentifiable_tag() {
    let uid = [0x04, 0x47, 0x37, 0xe2, 0x48, 0x3f, 0x80];
    let mut block = [0u8; 16];
    block[..8].copy_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]);
    block[8..12].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]); // not a vehicle
    assert_eq!(identify(&uid, &block), None);
}
