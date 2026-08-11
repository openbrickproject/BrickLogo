use super::*;

#[test]
fn test_encode_set_outputs_masks_to_six_bits() {
    assert_eq!(encode_set_outputs(0x00), "D00\n");
    assert_eq!(encode_set_outputs(0x3F), "D3F\n");
    // Only the low six bits are wired; higher bits are masked off.
    assert_eq!(encode_set_outputs(0xFF), "D3F\n");
    assert_eq!(encode_set_outputs(0x01), "D01\n");
}

#[test]
fn test_encode_batch_concatenates_frames_in_order() {
    let batch = encode_batch(&[0x01, 0x02, 0x00]);
    assert_eq!(batch, "D01\nD02\nD00\n");
}

#[test]
fn test_encode_batch_empty() {
    assert_eq!(encode_batch(&[]), "");
}

#[test]
fn test_parse_line_input_report() {
    assert_eq!(parse_line("I40"), Line::Input(0x40));
    assert_eq!(parse_line("I80"), Line::Input(0x80));
    assert_eq!(parse_line("Ic0"), Line::Input(0xC0));
}

#[test]
fn test_parse_line_input_report_strips_trailing_cr() {
    assert_eq!(parse_line("I40\r"), Line::Input(0x40));
}

#[test]
fn test_parse_line_replies_are_not_input() {
    assert_eq!(parse_line("CF"), Line::Reply("CF".to_string()));
    assert_eq!(parse_line("V1.0.1"), Line::Reply("V1.0.1".to_string()));
    assert_eq!(parse_line("BOOT"), Line::Reply("BOOT".to_string()));
}

#[test]
fn test_parse_line_malformed_input_falls_back_to_reply() {
    // Not two hex digits after 'I' — not a valid input report.
    assert_eq!(parse_line("IZZ"), Line::Reply("IZZ".to_string()));
    assert_eq!(parse_line("I4"), Line::Reply("I4".to_string()));
}

#[test]
fn test_drain_lines_splits_on_lf_and_trims_cr() {
    let mut buf = b"CF\r\nI40\nV1.0.1\r\n".to_vec();
    let lines = drain_lines(&mut buf);
    assert_eq!(
        lines,
        vec![
            Line::Reply("CF".to_string()),
            Line::Input(0x40),
            Line::Reply("V1.0.1".to_string()),
        ]
    );
    assert!(buf.is_empty());
}

#[test]
fn test_drain_lines_leaves_partial_trailing_line() {
    let mut buf = b"CF\r\nI4".to_vec();
    let lines = drain_lines(&mut buf);
    assert_eq!(lines, vec![Line::Reply("CF".to_string())]);
    assert_eq!(buf, b"I4");
}

#[test]
fn test_drain_lines_drops_empty_lines() {
    // A bare '\n' with nothing before it (not even a '\r') is a true empty
    // line and is dropped.
    let mut buf = b"\nCF\r\n".to_vec();
    let lines = drain_lines(&mut buf);
    assert_eq!(lines, vec![Line::Reply("CF".to_string())]);
}
