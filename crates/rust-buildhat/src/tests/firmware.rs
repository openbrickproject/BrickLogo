use super::*;
use std::collections::VecDeque;
use std::io::{Read, Write};

/// Scripted `Read + Write` that records every byte written and replays
/// canned reply fragments on read. Each reply is triggered by a *prefix*
/// in the accumulated write stream: once a write contains the trigger
/// substring, the reply bytes become available to subsequent `read` calls.
/// This lets tests model the "send command → wait for prompt" pattern the
/// Build HAT uploader uses without involving a real serial port.
struct MockPort {
    writes: Vec<u8>,
    /// How far into `writes` we've already scanned for trigger matches.
    /// Advancing this is what distinguishes the first `ETX\r` from the
    /// second — the second `\x03\r` trigger only matches bytes written
    /// after the first match was consumed.
    scan_pos: usize,
    outgoing: VecDeque<u8>,
    script: VecDeque<(String, Vec<u8>)>,
}

impl MockPort {
    fn new(script: Vec<(&str, &[u8])>) -> Self {
        MockPort {
            writes: Vec::new(),
            scan_pos: 0,
            outgoing: VecDeque::new(),
            script: script
                .into_iter()
                .map(|(t, r)| (t.to_string(), r.to_vec()))
                .collect(),
        }
    }

    fn writes_as_string(&self) -> String {
        String::from_utf8_lossy(&self.writes).into_owned()
    }

    fn fire_matching_triggers(&mut self) {
        loop {
            let Some((trigger, _)) = self.script.front() else { break };
            let needle: Vec<u8> = trigger.as_bytes().to_vec();
            let matched = if needle.is_empty() {
                Some(0usize)
            } else {
                let haystack = &self.writes[self.scan_pos..];
                haystack
                    .windows(needle.len())
                    .position(|w| w == needle.as_slice())
                    .map(|off| off + needle.len())
            };
            match matched {
                Some(advance) => {
                    let (_, reply) = self.script.pop_front().unwrap();
                    self.outgoing.extend(reply);
                    self.scan_pos += advance;
                }
                None => break,
            }
        }
    }
}

impl Read for MockPort {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.outgoing.is_empty() {
            return Ok(0);
        }
        let n = buf.len().min(self.outgoing.len());
        for slot in buf.iter_mut().take(n) {
            *slot = self.outgoing.pop_front().unwrap();
        }
        Ok(n)
    }
}

impl Write for MockPort {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.writes.extend_from_slice(data);
        self.fire_matching_triggers();
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ── detect_state ────────────────────────────────

#[test]
fn test_detect_state_returns_firmware_version() {
    let mut port = MockPort::new(vec![("version\r", b"Firmware version: 1902784 2024-12-01T12:34:56\r\n")]);
    let state = detect_state(&mut port).unwrap();
    assert!(matches!(state, HatState::Firmware(ref v) if v.starts_with("1902784")));
    // Sent the version command.
    assert!(port.writes_as_string().contains("version\r"));
}

#[test]
fn test_detect_state_returns_bootloader() {
    let mut port = MockPort::new(vec![("version\r", b"BuildHAT bootloader version 1\r\nBHBL> ")]);
    let state = detect_state(&mut port).unwrap();
    assert_eq!(state, HatState::Bootloader);
}

#[test]
fn test_detect_state_errors_with_no_response() {
    // No scripted replies — detect_state retries 5x, each with a 1s window.
    // Shorten by using a mock that just never responds.
    let mut port = MockPort::new(vec![]);
    let err = detect_state(&mut port).unwrap_err();
    assert!(err.contains("No Build HAT"), "got {:?}", err);
}

#[test]
fn test_detect_state_ignores_noise_before_version() {
    let mut port = MockPort::new(vec![(
        "version\r",
        b"garbage line\r\nanother line\r\nFirmware version: 42 x\r\n",
    )]);
    let state = detect_state(&mut port).unwrap();
    assert!(matches!(state, HatState::Firmware(_)));
}

// ── upload_firmware ─────────────────────────────

fn no_op_progress() -> ProgressFn {
    Box::new(|_| {})
}

#[test]
fn test_upload_firmware_sequence() {
    let firmware = vec![0xAAu8; 128];
    let signature = vec![0xBBu8; 64];

    // Replies keyed on prefixes that actually appear in the uploader's
    // write stream: after `clear\r` and after each `\x03\r` (ETX + CR),
    // the bootloader prints `BHBL>`. After `reboot\r` the firmware boots
    // and prints a version line.
    let mut port = MockPort::new(vec![
        ("clear\r", b"BHBL> "),
        ("\x03\r", b"BHBL> "),           // after firmware payload
        ("signature ", b""),              // no reply, just advance queue
        ("\x03\r", b"BHBL> "),           // after signature payload
        ("reboot\r", b"Firmware version: 1902784 2024-12-01\r\n"),
    ]);

    upload_firmware(&mut port, &firmware, &signature, &no_op_progress()).unwrap();

    let out = port.writes_as_string();
    // Sent the control commands in order.
    let clear_pos = out.find("clear\r").expect("clear sent");
    let load_pos = out.find("load ").expect("load sent");
    let sig_pos = out.find("signature ").expect("signature sent");
    let reboot_pos = out.find("reboot\r").expect("reboot sent");
    assert!(clear_pos < load_pos);
    assert!(load_pos < sig_pos);
    assert!(sig_pos < reboot_pos);

    // Firmware bytes were framed by STX (0x02) and ETX (0x03).
    assert!(
        port.writes.windows(1).any(|w| w == [STX]),
        "STX byte missing from write stream"
    );
    assert!(
        port.writes.windows(2).any(|w| w == [ETX, b'\r']),
        "ETX + CR frame terminator missing"
    );

    // Both payloads made it into the wire. Firmware body is a run of 0xAA;
    // signature body is a run of 0xBB. Neither should be corrupted.
    assert!(
        port.writes.windows(firmware.len()).any(|w| w == firmware.as_slice()),
        "firmware payload missing from write stream"
    );
    assert!(
        port.writes.windows(signature.len()).any(|w| w == signature.as_slice()),
        "signature payload missing from write stream"
    );
}

#[test]
fn test_upload_firmware_load_cmd_carries_size_and_checksum() {
    let firmware = vec![0x11u8; 200];
    let signature = vec![0x22u8; 32];
    let mut port = MockPort::new(vec![
        ("clear\r", b"BHBL> "),
        ("\x03\r", b"BHBL> "),
        ("\x03\r", b"BHBL> "),
        ("reboot\r", b"Firmware version: 1\r\n"),
    ]);

    upload_firmware(&mut port, &firmware, &signature, &no_op_progress()).unwrap();

    let out = port.writes_as_string();
    let expected = format!("load {} {}", firmware.len(), firmware_checksum(&firmware));
    assert!(
        out.contains(&expected),
        "expected load command {:?} in {:?}",
        expected,
        out.lines().find(|l| l.contains("load")).unwrap_or(""),
    );

    let sig_expected = format!("signature {}", signature.len());
    assert!(out.contains(&sig_expected));
}

#[test]
fn test_upload_firmware_times_out_without_prompt() {
    // No scripted response to `clear\r`: wait_for_prompt should time out.
    // wait_for_prompt's deadline is 10s — we shorten the test by pre-filling
    // a junk response so the loop returns quickly with "prompt not found".
    // Still, this assertion primarily ensures we surface a timeout error
    // rather than blocking indefinitely or panicking.
    let firmware = vec![0u8; 16];
    let signature = vec![0u8; 8];
    // This test is slow by design; gate on CI/explicit request so the
    // common `cargo test` stays fast.
    if std::env::var("BRICKLOGO_SLOW_TESTS").is_err() {
        return;
    }
    let mut port = MockPort::new(vec![]);
    let err = upload_firmware(&mut port, &firmware, &signature, &no_op_progress()).unwrap_err();
    assert!(err.contains("Timed out"), "got {:?}", err);
}

#[test]
fn test_upload_firmware_still_ok_when_version_line_missed() {
    // If the bootloader sends `BHBL>` for every prompt except we never see
    // a post-reboot "Firmware version:" string, upload_firmware should
    // still return Ok (the caller re-runs detect_state).
    let firmware = vec![0u8; 16];
    let signature = vec![0u8; 8];
    let mut port = MockPort::new(vec![
        ("clear\r", b"BHBL> "),
        ("\x03\r", b"BHBL> "),
        ("\x03\r", b"BHBL> "),
        // After reboot, the bootloader replies with nothing the matcher
        // recognises, so the post-boot wait silently exhausts its deadline.
        // This test is slow by design (10s boot wait); gate on env.
    ]);
    if std::env::var("BRICKLOGO_SLOW_TESTS").is_err() {
        return;
    }
    upload_firmware(&mut port, &firmware, &signature, &no_op_progress()).unwrap();
}

#[test]
fn test_upload_firmware_progress_phases() {
    let firmware = vec![0xAAu8; 32];
    let signature = vec![0xBBu8; 16];
    let mut port = MockPort::new(vec![
        ("clear\r", b"BHBL> "),
        ("\x03\r", b"BHBL> "),
        ("\x03\r", b"BHBL> "),
        ("reboot\r", b"Firmware version: 1\r\n"),
    ]);
    let phases = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let phases_inner = phases.clone();
    let progress: ProgressFn = Box::new(move |p| {
        phases_inner.lock().unwrap().push(p.to_string());
    });
    upload_firmware(&mut port, &firmware, &signature, &progress).unwrap();

    let observed = phases.lock().unwrap().clone();
    // Expect at least the 4 main phase labels in order.
    let joined = observed.join("|");
    assert!(joined.contains("Clearing"));
    let clearing = observed.iter().position(|p| p.contains("Clearing")).unwrap();
    let firmware_up = observed.iter().position(|p| p.contains("Uploading firmware")).unwrap();
    let sig_up = observed.iter().position(|p| p.contains("Uploading signature")).unwrap();
    let reboot = observed.iter().position(|p| p.contains("Rebooting")).unwrap();
    assert!(clearing < firmware_up);
    assert!(firmware_up < sig_up);
    assert!(sig_up < reboot);
}
