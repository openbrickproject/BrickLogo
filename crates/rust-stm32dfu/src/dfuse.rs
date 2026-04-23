//! ST DfuSe file format parser.
//!
//! File layout:
//!
//! ```text
//! Prefix (11 bytes):
//!   [0..5]    b"DfuSe"
//!   [5]       bcdDfuSe            (u8, typically 0x01)
//!   [6..10]   dwDfuFileSize       (u32 LE, total file size excl. 16-byte suffix)
//!   [10]      bTargets            (u8, target count)
//!
//! Per target (274-byte header + payload):
//!   [0..6]    b"Target"
//!   [6]       bAlternateSetting   (u8)
//!   [7..11]   bTargetNamed        (u32 LE, 0 or 1)
//!   [11..266] szTargetName        (255 bytes, NUL-padded)
//!   [266..270] dwTargetSize       (u32 LE, payload bytes)
//!   [270..274] dwNElements        (u32 LE)
//!   then `dwTargetSize` bytes of element data:
//!     Element:
//!       [0..4]  dwElementAddress  (u32 LE)
//!       [4..8]  dwElementSize     (u32 LE)
//!       [8..]   data
//!
//! Suffix (16 bytes, last in file):
//!   [0..2]    bcdDevice           (u16 LE)
//!   [2..4]    idProduct           (u16 LE)
//!   [4..6]    idVendor            (u16 LE)
//!   [6..8]    bcdDFU              (u16 LE, 0x011A for DfuSe)
//!   [8..11]   ucDfuSig            (b"UFD" reversed on wire, stored as "UFD" reading LSB-first)
//!   [11]      bLength             (u8, 0x10)
//!   [12..16]  dwCRC               (u32 LE, CRC32 over all preceding bytes)
//! ```

use crc32fast::Hasher;
use flate2::read::GzDecoder;
use std::io::Read;

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub address: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub alt: u8,
    pub name: String,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DfuSeFile {
    pub vendor: u16,
    pub product: u16,
    pub device: u16,
    pub targets: Vec<Target>,
}

/// Decompress a gzipped `.dfuse.gz` blob, then parse.
pub fn parse_gz(bytes: &[u8]) -> Result<DfuSeFile> {
    let mut decoder = GzDecoder::new(bytes);
    let mut buf = Vec::with_capacity(bytes.len() * 2);
    decoder.read_to_end(&mut buf)?;
    parse(&buf)
}

/// Parse a DfuSe image from already-decompressed bytes.
pub fn parse(bytes: &[u8]) -> Result<DfuSeFile> {
    if bytes.len() < 11 + 16 {
        return Err(Error::Parse("file too small for DfuSe".into()));
    }
    if &bytes[0..5] != b"DfuSe" {
        return Err(Error::Parse("missing DfuSe magic".into()));
    }

    let suffix_start = bytes.len() - 16;
    let suffix = &bytes[suffix_start..];
    // The DFU signature is stored in reverse byte order: wire bytes are
    // "DFU" (read LSB-first → "UFD").
    if &suffix[8..11] != b"UFD" {
        return Err(Error::Parse("missing UFD signature in DFU suffix".into()));
    }
    if suffix[11] != 0x10 {
        return Err(Error::Parse(format!(
            "bad DFU suffix length {:#x}", suffix[11]
        )));
    }

    // CRC32 is computed over every byte before the CRC field itself.
    let declared_crc = u32::from_le_bytes(suffix[12..16].try_into().unwrap());
    let mut hasher = Hasher::new();
    hasher.update(&bytes[..suffix_start + 12]);
    let computed_crc = !hasher.finalize();
    if declared_crc != computed_crc {
        return Err(Error::Parse(format!(
            "DFU suffix CRC mismatch: declared {:#010x}, computed {:#010x}",
            declared_crc, computed_crc
        )));
    }

    let vendor = u16::from_le_bytes(suffix[4..6].try_into().unwrap());
    let product = u16::from_le_bytes(suffix[2..4].try_into().unwrap());
    let device = u16::from_le_bytes(suffix[0..2].try_into().unwrap());

    let n_targets = bytes[10];
    let mut cursor = 11;
    let mut targets = Vec::with_capacity(n_targets as usize);

    for _ in 0..n_targets {
        if cursor + 274 > suffix_start {
            return Err(Error::Parse("target header truncated".into()));
        }
        if &bytes[cursor..cursor + 6] != b"Target" {
            return Err(Error::Parse("missing Target magic".into()));
        }
        let alt = bytes[cursor + 6];
        let name_bytes = &bytes[cursor + 11..cursor + 266];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
        let target_size = u32::from_le_bytes(bytes[cursor + 266..cursor + 270].try_into().unwrap()) as usize;
        let n_elements = u32::from_le_bytes(bytes[cursor + 270..cursor + 274].try_into().unwrap()) as usize;
        cursor += 274;

        if cursor + target_size > suffix_start {
            return Err(Error::Parse("target payload exceeds file".into()));
        }
        let payload_end = cursor + target_size;
        let mut elements = Vec::with_capacity(n_elements);
        for _ in 0..n_elements {
            if cursor + 8 > payload_end {
                return Err(Error::Parse("element header truncated".into()));
            }
            let address = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            let size = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
            cursor += 8;
            if cursor + size > payload_end {
                return Err(Error::Parse("element data truncated".into()));
            }
            let data = bytes[cursor..cursor + size].to_vec();
            cursor += size;
            elements.push(Element { address, data });
        }
        // Skip any slack inside the target payload (the format technically allows it).
        cursor = payload_end;

        targets.push(Target { alt, name, elements });
    }

    Ok(DfuSeFile { vendor, product, device, targets })
}

#[cfg(test)]
#[path = "tests/dfuse.rs"]
mod tests;
