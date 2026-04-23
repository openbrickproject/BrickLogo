use super::*;
use bricklogo_net::{DEFAULT_PORT, NetRole};

// ── Bare invocation ─────────────────────────────

#[test]
fn test_no_args_is_repl_mode() {
    let cli = parse_cli_from(&[]).unwrap();
    assert!(cli.script.is_none());
    assert!(cli.net.role.is_none());
    assert!(cli.net.password.is_none());
    assert!(cli.firmware.is_none());
}

// ── Script mode ─────────────────────────────────

#[test]
fn test_positional_path_is_script_file() {
    let cli = parse_cli_from(&["path/to/program.logo"]).unwrap();
    match cli.script {
        Some(ScriptSource::File(ref p)) => assert_eq!(p, &PathBuf::from("path/to/program.logo")),
        other => panic!("expected ScriptSource::File, got {:?}", other.is_some()),
    }
}

#[test]
fn test_bare_dash_is_stdin_script() {
    let cli = parse_cli_from(&["-"]).unwrap();
    assert!(matches!(cli.script, Some(ScriptSource::Stdin)));
}

#[test]
fn test_second_positional_is_rejected() {
    // Once a script path is set, a second positional should fall through
    // to the "Unknown argument" error rather than silently overwriting.
    let err = parse_cli_from(&["first.logo", "second.logo"]).unwrap_err();
    assert!(err.contains("Unknown argument"));
}

// ── Host / join / password ──────────────────────

#[test]
fn test_host_alone_uses_default_port() {
    let cli = parse_cli_from(&["--host"]).unwrap();
    assert!(matches!(cli.net.role, Some(NetRole::Host(p)) if p == DEFAULT_PORT));
}

#[test]
fn test_host_with_port_parses_number() {
    let cli = parse_cli_from(&["--host", "5000"]).unwrap();
    assert!(matches!(cli.net.role, Some(NetRole::Host(5000))));
}

#[test]
fn test_host_with_non_numeric_next_arg_is_default_port_plus_positional() {
    // `bricklogo --host script.logo` — the "script.logo" is a path, not a
    // port number. Host defaults, and the positional becomes the script.
    let cli = parse_cli_from(&["--host", "script.logo"]).unwrap();
    assert!(matches!(cli.net.role, Some(NetRole::Host(p)) if p == DEFAULT_PORT));
    assert!(matches!(cli.script, Some(ScriptSource::File(_))));
}

#[test]
fn test_join_bare_address_adds_default_port() {
    let cli = parse_cli_from(&["--join", "192.168.1.50"]).unwrap();
    match cli.net.role {
        Some(NetRole::Client(ref addr)) => {
            assert_eq!(addr, &format!("192.168.1.50:{}", DEFAULT_PORT));
        }
        _ => panic!("expected NetRole::Client"),
    }
}

#[test]
fn test_join_address_with_port_preserved() {
    let cli = parse_cli_from(&["--join", "192.168.1.50:6000"]).unwrap();
    match cli.net.role {
        Some(NetRole::Client(ref addr)) => assert_eq!(addr, "192.168.1.50:6000"),
        _ => panic!("expected NetRole::Client"),
    }
}

#[test]
fn test_join_without_address_errors() {
    let err = parse_cli_from(&["--join"]).unwrap_err();
    assert!(err.contains("--join"));
}

#[test]
fn test_password_is_captured() {
    let cli = parse_cli_from(&["--host", "--password", "hunter2"]).unwrap();
    assert_eq!(cli.net.password.as_deref(), Some("hunter2"));
}

#[test]
fn test_password_without_value_errors() {
    let err = parse_cli_from(&["--password"]).unwrap_err();
    assert!(err.contains("--password"));
}

// ── Firmware mode ───────────────────────────────

#[test]
fn test_firmware_rcx_parses() {
    let cli = parse_cli_from(&["--firmware", "rcx"]).unwrap();
    let fw = cli.firmware.expect("firmware args");
    assert_eq!(fw.device, FirmwareDevice::Rcx);
    assert!(fw.image.is_none());
}

#[test]
fn test_firmware_spike_parses() {
    let cli = parse_cli_from(&["--firmware", "spike"]).unwrap();
    assert_eq!(cli.firmware.unwrap().device, FirmwareDevice::Spike);
}

#[test]
fn test_firmware_buildhat_parses() {
    let cli = parse_cli_from(&["--firmware", "buildhat"]).unwrap();
    assert_eq!(cli.firmware.unwrap().device, FirmwareDevice::BuildHat);
}

#[test]
fn test_firmware_image_override() {
    let cli = parse_cli_from(&["--firmware", "rcx", "--image", "/tmp/custom.srec"]).unwrap();
    let fw = cli.firmware.unwrap();
    assert_eq!(fw.image.as_deref(), Some(std::path::Path::new("/tmp/custom.srec")));
}

#[test]
fn test_firmware_unknown_device_errors() {
    let err = parse_cli_from(&["--firmware", "mindstorms"]).unwrap_err();
    assert!(err.contains("'rcx', 'spike', or 'buildhat'"));
}

#[test]
fn test_firmware_without_value_errors() {
    let err = parse_cli_from(&["--firmware"]).unwrap_err();
    assert!(err.contains("--firmware"));
}

#[test]
fn test_image_without_value_errors() {
    let err = parse_cli_from(&["--image"]).unwrap_err();
    assert!(err.contains("--image"));
}

#[test]
fn test_image_without_firmware_errors() {
    let err = parse_cli_from(&["--image", "/tmp/fw.bin"]).unwrap_err();
    assert!(err.contains("--image requires --firmware"));
}

// ── Firmware mutex ──────────────────────────────

#[test]
fn test_firmware_with_host_is_rejected() {
    let err = parse_cli_from(&["--firmware", "rcx", "--host"]).unwrap_err();
    assert!(err.contains("cannot be combined"));
}

#[test]
fn test_firmware_with_join_is_rejected() {
    let err = parse_cli_from(&["--firmware", "spike", "--join", "1.2.3.4"]).unwrap_err();
    assert!(err.contains("cannot be combined"));
}

#[test]
fn test_firmware_with_password_is_rejected() {
    let err = parse_cli_from(&["--firmware", "rcx", "--password", "x"]).unwrap_err();
    assert!(err.contains("cannot be combined"));
}

#[test]
fn test_firmware_with_script_path_is_rejected() {
    let err = parse_cli_from(&["--firmware", "rcx", "script.logo"]).unwrap_err();
    assert!(err.contains("cannot be combined"));
}

// ── Flag ordering / shebang compat ──────────────

#[test]
fn test_flags_after_script_path_still_parse() {
    // Shebang `#!/usr/bin/env bricklogo --host` gives argv
    // ["script.logo", "--host"] on Linux and ["--host", "script.logo"]
    // on macOS — both must work.
    let macos = parse_cli_from(&["--host", "script.logo"]).unwrap();
    assert!(matches!(macos.script, Some(ScriptSource::File(_))));
    assert!(macos.net.role.is_some());

    let linux = parse_cli_from(&["script.logo", "--host"]).unwrap();
    assert!(matches!(linux.script, Some(ScriptSource::File(_))));
    assert!(linux.net.role.is_some());
}

#[test]
fn test_unknown_flag_errors() {
    let err = parse_cli_from(&["--verbose"]).unwrap_err();
    assert!(err.contains("--verbose"));
}

// ── Combinations ────────────────────────────────

#[test]
fn test_host_script_password_all_set() {
    let cli =
        parse_cli_from(&["--host", "5000", "--password", "s3cr3t", "script.logo"]).unwrap();
    assert!(matches!(cli.net.role, Some(NetRole::Host(5000))));
    assert_eq!(cli.net.password.as_deref(), Some("s3cr3t"));
    assert!(matches!(cli.script, Some(ScriptSource::File(_))));
    assert!(cli.firmware.is_none());
}

#[test]
fn test_join_script_reads_both() {
    let cli = parse_cli_from(&["--join", "host.local", "-"]).unwrap();
    assert!(matches!(cli.net.role, Some(NetRole::Client(_))));
    assert!(matches!(cli.script, Some(ScriptSource::Stdin)));
}
