//! End-to-end SIGINT test for script mode.
//!
//! The `script::run` unit tests pin the `exit_code_for` policy in
//! isolation, but they don't exercise the wiring that actually hooks
//! SIGINT to that flag — `ctrlc::set_handler` install, signal delivery,
//! and the `std::process::exit` call at the end of `run`. This test
//! spawns a real `bricklogo` subprocess, sends SIGINT to it while a
//! Logo `forever` loop is running, and asserts exit code 130.
//!
//! Unix-only because SIGINT semantics on Windows (via
//! `GenerateConsoleCtrlEvent`) are enough of a separate story that a
//! parallel test isn't worth maintaining here.

#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn script_sigint_exits_130() {
    // Write a script that prints a sentinel then hangs, so the test can
    // wait until the handler is installed and the loop has started
    // before sending the signal.
    let dir = std::env::temp_dir().join(format!("bricklogo-sigint-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("loop.logo");
    std::fs::write(&script, "print \"ready\nforever [wait 1]\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_bricklogo");
    let mut child = Command::new(bin)
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn bricklogo");

    // Wait for the script to print "ready" before sending SIGINT. That
    // guarantees the `ctrlc` handler is installed and evaluate() has
    // entered the forever loop.
    {
        let stdout = child.stdout.as_mut().expect("piped stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("failed to read stdout line");
        assert!(line.trim() == "ready", "unexpected first line: {:?}", line);
    }

    // Send SIGINT.
    let pid = child.id() as i32;
    let rc = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(rc, 0, "kill(SIGINT) failed");

    // Wait (with timeout) for the process to exit.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                panic!("bricklogo did not exit within 5s of SIGINT");
            }
            Err(e) => panic!("try_wait failed: {}", e),
        }
    };

    let code = exit_status.code();
    // Cleanup.
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        code,
        Some(130),
        "expected exit code 130 after SIGINT, got {:?} (signal={:?})",
        code,
        std::os::unix::process::ExitStatusExt::signal(&exit_status)
    );
}
