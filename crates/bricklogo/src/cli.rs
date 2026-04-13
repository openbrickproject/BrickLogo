use std::path::PathBuf;

use bricklogo_net::{DEFAULT_PORT, NetRole};

use crate::script::ScriptSource;

pub struct NetArgs {
    pub role: Option<NetRole>,
    pub password: Option<String>,
}

pub struct Cli {
    pub script: Option<ScriptSource>,
    pub net: NetArgs,
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

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--host" => {
                let port = args
                    .get(i + 1)
                    .and_then(|s| s.parse::<u16>().ok());
                if port.is_some() {
                    role = Some(NetRole::Host(port.unwrap()));
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

    Cli {
        script,
        net: NetArgs { role, password },
    }
}
