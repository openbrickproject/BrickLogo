use super::{exit_code_for, strip_shebang_and_bom};

#[test]
fn strips_unix_shebang() {
    let src = "#!/usr/bin/env bricklogo\nprint \"hi\n";
    assert_eq!(strip_shebang_and_bom(src), "print \"hi\n");
}

#[test]
fn strips_bom() {
    let src = "\u{FEFF}print \"hi\n";
    assert_eq!(strip_shebang_and_bom(src), "print \"hi\n");
}

#[test]
fn strips_bom_then_shebang() {
    let src = "\u{FEFF}#!/usr/bin/env bricklogo\nprint \"hi\n";
    assert_eq!(strip_shebang_and_bom(src), "print \"hi\n");
}

#[test]
fn leaves_normal_source_untouched() {
    let src = "print \"hi\n";
    assert_eq!(strip_shebang_and_bom(src), "print \"hi\n");
}

#[test]
fn shebang_only_file_becomes_empty() {
    let src = "#!/usr/bin/env bricklogo";
    assert_eq!(strip_shebang_and_bom(src), "");
}

// ── Exit code policy ────────────────────────────

#[test]
fn test_exit_code_clean_finish_is_zero() {
    assert_eq!(exit_code_for(false, false), 0);
}

#[test]
fn test_exit_code_runtime_error_is_one() {
    assert_eq!(exit_code_for(true, false), 1);
}

#[test]
fn test_exit_code_sigint_trumps_clean_finish() {
    // Even if evaluate returned Ok (because top-level `evaluate`
    // catches LogoError::Stop), an observed SIGINT must still exit
    // 130 so callers can distinguish interruption from success.
    assert_eq!(exit_code_for(false, true), 130);
}

#[test]
fn test_exit_code_sigint_trumps_error() {
    assert_eq!(exit_code_for(true, true), 130);
}
