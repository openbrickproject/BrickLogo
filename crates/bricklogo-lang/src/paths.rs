//! Path resolution for user-named files (scripts, firmware).
//!
//! BrickLogo is typically installed to `~/.bricklogo/` with `examples/` and
//! `firmware/` subdirectories, and run from anywhere on the user's PATH.
//! When the user writes `load "hosttest` or `firmware "rcx "firm0332.srec`,
//! the name could refer to a file in their current working directory *or* a
//! bundled file next to the binary. This module resolves both cases with a
//! consistent search order.

use std::path::{Path, PathBuf};

/// Resolve a user-supplied file name using the standard search order.
///
/// Search order:
/// 1. Absolute paths are used as-is.
/// 2. Paths with an explicit `./` or `../` prefix resolve against `base` only
///    (no bundled fallback — the user asked for a specific location).
/// 3. Otherwise, try `<base>/<name>` first; if it doesn't exist, fall back to
///    `<exe_dir>/<subdir>/<name>`.
///
/// Returns the first existing candidate, or `<base>/<name>` if nothing was
/// found (so callers produce a sensible "not found" error message).
pub fn resolve_bundled(name: &str, base: &Path, subdir: &str) -> PathBuf {
    let path = Path::new(name);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if name.starts_with("./") || name.starts_with("../") {
        return base.join(name);
    }
    let base_candidate = base.join(name);
    if base_candidate.exists() {
        return base_candidate;
    }
    if let Some(bundled) = bundled_dir(subdir) {
        let bundled_candidate = bundled.join(name);
        if bundled_candidate.exists() {
            return bundled_candidate;
        }
    }
    base_candidate
}

/// Return `<exe_dir>/<subdir>` if the current executable's location can be
/// determined. Follows symlinks so the install-script's `~/.local/bin/bricklogo`
/// symlink resolves back to `~/.bricklogo/`.
pub fn bundled_dir(subdir: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let exe_dir = exe.parent()?;
    Some(exe_dir.join(subdir))
}
