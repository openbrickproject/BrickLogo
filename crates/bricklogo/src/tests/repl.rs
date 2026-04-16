use super::*;

#[derive(Default)]
struct FakeRestorer {
    calls: Vec<&'static str>,
    fail_on: Option<&'static str>,
}

impl TerminalRestorer for FakeRestorer {
    fn disable_raw_mode(&mut self) -> io::Result<()> {
        self.calls.push("disable_raw_mode");
        if self.fail_on == Some("disable_raw_mode") {
            return Err(io::Error::other("fail"));
        }
        Ok(())
    }

    fn leave_alt_screen(&mut self) -> io::Result<()> {
        self.calls.push("leave_alt_screen");
        if self.fail_on == Some("leave_alt_screen") {
            return Err(io::Error::other("fail"));
        }
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.calls.push("show_cursor");
        if self.fail_on == Some("show_cursor") {
            return Err(io::Error::other("fail"));
        }
        Ok(())
    }
}

#[test]
fn test_terminal_lifecycle_restores_full_session() {
    let mut lifecycle = TerminalLifecycle::default();
    lifecycle.mark_raw_mode_enabled();
    lifecycle.mark_alt_screen_entered();
    let mut restorer = FakeRestorer::default();

    lifecycle.restore(&mut restorer).unwrap();

    assert_eq!(
        restorer.calls,
        vec!["disable_raw_mode", "leave_alt_screen", "show_cursor"]
    );
    assert!(!lifecycle.raw_mode_enabled);
    assert!(!lifecycle.alt_screen_entered);
}

#[test]
fn test_terminal_lifecycle_restores_partial_session() {
    let mut lifecycle = TerminalLifecycle::default();
    lifecycle.mark_raw_mode_enabled();
    let mut restorer = FakeRestorer::default();

    lifecycle.restore(&mut restorer).unwrap();

    assert_eq!(restorer.calls, vec!["disable_raw_mode", "show_cursor"]);
}

#[test]
fn test_terminal_lifecycle_show_cursor_runs_even_without_setup() {
    let mut lifecycle = TerminalLifecycle::default();
    let mut restorer = FakeRestorer::default();

    lifecycle.restore(&mut restorer).unwrap();

    assert_eq!(restorer.calls, vec!["show_cursor"]);
}

// ── Word boundary tests ─────────────────────────

#[test]
fn test_word_boundary_left_from_end() {
    assert_eq!(word_boundary_left("hello world", 11), 6);
}

#[test]
fn test_word_boundary_left_from_middle_of_word() {
    assert_eq!(word_boundary_left("hello world", 8), 6);
}

#[test]
fn test_word_boundary_left_from_word_start() {
    // From start of "world", jumps back over space to start of "hello"
    assert_eq!(word_boundary_left("hello world", 6), 0);
    assert_eq!(word_boundary_left("hello  world", 7), 0);
}

#[test]
fn test_word_boundary_left_at_start() {
    assert_eq!(word_boundary_left("hello", 0), 0);
}

#[test]
fn test_word_boundary_left_single_word() {
    assert_eq!(word_boundary_left("hello", 5), 0);
}

#[test]
fn test_word_boundary_left_multiple_words() {
    assert_eq!(word_boundary_left("one two three", 13), 8);
    assert_eq!(word_boundary_left("one two three", 8), 4);
    assert_eq!(word_boundary_left("one two three", 4), 0);
}

#[test]
fn test_word_boundary_right_from_start() {
    assert_eq!(word_boundary_right("hello world", 0), 6);
}

#[test]
fn test_word_boundary_right_from_middle_of_word() {
    assert_eq!(word_boundary_right("hello world", 2), 6);
}

#[test]
fn test_word_boundary_right_from_space() {
    assert_eq!(word_boundary_right("hello world", 5), 6);
}

#[test]
fn test_word_boundary_right_at_end() {
    assert_eq!(word_boundary_right("hello", 5), 5);
}

#[test]
fn test_word_boundary_right_single_word() {
    assert_eq!(word_boundary_right("hello", 0), 5);
}

#[test]
fn test_word_boundary_right_multiple_words() {
    assert_eq!(word_boundary_right("one two three", 0), 4);
    assert_eq!(word_boundary_right("one two three", 4), 8);
    assert_eq!(word_boundary_right("one two three", 8), 13);
}

#[test]
fn test_word_boundary_empty_string() {
    assert_eq!(word_boundary_left("", 0), 0);
    assert_eq!(word_boundary_right("", 0), 0);
}

#[test]
fn test_word_boundary_multiple_spaces() {
    assert_eq!(word_boundary_left("hello   world", 13), 8);
    assert_eq!(word_boundary_right("hello   world", 0), 8);
}
