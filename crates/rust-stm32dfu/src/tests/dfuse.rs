use super::*;
use crc32fast::Hasher;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::io::Write;

fn build_dfuse(targets: Vec<(u8, &str, Vec<Element>)>, vendor: u16, product: u16, device: u16) -> Vec<u8> {
    let mut body = Vec::new();
    // Prefix
    body.extend_from_slice(b"DfuSe");
    body.push(0x01);
    let size_placeholder = body.len();
    body.extend_from_slice(&[0u8; 4]); // dwDfuFileSize, patched later
    body.push(targets.len() as u8);

    for (alt, name, elements) in &targets {
        body.extend_from_slice(b"Target");
        body.push(*alt);
        body.extend_from_slice(&1u32.to_le_bytes()); // bTargetNamed
        let mut name_buf = [0u8; 255];
        let name_bytes = name.as_bytes();
        name_buf[..name_bytes.len()].copy_from_slice(name_bytes);
        body.extend_from_slice(&name_buf);
        let mut payload = Vec::new();
        for el in elements {
            payload.extend_from_slice(&el.address.to_le_bytes());
            payload.extend_from_slice(&(el.data.len() as u32).to_le_bytes());
            payload.extend_from_slice(&el.data);
        }
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // dwTargetSize
        body.extend_from_slice(&(elements.len() as u32).to_le_bytes()); // dwNElements
        body.extend_from_slice(&payload);
    }

    // Patch dwDfuFileSize (everything before the 16-byte suffix).
    let size = body.len() as u32;
    body[size_placeholder..size_placeholder + 4].copy_from_slice(&size.to_le_bytes());

    // Suffix (first 12 bytes are covered by CRC, the last 4 are the CRC itself).
    body.extend_from_slice(&device.to_le_bytes());
    body.extend_from_slice(&product.to_le_bytes());
    body.extend_from_slice(&vendor.to_le_bytes());
    body.extend_from_slice(&0x011Au16.to_le_bytes()); // bcdDFU
    body.extend_from_slice(b"UFD");
    body.push(0x10); // bLength

    let mut hasher = Hasher::new();
    hasher.update(&body);
    let crc = !hasher.finalize();
    body.extend_from_slice(&crc.to_le_bytes());
    body
}

#[test]
fn test_parse_simple() {
    let elements = vec![Element { address: 0x0800_0000, data: vec![0xAA, 0xBB, 0xCC] }];
    let bytes = build_dfuse(
        vec![(0, "STM32F413", elements.clone())],
        0x0483,
        0xDF11,
        0x2200,
    );
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.vendor, 0x0483);
    assert_eq!(parsed.product, 0xDF11);
    assert_eq!(parsed.device, 0x2200);
    assert_eq!(parsed.targets.len(), 1);
    assert_eq!(parsed.targets[0].alt, 0);
    assert_eq!(parsed.targets[0].name, "STM32F413");
    assert_eq!(parsed.targets[0].elements, elements);
}

#[test]
fn test_parse_gz_roundtrip() {
    let elements = vec![Element { address: 0x0800_4000, data: vec![1, 2, 3, 4, 5, 6, 7, 8] }];
    let bytes = build_dfuse(vec![(0, "STM32H562", elements.clone())], 0x0483, 0xDF11, 0x0001);
    let mut gz = Vec::new();
    {
        let mut encoder = GzEncoder::new(&mut gz, Compression::default());
        encoder.write_all(&bytes).unwrap();
        encoder.finish().unwrap();
    }
    let parsed = parse_gz(&gz).unwrap();
    assert_eq!(parsed.targets[0].elements, elements);
    assert_eq!(parsed.targets[0].name, "STM32H562");
}

#[test]
fn test_missing_magic_errors() {
    let mut bytes = build_dfuse(vec![(0, "x", vec![])], 0, 0, 0);
    bytes[0] = b'X';
    assert!(parse(&bytes).is_err());
}

#[test]
fn test_crc_mismatch_errors() {
    let mut bytes = build_dfuse(vec![(0, "x", vec![])], 0, 0, 0);
    let n = bytes.len();
    bytes[n - 1] ^= 0xFF;
    assert!(parse(&bytes).is_err());
}

#[test]
fn test_multiple_elements() {
    let elements = vec![
        Element { address: 0x0800_0000, data: vec![0xAA; 16] },
        Element { address: 0x0800_1000, data: vec![0xBB; 32] },
    ];
    let bytes = build_dfuse(vec![(0, "STM32F413", elements.clone())], 0, 0, 0);
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.targets[0].elements.len(), 2);
    assert_eq!(parsed.targets[0].elements[1].address, 0x0800_1000);
}

/// The bundled SPIKE DfuSe files use an opaque `"ST..."` target name (LEGO
/// strips the chip id) — chip identification at runtime comes from the USB
/// iProduct string instead. We just assert the files parse and carry a
/// non-empty element targeted at flash.
#[test]
fn test_bundled_spike_prime_f4_parses() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../firmware/spike-prime/prime-f4-hubos-3.4.0-dfuse.gz");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(&path).unwrap();
    let parsed = parse_gz(&bytes).unwrap();
    assert!(!parsed.targets.is_empty());
    assert!(!parsed.targets[0].elements.is_empty());
    let first = &parsed.targets[0].elements[0];
    assert!(first.address >= 0x0800_0000, "expected flash address, got {:#x}", first.address);
    assert!(first.data.len() > 1024);
}

#[test]
fn test_bundled_spike_prime_h5_parses() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../firmware/spike-prime/prime-h5-hubos-3.4.0-dfuse.gz");
    if !path.exists() {
        return;
    }
    let bytes = std::fs::read(&path).unwrap();
    let parsed = parse_gz(&bytes).unwrap();
    assert!(!parsed.targets.is_empty());
    assert!(!parsed.targets[0].elements.is_empty());
}
