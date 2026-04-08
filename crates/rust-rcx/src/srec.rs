/// Parsed firmware image ready for upload to the RCX.
#[derive(Debug, Clone)]
pub struct FirmwareImage {
    /// Binary image data.
    pub data: Vec<u8>,
    /// Base address (typically 0x8000).
    pub base_address: u16,
    /// Entry point address from S9 record.
    pub entry_point: u16,
    /// Checksum: sum of all data bytes (wrapping).
    pub checksum: u16,
}

/// Parse a Motorola S-Record file into a firmware image.
/// Handles S0 (header), S1 (data with 16-bit address), S9 (entry point).
pub fn parse_srec(content: &str) -> Result<FirmwareImage, String> {
    let mut min_addr: Option<u16> = None;
    let mut max_addr: u16 = 0;
    let mut entry_point: u16 = 0;
    let mut records: Vec<(u16, Vec<u8>)> = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() { continue; }

        if line.len() < 2 || !line.starts_with('S') {
            return Err(format!("Line {}: not an S-Record", line_num + 1));
        }

        let record_type = line.as_bytes()[1];
        match record_type {
            b'0' => continue, // Header record, skip
            b'1' => {
                let bytes = decode_hex_line(&line[2..], line_num)?;
                verify_checksum(&bytes, line_num)?;

                if bytes.len() < 3 {
                    return Err(format!("Line {}: S1 record too short", line_num + 1));
                }

                let addr = u16::from_be_bytes([bytes[1], bytes[2]]);
                let data = &bytes[3..bytes.len() - 1]; // exclude checksum byte

                let end_addr = addr.checked_add(data.len() as u16)
                    .ok_or_else(|| format!("Line {}: address overflow", line_num + 1))?;

                min_addr = Some(min_addr.map_or(addr, |m: u16| m.min(addr)));
                max_addr = max_addr.max(end_addr);

                records.push((addr, data.to_vec()));
            }
            b'9' => {
                let bytes = decode_hex_line(&line[2..], line_num)?;
                verify_checksum(&bytes, line_num)?;
                if bytes.len() >= 3 {
                    entry_point = u16::from_be_bytes([bytes[1], bytes[2]]);
                }
            }
            _ => continue, // Ignore unknown record types
        }
    }

    if records.is_empty() {
        return Err("No data records found in S-Record file".to_string());
    }

    let base_address = min_addr.unwrap();
    let image_len = (max_addr - base_address) as usize;

    if image_len > 0x7000 {
        return Err(format!("Firmware image too large: {} bytes (max 28672)", image_len));
    }

    let mut data = vec![0u8; image_len];
    for (addr, record_data) in &records {
        let offset = (*addr - base_address) as usize;
        data[offset..offset + record_data.len()].copy_from_slice(record_data);
    }

    // Checksum only covers bytes up to address 0xCC00.
    // Bytes at 0xCC00 and beyond contain the signature string and are excluded.
    let checksum_end = (0xCC00u16.saturating_sub(base_address)) as usize;
    let checksum_slice = if checksum_end < data.len() { &data[..checksum_end] } else { &data };
    let checksum: u16 = checksum_slice.iter().fold(0u16, |acc, &b| acc.wrapping_add(b as u16));

    Ok(FirmwareImage {
        data,
        base_address,
        entry_point,
        checksum,
    })
}

fn decode_hex_line(hex: &str, line_num: usize) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err(format!("Line {}: odd hex length", line_num + 1));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|_| format!("Line {}: invalid hex at position {}", line_num + 1, i))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn verify_checksum(bytes: &[u8], line_num: usize) -> Result<(), String> {
    if bytes.is_empty() {
        return Err(format!("Line {}: empty record", line_num + 1));
    }
    // Byte count is first byte, checksum is last byte.
    // Sum of all bytes (including count and checksum) should be 0xFF.
    let sum: u8 = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    if sum != 0xFF {
        return Err(format!("Line {}: checksum error (sum={:#04x}, expected 0xFF)", line_num + 1, sum));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_srec() {
        // S0 header + S1 data at 0x8000 with 4 bytes + S9 entry point
        let srec = "\
S00600004844521B
S1078000AABBCCDD6A
S90380007C
";
        let image = parse_srec(srec).unwrap();
        assert_eq!(image.base_address, 0x8000);
        assert_eq!(image.data, vec![0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(image.entry_point, 0x8000);
        assert_eq!(image.checksum, 0xAA + 0xBB + 0xCC + 0xDD);
    }

    #[test]
    fn test_parse_multiple_records() {
        let srec = "\
S1078000AABBCCDD6A
S1078004010203046A
S90380007C
";
        let image = parse_srec(srec).unwrap();
        assert_eq!(image.base_address, 0x8000);
        assert_eq!(image.data.len(), 8);
        assert_eq!(&image.data[0..4], &[0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(&image.data[4..8], &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_empty_file() {
        let result = parse_srec("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No data records"));
    }

    #[test]
    fn test_parse_checksum_error() {
        // Corrupt the checksum (last hex byte)
        let srec = "S1078000AABBCCDDFF\nS9038000FC\n";
        let result = parse_srec(srec);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("checksum error"));
    }

    #[test]
    fn test_parse_malformed_hex() {
        let srec = "S1078000GGHHIIJJ00\n";
        let result = parse_srec(srec);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid hex"));
    }

    #[test]
    fn test_parse_image_too_large() {
        // Create a record that would exceed 0x7000 bytes
        // Address 0x8000, claiming to extend past 0xF000
        let srec = "S1058000AAD0\nS105F001BB4E\nS90380007C\n";
        let result = parse_srec(srec);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too large"));
    }

    #[test]
    fn test_decode_hex_line() {
        let bytes = decode_hex_line("0A1B2C", 0).unwrap();
        assert_eq!(bytes, vec![0x0A, 0x1B, 0x2C]);
    }

    #[test]
    fn test_verify_checksum_valid() {
        // byte_count=0x03, addr=0x80,0x00, checksum=0xFC
        // 0x03 + 0x80 + 0x00 + 0xFC = 0x17F, & 0xFF = 0x7F...
        // Actually let's use the S9 record: 03 80 00 FC
        // 03+80+00+FC = 17F, 7F != FF. Hmm.
        // The S-Record checksum: count all bytes including byte_count,
        // one's complement of sum should give the checksum byte.
        // So sum of all bytes = 0xFF.
        // 03 + 80 + 00 = 83, checksum = FF - 83 = 7C
        // Let's verify: 03 + 80 + 00 + 7C = FF. Yes.
        let bytes = vec![0x03, 0x80, 0x00, 0x7C];
        assert!(verify_checksum(&bytes, 0).is_ok());
    }

    #[test]
    fn test_verify_checksum_invalid() {
        let bytes = vec![0x03, 0x80, 0x00, 0x00];
        assert!(verify_checksum(&bytes, 0).is_err());
    }
}
