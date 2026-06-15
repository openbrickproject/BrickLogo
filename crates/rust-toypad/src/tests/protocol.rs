use super::*;
use crate::constants::{Action, Panel};

#[test]
fn encodes_commands_into_padded_packets() {
    let cmd = Command { id: 0xc0, params: vec![Panel::Left as u8, 0xaa, 0xbb, 0xcc] };
    let encoded = encode_command(&cmd, 1);
    assert_eq!(encoded.len(), 32);
    assert_eq!(encoded[0], 0x55);
    assert_eq!(encoded[1], 6); // params.len()+2
    assert_eq!(encoded[2], 0xc0);
    assert_eq!(encoded[3], 0x01);
}

#[test]
fn encode_command_appends_sum_checksum() {
    let cmd = Command { id: 0xc0, params: vec![0x02, 0xaa, 0xbb, 0xcc] };
    let encoded = encode_command(&cmd, 1);
    // checksum = sum of [0x55,0x06,0xc0,0x01,0x02,0xaa,0xbb,0xcc] & 0xff
    let expected = [0x55u8, 0x06, 0xc0, 0x01, 0x02, 0xaa, 0xbb, 0xcc]
        .iter()
        .fold(0u8, |a, b| a.wrapping_add(*b));
    assert_eq!(encoded[8], expected);
}

#[test]
fn builds_set_color_with_rgb_components() {
    let cmd = create_set_color(Panel::Right, (0x12, 0x34, 0x56));
    assert_eq!(cmd.id, 0xc0);
    assert_eq!(cmd.params, vec![Panel::Right as u8, 0x12, 0x34, 0x56]);
}

#[test]
fn builds_set_color_all_with_per_pad_colors() {
    let cmd = create_set_color_all(
        Some((0xff, 0x00, 0x00)),
        Some((0x00, 0xff, 0x00)),
        Some((0x00, 0x00, 0xff)),
    );
    assert_eq!(cmd.id, 0xc8);
    assert_eq!(
        cmd.params,
        vec![1, 0xff, 0x00, 0x00, 1, 0x00, 0xff, 0x00, 1, 0x00, 0x00, 0xff]
    );
}

#[test]
fn builds_set_color_all_with_none_pads_skipped() {
    let cmd = create_set_color_all(None, Some((0x00, 0xff, 0x00)), None);
    assert_eq!(
        cmd.params,
        vec![0, 0, 0, 0, 1, 0x00, 0xff, 0x00, 0, 0, 0, 0]
    );
}

#[test]
fn truncates_oversized_command_packets() {
    let cmd = Command { id: 0xc0, params: vec![0xff; 40] };
    let encoded = encode_command(&cmd, 1);
    assert_eq!(encoded.len(), 32);
}

#[test]
fn decodes_action_events() {
    let mut bytes = [0u8; 32];
    let payload = [
        0x56, 0x0b, 0x02, 0x00, 0x01, 0x00, 0x04, 0x64, 0x74, 0xfa, 0x00, 0x49, 0x81,
    ];
    bytes[..payload.len()].copy_from_slice(&payload);
    match decode_message(&bytes) {
        Some(Incoming::Event(ev)) => {
            assert_eq!(ev.panel, Panel::Left);
            assert_eq!(ev.action, Action::Add);
            assert_eq!(ev.index, 1);
            assert_eq!(format_uid(&ev.uid), "04 64 74 fa 00 49 81");
        }
        other => panic!("expected event, got {:?}", other),
    }
}

#[test]
fn ignores_invalid_action_events_and_unknown_packets() {
    assert_eq!(decode_message(&[0x56, 0xff, 0x00]), None);
    assert_eq!(decode_message(&[0x10, 0x00, 0x00]), None);
}

#[test]
fn decodes_response_messages_with_payloads() {
    let mut bytes = [0u8; 32];
    bytes[..6].copy_from_slice(&[0x55, 0x04, 0x10, 0xde, 0xad, 0xbe]);
    match decode_message(&bytes) {
        Some(Incoming::Response { request_id, payload }) => {
            assert_eq!(request_id, 0x10);
            assert_eq!(payload, vec![0xde, 0xad, 0xbe]);
        }
        other => panic!("expected response, got {:?}", other),
    }
}

#[test]
fn strips_leading_hid_report_id() {
    // A leading 0x00 report-id byte must be stripped before decoding.
    let mut bytes = vec![0x00, 0x55, 0x04, 0x10, 0xde, 0xad, 0xbe];
    bytes.resize(33, 0);
    match decode_message(&bytes) {
        Some(Incoming::Response { request_id, .. }) => assert_eq!(request_id, 0x10),
        other => panic!("expected response, got {:?}", other),
    }
}

#[test]
fn returns_none_for_empty_or_truncated_responses() {
    assert_eq!(decode_message(&[]), None);
    assert_eq!(decode_message(&[0x55, 0x01, 0x10]), None);
}

#[test]
fn builds_list_tags_with_no_params() {
    let cmd = create_list_tags();
    assert_eq!(cmd.id, 0xd0);
    assert!(cmd.params.is_empty());
}

#[test]
fn decodes_list_tags_responses() {
    let entries = decode_list_tags(&[0x30, 0x00, 0x21, 0x00, 0x14, 0x08]);
    assert_eq!(
        entries,
        vec![
            ListEntry { panel: Panel::Right, index: 0, ok: true },
            ListEntry { panel: Panel::Left, index: 1, ok: true },
            ListEntry { panel: Panel::Center, index: 4, ok: false },
        ]
    );
}

#[test]
fn skips_list_tags_entries_with_invalid_panels() {
    let entries = decode_list_tags(&[0x00, 0x00, 0x21, 0x00]);
    assert_eq!(entries, vec![ListEntry { panel: Panel::Left, index: 1, ok: true }]);
}

#[test]
fn builds_read_tag_command() {
    let cmd = create_read_tag(1, 0x24);
    assert_eq!(cmd.id, 0xd2);
    assert_eq!(cmd.params, vec![1, 0x24]);
}
