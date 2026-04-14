use super::*;

#[test]
fn lc0_positive() {
    assert_eq!(lc0_try(0), Some(0x00));
    assert_eq!(lc0_try(1), Some(0x01));
    assert_eq!(lc0_try(5), Some(0x05));
    assert_eq!(lc0_try(31), Some(0x1F));
}

#[test]
fn lc0_negative_uses_sign_bit() {
    // Sign bit is bit 5 (0x20). Magnitude in low 5 bits.
    assert_eq!(lc0_try(-1), Some(0x21));
    assert_eq!(lc0_try(-5), Some(0x25));
    assert_eq!(lc0_try(-31), Some(0x3F));
}

#[test]
fn lc0_out_of_range() {
    assert_eq!(lc0_try(32), None);
    assert_eq!(lc0_try(-32), None);
    assert_eq!(lc0_try(127), None);
    assert_eq!(lc0_try(-128), None);
}

#[test]
fn pack_lc_picks_smallest() {
    let mut b = Vec::new();
    pack_lc(&mut b, 5);
    assert_eq!(b, vec![0x05]); // LC0

    b.clear();
    pack_lc(&mut b, 50);
    assert_eq!(b, vec![0x81, 0x32]); // LC1

    b.clear();
    pack_lc(&mut b, 1000);
    assert_eq!(b, vec![0x82, 0xE8, 0x03]); // LC2

    b.clear();
    pack_lc(&mut b, 100_000);
    assert_eq!(b, vec![0x83, 0xA0, 0x86, 0x01, 0x00]); // LC4
}

#[test]
fn gv0_encoding() {
    assert_eq!(gv0(0), 0x60);
    assert_eq!(gv0(1), 0x61);
    assert_eq!(gv0(31), 0x7F);
}

#[test]
fn direct_header_packs_sizes() {
    assert_eq!(direct_header(0, 0), 0x0000);
    assert_eq!(direct_header(1, 0), 0x0001);
    // 4 bytes global, 0 local → 0x0004
    assert_eq!(direct_header(4, 0), 0x0004);
    // 0 global, 8 local → 0b 00001000 << 10 = 0x2000
    assert_eq!(direct_header(0, 8), 0x2000);
}

#[test]
fn output_power_frame_bytes() {
    // Set port A (mask 0x01) power to 50. Counter = 1.
    // Expected wire bytes, no reply:
    //   length=0x000A, counter=0x0001, type=0x80, header=0x0000,
    //   body = opOUTPUT_POWER(0xA4), LC0(0)=0x00, LC0(1)=0x01, LC1(50)=0x81,0x32
    let frame = cmd_output_power(1, 0x01, 50);
    let bytes = frame.encode();
    assert_eq!(
        bytes,
        vec![0x0A, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0xA4, 0x00, 0x01, 0x81, 0x32]
    );
}

#[test]
fn output_start_frame_bytes() {
    let frame = cmd_output_start(1, 0x01);
    let bytes = frame.encode();
    // length=8, counter=1, type=0x80, header=0x0000, body=opOUTPUT_START 0 1
    assert_eq!(
        bytes,
        vec![0x08, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0xA6, 0x00, 0x01]
    );
}

#[test]
fn output_test_busy_frame_has_reply_buffer() {
    // 1 byte reply expected
    let frame = cmd_output_test_busy(42, 0x0F);
    assert_eq!(frame.message_type, MessageType::DirectCmdReply);
    assert_eq!(frame.header, 0x0001); // 1 global byte, 0 local
    // Body should end with gv0(0) = 0x60
    assert_eq!(*frame.body.last().unwrap(), 0x60);
}

#[test]
fn output_get_count_reserves_4_bytes() {
    let frame = cmd_output_get_count(1, 0);
    assert_eq!(frame.message_type, MessageType::DirectCmdReply);
    assert_eq!(frame.header, 0x0004); // i32 = 4 bytes
}

#[test]
fn input_read_pct_frame_shape() {
    // Port 0, sensor type 29 (EV3 color), mode 0 (color ID). Counter 7.
    let frame = cmd_input_read_pct(7, 0, 29, 0);
    assert_eq!(frame.message_type, MessageType::DirectCmdReply);
    assert_eq!(frame.header, 0x0001);
    // Body starts with opINPUT_DEVICE, then LC0(SUBCMD_READY_PCT=27).
    // 27 fits in LC0. Encoding: magnitude 27 (0x1B), no sign bit.
    assert_eq!(frame.body[0], OP_INPUT_DEVICE);
    assert_eq!(frame.body[1], 0x1B);
}
