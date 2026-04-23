use std::path::PathBuf;

use bricklogo_net::{DEFAULT_PORT, NetRole};

use crate::script::ScriptSource;

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

pub struct FirmwareArgs {
    pub device: FirmwareDevice,
    /// Optional override. If `None`, the CLI uses the bundled default for
    /// the device.
    pub image: Option<PathBuf>,
}

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
    let mut role: Option<NetRole> = None;
    let mut password: Option<String> = None;
    let mut script: Option<ScriptSource> = None;
    let mut firmware_device: Option<FirmwareDevice> = None;
    let mut firmware_image: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--firmware" => {
                if i + 1 >= args.len() {
                    eprintln!("--firmware requires a device name (rcx or spike)");
                    std::process::exit(1);
                }
                firmware_device = Some(match args[i + 1].as_str() {
                    "rcx" => FirmwareDevice::Rcx,
                    "spike" => FirmwareDevice::Spike,
                    "buildhat" => FirmwareDevice::BuildHat,
                    other => {
                        eprintln!("--firmware expects 'rcx', 'spike', or 'buildhat', got {:?}", other);
                        std::process::exit(1);
                    }
                });
                i += 2;
            }
            "--image" => {
                if i + 1 >= args.len() {
                    eprintln!("--image requires a path");
                    std::process::exit(1);
                }
                firmware_image = Some(PathBuf::from(&args[i + 1]));
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
                    eprintln!("--join requires an address");
                    std::process::exit(1);
                }
                let addr = &args[i + 1];
                let full = if addr.contains(':') {
                    addr.clone()
                } else {
                    format!("{}:{}", addr, DEFAULT_PORT)
                };
                role = Some(NetRole::Client(full));
                i += 2;
            }
            "--password" => {
                if i + 1 >= args.len() {
                    eprintln!("--password requires a value");
                    std::process::exit(1);
                }
                password = Some(args[i + 1].clone());
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
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
    }

    // `--firmware` is a one-shot mode: reject mixing it with script / network flags.
    let firmware = if let Some(device) = firmware_device {
        if script.is_some() || role.is_some() || password.is_some() {
            eprintln!("--firmware cannot be combined with --host/--join/--password or a script path");
            std::process::exit(1);
        }
        Some(FirmwareArgs { device, image: firmware_image })
    } else {
        if firmware_image.is_some() {
            eprintln!("--image requires --firmware");
            std::process::exit(1);
        }
        None
    };

    Cli {
        script,
        net: NetArgs { role, password },
        firmware,
    }
}
