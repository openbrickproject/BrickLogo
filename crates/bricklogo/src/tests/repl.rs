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
