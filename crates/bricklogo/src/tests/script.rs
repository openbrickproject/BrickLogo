use super::strip_shebang_and_bom;

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
