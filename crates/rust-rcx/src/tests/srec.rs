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
