use std::path::PathBuf;

use bricklogo_net::{DEFAULT_PORT, NetRole};

use crate::script::ScriptSource;

#[derive(Debug)]
pub struct NetArgs {
    pub role: Option<NetRole>,
    pub password: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirmwareDevice {
    Rcx,
    Spike,
    BuildHat,
}

#[derive(Debug)]
pub struct FirmwareArgs {
    pub device: FirmwareDevice,
    /// Optional override. If `None`, the CLI uses the bundled default for
    /// the device.
    pub image: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Cli {
    pub script: Option<ScriptSource>,
    pub net: NetArgs,
    pub firmware: Option<FirmwareArgs>,
}

/// Parse argv into either a script-mode or REPL-mode invocation.
///
/// Recognized:
///   --host [port]    host networking (REPL or script)
///   --join addr      join host (REPL or script)
///   --password p     plaintext password
///   <path>           run path as a script
///   -                run script from stdin
///
/// The first positional that is not a flag value is treated as the script
/// path. Flag order is flexible so shebangs (`#!/usr/bin/env bricklogo`)
/// work regardless of where the kernel inserts the script path.
pub fn parse_cli_args() -> Cli {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    match parse_cli_from(&args_ref) {
        Ok(cli) => cli,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    }
}

/// Testable core: same logic as [`parse_cli_args`] but returns errors
/// instead of exiting the process.
pub fn parse_cli_from(args: &[&str]) -> Result<Cli, String> {
    let mut role: Option<NetRole> = None;
    let mut password: Option<String> = None;
    let mut script: Option<ScriptSource> = None;
    let mut firmware_device: Option<FirmwareDevice> = None;
    let mut firmware_image: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--firmware" => {
                if i + 1 >= args.len() {
                    return Err(
                        "--firmware requires a device name (rcx or spike)".to_string(),
                    );
                }
                firmware_device = Some(match args[i + 1] {
                    "rcx" => FirmwareDevice::Rcx,
                    "spike" => FirmwareDevice::Spike,
                    "buildhat" => FirmwareDevice::BuildHat,
                    other => {
                        return Err(format!(
                            "--firmware expects 'rcx', 'spike', or 'buildhat', got {:?}",
                            other
                        ));
                    }
                });
                i += 2;
            }
            "--image" => {
                if i + 1 >= args.len() {
                    return Err("--image requires a path".to_string());
                }
                firmware_image = Some(PathBuf::from(args[i + 1]));
                i += 2;
            }
            "--host" => {
                if let Some(port) = args.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                    role = Some(NetRole::Host(port));
                    i += 2;
                } else {
                    role = Some(NetRole::Host(DEFAULT_PORT));
                    i += 1;
                }
            }
            "--join" => {
                if i + 1 >= args.len() {
                    return Err("--join requires an address".to_string());
                }
                let addr = args[i + 1];
                let full = if addr.contains(':') {
                    addr.to_string()
                } else {
                    format!("{}:{}", addr, DEFAULT_PORT)
                };
                role = Some(NetRole::Client(full));
                i += 2;
            }
            "--password" => {
                if i + 1 >= args.len() {
                    return Err("--password requires a value".to_string());
                }
                password = Some(args[i + 1].to_string());
                i += 2;
            }
            "-" if script.is_none() => {
                script = Some(ScriptSource::Stdin);
                i += 1;
            }
            other if !other.starts_with('-') && script.is_none() => {
                script = Some(ScriptSource::File(PathBuf::from(other)));
                i += 1;
            }
            other => {
                return Err(format!("Unknown argument: {}", other));
            }
        }
    }

    // `--firmware` is a one-shot mode: reject mixing it with script /
    // network flags.
    let firmware = if let Some(device) = firmware_device {
        if script.is_some() || role.is_some() || password.is_some() {
            return Err(
                "--firmware cannot be combined with --host/--join/--password or a script path"
                    .to_string(),
            );
        }
        Some(FirmwareArgs { device, image: firmware_image })
    } else {
        if firmware_image.is_some() {
            return Err("--image requires --firmware".to_string());
        }
        None
    };

    Ok(Cli {
        script,
        net: NetArgs { role, password },
        firmware,
    })
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
