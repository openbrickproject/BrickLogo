//! Tag identity decoding.
//!
//! Ported from `node-toypad` (`src/tag.ts`), with that JS as the oracle (see
//! the crypto vectors in `tests/tag.rs`). Vehicle ids are stored in plaintext
//! on the tag; character ids are TEA-encrypted with a key derived from the
//! tag's 7-byte UID and are only trusted when the decrypt self-validates
//! (`v0 == v1`).

const VEHICLE_MARKER: [u8; 4] = [0x00, 0x01, 0x00, 0x00];
const TEA_DELTA: u32 = 0x9e37_79b9;
const TEA_SUM_INIT: u32 = 0xc6ef_3720;

/// Whether a tag's identity page describes a vehicle or a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    Vehicle,
    Character,
}

/// A resolved tag identity (a numeric id; names are out of scope).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    Character(u16),
    Vehicle(u16),
}

/// True if the 4-byte marker (page 0x24 bytes [8..12]) identifies a vehicle.
pub fn is_vehicle(marker: &[u8]) -> bool {
    marker.len() >= 4 && marker[..4] == VEHICLE_MARKER
}

/// Classify a tag from its identity-page marker bytes. Anything that isn't a
/// vehicle is assumed to be a character (and validated later by the decrypt).
pub fn detect_tag_type(marker: &[u8]) -> TagType {
    if is_vehicle(marker) {
        TagType::Vehicle
    } else {
        TagType::Character
    }
}

/// Read the plaintext vehicle id from the identity page (little-endian, bytes
/// [0..2]).
pub fn get_vehicle_id(block: &[u8]) -> u16 {
    ((block[1] as u16) << 8) | (block[0] as u16)
}

/// Decrypt and validate a character id. Returns `None` when the decrypt fails
/// its self-check (`v0 != v1`) — i.e. the tag is not a valid Dimensions
/// character (a blank/foreign tag, or a read error).
pub fn get_character_id(uid: &[u8; 7], encrypted: &[u8]) -> Option<u16> {
    if encrypted.len() < 8 {
        return None;
    }
    let key = generate_keys(uid);
    let v0 = u32::from_le_bytes([encrypted[0], encrypted[1], encrypted[2], encrypted[3]]);
    let v1 = u32::from_le_bytes([encrypted[4], encrypted[5], encrypted[6], encrypted[7]]);
    let (d0, d1) = tea_decrypt(v0, v1, &key);
    if d0 != d1 {
        return None;
    }
    Some((d0 & 0xffff) as u16)
}

/// Resolve a tag's identity from its UID and the 16-byte identity page (0x24).
/// `None` means the tag couldn't be identified (and should appear in `tags`
/// only, never in `characters`/`vehicles`).
pub fn identify(uid: &[u8; 7], block24: &[u8]) -> Option<Identity> {
    if block24.len() < 12 {
        return None;
    }
    match detect_tag_type(&block24[8..12]) {
        TagType::Vehicle => Some(Identity::Vehicle(get_vehicle_id(block24))),
        TagType::Character => get_character_id(uid, &block24[0..8]).map(Identity::Character),
    }
}

fn rotate_right(value: u32, count: u32) -> u32 {
    value.rotate_right(count & 31)
}

fn scramble(uid: &[u8; 7], count: usize) -> u32 {
    let mut base: [u8; 24] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xb7, 0xd5, 0xd7, 0xe6, 0xe7, 0xba, 0x3c, 0xa8,
        0xd8, 0x75, 0x47, 0x68, 0xcf, 0x23, 0xe9, 0xfe, 0xaa,
    ];
    base[..7].copy_from_slice(uid);
    base[count * 4 - 1] = 0xaa;

    let mut v2: u32 = 0;
    for i in 0..count {
        let b = u32::from_le_bytes([
            base[i * 4],
            base[i * 4 + 1],
            base[i * 4 + 2],
            base[i * 4 + 3],
        ]);
        v2 = b
            .wrapping_add(rotate_right(v2, 25))
            .wrapping_add(rotate_right(v2, 10))
            .wrapping_sub(v2);
    }
    v2
}

fn generate_keys(uid: &[u8; 7]) -> [u32; 4] {
    [
        scramble(uid, 3),
        scramble(uid, 4),
        scramble(uid, 5),
        scramble(uid, 6),
    ]
}

fn tea_decrypt(mut v0: u32, mut v1: u32, key: &[u32; 4]) -> (u32, u32) {
    let mut sum = TEA_SUM_INIT;
    let (k0, k1, k2, k3) = (key[0], key[1], key[2], key[3]);
    for _ in 0..32 {
        v1 = v1.wrapping_sub(
            (v0.wrapping_shl(4).wrapping_add(k2))
                ^ v0.wrapping_add(sum)
                ^ (v0.wrapping_shr(5).wrapping_add(k3)),
        );
        v0 = v0.wrapping_sub(
            (v1.wrapping_shl(4).wrapping_add(k0))
                ^ v1.wrapping_add(sum)
                ^ (v1.wrapping_shr(5).wrapping_add(k1)),
        );
        sum = sum.wrapping_sub(TEA_DELTA);
    }
    (v0, v1)
}

#[cfg(test)]
#[path = "tests/tag.rs"]
mod tests;
