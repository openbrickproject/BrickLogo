use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, mpsc};

use bricklogo_hal::port_manager::PortManager;
use bricklogo_net::NetRole;
use bricklogo_tui::runtime::build_evaluator;

use crate::cli::NetArgs;

#[derive(Debug)]
pub enum ScriptSource {
    File(PathBuf),
    Stdin,
}

/// RAII guard that disconnects every adapter when dropped. Ensures motors
/// stop and BLE peripherals release on every script-exit path.
struct DisconnectGuard(Arc<Mutex<PortManager>>);

impl Drop for DisconnectGuard {
    fn drop(&mut self) {
        if let Ok(mut pm) = self.0.lock() {
            pm.remove_all();
        }
    }
}

/// Strip a UTF-8 BOM and a leading `#!` shebang line from a script source.
fn strip_shebang_and_bom(s: &str) -> String {
    let s = s.strip_prefix('\u{FEFF}').unwrap_or(s);
    if s.starts_with("#!") {
        match s.find('\n') {
            Some(i) => s[i + 1..].to_string(),
            None => String::new(),
        }
    } else {
        s.to_string()
    }
}

pub fn run(source: ScriptSource, net_args: NetArgs) -> Result<(), Box<dyn std::error::Error>> {
    // ── Resolve file path (CWD first, then bundled examples/) ──
    let source = match source {
        ScriptSource::File(path) => {
            let name = path.to_string_lossy().into_owned();
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let resolved = bricklogo_lang::paths::resolve_bundled(&name, &cwd, "examples");
            ScriptSource::File(resolved)
        }
        other => other,
    };

    // ── Read source ───────────────────────────────
    let source_text = match &source {
        ScriptSource::File(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read script {}: {}", path.display(), e))?,
        ScriptSource::Stdin => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("Cannot read stdin: {}", e))?;
            buf
        }
    };
    let cleaned = strip_shebang_and_bom(&source_text);

    // ── Wire callbacks ────────────────────────────
    // print/show/type → stdout (flushed so piping works).
    // System messages and errors → stderr.
    //
    // Each output_callback invocation corresponds to one logical line of output
    // (matching how the TUI renders callbacks as separate OutputLine entries),
    // so append a newline per call.
    let output_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|text: &str| {
        let mut out = io::stdout().lock();
        let _ = out.write_all(text.as_bytes());
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    });
    let system_fn: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|text: &str| {
        let mut err = io::stderr().lock();
        let _ = writeln!(err, "{}", text);
    });

    let (mut evaluator, port_manager) = build_evaluator(output_fn, system_fn.clone());

    // ── Resolve `load` relative to the script's directory ──
    if let ScriptSource::File(path) = &source {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                evaluator.set_disk_path(dir.to_path_buf());
            }
        }
    }

    // ── Optional networking ───────────────────────
    if let Some(role) = net_args.role {
        let (tx, rx) = mpsc::channel();
        evaluator.set_var_broadcast(tx);
        let global_vars = evaluator.global_vars_ref();
        let status = match &role {
            NetRole::Host(_) => Arc::new(Mutex::new("hosting (0 clients)".to_string())),
            NetRole::Client(_) => Arc::new(Mutex::new("connected".to_string())),
        };
        bricklogo_net::start_network(
            role,
            global_vars,
            rx,
            system_fn.clone(),
            status,
            net_args.password,
        )?;
    }

    // ── SIGINT: flip the evaluator's stop flag ────
    {
        let stop_flag = evaluator.stop_flag();
        let _ = ctrlc::set_handler(move || {
            stop_flag.store(true, Ordering::SeqCst);
        });
    }

    // ── Run ───────────────────────────────────────
    // The guard runs `remove_all` on every exit path (including panic).
    let _guard = DisconnectGuard(port_manager.clone());
    let interrupted_flag = evaluator.stop_flag();

    let exit_code = match evaluator.evaluate(&cleaned) {
        Ok(_) => {
            if interrupted_flag.load(Ordering::SeqCst) {
                130 // SIGINT convention
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            if interrupted_flag.load(Ordering::SeqCst) {
                130
            } else {
                1
            }
        }
    };

    // Drop the guard explicitly so disconnect happens before std::process::exit
    // bypasses the rest of stack unwinding.
    drop(_guard);
    std::process::exit(exit_code);
}

#[cfg(test)]
#[path = "tests/script.rs"]
mod tests;
