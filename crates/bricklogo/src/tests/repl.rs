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

#[test]
fn test_terminal_lifecycle_restore_is_idempotent() {
    // Calling restore twice should be harmless — the second call only
    // emits show_cursor because the other flags were cleared by the first.
    let mut lifecycle = TerminalLifecycle::default();
    lifecycle.mark_raw_mode_enabled();
    lifecycle.mark_alt_screen_entered();
    let mut restorer = FakeRestorer::default();

    lifecycle.restore(&mut restorer).unwrap();
    let first_calls = restorer.calls.clone();
    restorer.calls.clear();

    lifecycle.restore(&mut restorer).unwrap();
    assert_eq!(first_calls, vec!["disable_raw_mode", "leave_alt_screen", "show_cursor"]);
    assert_eq!(restorer.calls, vec!["show_cursor"]);
}

#[test]
fn test_terminal_lifecycle_restore_propagates_raw_mode_error() {
    // If disable_raw_mode fails, restore should return the error without
    // swallowing it. The state flag is still cleared optimistically — the
    // contract is "we tried", not "we succeeded".
    let mut lifecycle = TerminalLifecycle::default();
    lifecycle.mark_raw_mode_enabled();
    lifecycle.mark_alt_screen_entered();
    let mut restorer = FakeRestorer {
        fail_on: Some("disable_raw_mode"),
        ..Default::default()
    };

    let err = lifecycle.restore(&mut restorer).unwrap_err();
    assert!(format!("{}", err).contains("fail"));
}

#[test]
fn test_terminal_lifecycle_restore_propagates_alt_screen_error() {
    let mut lifecycle = TerminalLifecycle::default();
    lifecycle.mark_raw_mode_enabled();
    lifecycle.mark_alt_screen_entered();
    let mut restorer = FakeRestorer {
        fail_on: Some("leave_alt_screen"),
        ..Default::default()
    };

    let err = lifecycle.restore(&mut restorer).unwrap_err();
    assert!(format!("{}", err).contains("fail"));
    // disable_raw_mode ran before the failure.
    assert!(restorer.calls.contains(&"disable_raw_mode"));
}

#[test]
fn test_terminal_lifecycle_default_is_clean_slate() {
    let lifecycle = TerminalLifecycle::default();
    assert!(!lifecycle.raw_mode_enabled);
    assert!(!lifecycle.alt_screen_entered);
}

#[test]
fn test_terminal_lifecycle_mark_methods_set_flags() {
    let mut lifecycle = TerminalLifecycle::default();
    lifecycle.mark_raw_mode_enabled();
    assert!(lifecycle.raw_mode_enabled);
    assert!(!lifecycle.alt_screen_entered);
    lifecycle.mark_alt_screen_entered();
    assert!(lifecycle.raw_mode_enabled);
    assert!(lifecycle.alt_screen_entered);
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

// ── Double-Esc: clear current line ──────────────

use bricklogo_tui::app::App;
use std::time::{Duration, Instant};

fn make_app_with_input(s: &str) -> App {
    let mut app = App::new(None, "0.0.0", None).unwrap();
    app.input = s.to_string();
    app.cursor_position = s.len();
    app
}

#[test]
fn test_escape_on_empty_input_is_noop() {
    let mut app = make_app_with_input("");
    let mut esc_at: Option<Instant> = None;
    handle_escape_press(&mut app, &mut esc_at, Instant::now());
    assert!(esc_at.is_none(), "Esc on empty input must not arm the window");
    assert!(!app.esc_message, "no flash message on empty input");
}

#[test]
fn test_first_escape_with_input_arms_window() {
    let mut app = make_app_with_input("rotate 90");
    let mut esc_at: Option<Instant> = None;
    let now = Instant::now();
    handle_escape_press(&mut app, &mut esc_at, now);
    assert!(esc_at.is_some(), "first Esc should arm the window");
    assert!(app.esc_message);
    // Input is untouched on the first press.
    assert_eq!(app.input, "rotate 90");
    assert_eq!(app.cursor_position, 9);
}

#[test]
fn test_second_escape_within_window_clears_input() {
    let mut app = make_app_with_input("rotate 90");
    let mut esc_at = Some(Instant::now());
    app.esc_message = true;
    handle_escape_press(&mut app, &mut esc_at, Instant::now());
    assert!(app.input.is_empty(), "second Esc should clear the input");
    assert_eq!(app.cursor_position, 0);
    assert!(esc_at.is_none(), "window should close after clearing");
    assert!(!app.esc_message);
}

#[test]
fn test_escape_while_busy_requests_stop_and_clears_window() {
    let mut app = make_app_with_input("forever");
    app.busy = true;
    // Pretend the user had armed Esc before going busy.
    let mut esc_at = Some(Instant::now());
    app.esc_message = true;
    handle_escape_press(&mut app, &mut esc_at, Instant::now());
    // Input must not be cleared — busy mode means stop-the-running-program.
    assert_eq!(app.input, "forever");
    // Armed window is reset so double-Esc doesn't accidentally clear after stop.
    assert!(esc_at.is_none());
    assert!(!app.esc_message);
    // Stop flag propagates via App::request_stop — we can't observe that
    // without reaching into private state, but the priority is correct if
    // no other side effect fired (input unchanged).
}

#[test]
fn test_escape_in_multiline_cancels_definition_and_clears_window() {
    let mut app = make_app_with_input("");
    // Start a definition so multi_line is Some.
    app.input = "to greet".to_string();
    app.submit_input();
    assert!(app.multi_line.is_some());
    app.input = "partial".to_string();
    app.cursor_position = app.input.len();

    let mut esc_at: Option<Instant> = None;
    handle_escape_press(&mut app, &mut esc_at, Instant::now());

    assert!(app.multi_line.is_none(), "Esc should cancel the definition");
    // Input line itself is left as-is — single Esc in multi-line bails
    // out of the definition, not the line buffer.
    assert_eq!(app.input, "partial");
    assert!(esc_at.is_none());
    assert!(!app.esc_message);
}

#[test]
fn test_expire_esc_window_clears_stale_arm() {
    let mut app = make_app_with_input("x");
    let mut esc_at = Some(Instant::now() - Duration::from_secs(2));
    app.esc_message = true;
    let changed = expire_esc_window(&mut app, &mut esc_at, Instant::now());
    assert!(changed, "expired window should report state change");
    assert!(esc_at.is_none());
    assert!(!app.esc_message);
}

#[test]
fn test_expire_esc_window_keeps_fresh_arm() {
    let mut app = make_app_with_input("x");
    let mut esc_at = Some(Instant::now());
    app.esc_message = true;
    let changed = expire_esc_window(&mut app, &mut esc_at, Instant::now());
    assert!(!changed);
    assert!(esc_at.is_some(), "recent arm must not be expired");
    assert!(app.esc_message);
}

#[test]
fn test_expire_esc_window_no_op_when_not_armed() {
    let mut app = make_app_with_input("x");
    let mut esc_at: Option<Instant> = None;
    let changed = expire_esc_window(&mut app, &mut esc_at, Instant::now());
    assert!(!changed);
}

#[test]
fn test_esc_outside_window_acts_as_first_press_again() {
    // Arm the window at t=0, simulate expiry at t=2s, then press Esc
    // again with no arming — should re-arm, not clear.
    let mut app = make_app_with_input("hello");
    let mut esc_at = Some(Instant::now() - Duration::from_secs(2));
    app.esc_message = true;
    let now = Instant::now();
    expire_esc_window(&mut app, &mut esc_at, now);
    assert!(esc_at.is_none());

    handle_escape_press(&mut app, &mut esc_at, now);
    assert!(esc_at.is_some(), "Esc after expiry should re-arm, not clear");
    assert_eq!(app.input, "hello");
}

// ── Tab completion — pure function ──────────────

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// `submit_input` dispatches evaluation to a background thread; wait for
/// it to finish so observable state (procedures, globals) has settled
/// before we query it from the tests.
fn run_until_idle(app: &mut App) {
    for _ in 0..50 {
        app.tick();
        if !app.busy {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    app.tick();
}

#[test]
fn test_complete_unique_primitive() {
    // Input `rota` with commands `["rotate", "rotateto"]` has two
    // matches sharing the prefix `rotate` — LCP extension fills that
    // in. Use a single candidate for a truly unique full completion.
    let commands = s(&["rotate"]);
    let (new_input, new_cursor) = complete_at_cursor("rota", 4, &commands, &[]).unwrap();
    assert_eq!(new_input, "rotate");
    assert_eq!(new_cursor, 6);
}

#[test]
fn test_complete_full_unique_match() {
    let commands = s(&["print"]);
    let (new_input, new_cursor) = complete_at_cursor("prin", 4, &commands, &[]).unwrap();
    assert_eq!(new_input, "print");
    assert_eq!(new_cursor, 5);
}

#[test]
fn test_complete_lcp_extension() {
    let commands = s(&["rotate", "rotateto", "rotatetoabs", "rotateby"]);
    let (new_input, _) = complete_at_cursor("rot", 3, &commands, &[]).unwrap();
    assert_eq!(new_input, "rotate");
}

#[test]
fn test_complete_zero_matches() {
    let commands = s(&["rotate"]);
    assert!(complete_at_cursor("xyz", 3, &commands, &[]).is_none());
}

#[test]
fn test_complete_already_at_lcp() {
    // `rotate` is the shared prefix of all three — no further
    // extension possible without committing to one variant.
    let commands = s(&["rotate", "rotateto", "rotateby"]);
    assert!(complete_at_cursor("rotate", 6, &commands, &[]).is_none());
}

#[test]
fn test_complete_procedure_folds_into_commands() {
    // Two candidates whose shared prefix `g` is shorter than the
    // typed `gr` — nothing to extend.
    let commands = s(&["greet", "group"]);
    assert!(complete_at_cursor("gr", 2, &commands, &[]).is_none());
}

#[test]
fn test_complete_quote_token_is_noop() {
    let commands = s(&["science", "spike"]);
    assert!(complete_at_cursor("connectto \"s", 12, &commands, &[]).is_none());
}

#[test]
fn test_complete_colon_variable_unique() {
    let vars = s(&["count"]);
    let (new_input, new_cursor) = complete_at_cursor(":c", 2, &[], &vars).unwrap();
    assert_eq!(new_input, ":count");
    assert_eq!(new_cursor, 6);
}

#[test]
fn test_complete_colon_variable_lcp() {
    let vars = s(&["count", "counter"]);
    let (new_input, _) = complete_at_cursor(":c", 2, &[], &vars).unwrap();
    assert_eq!(new_input, ":count");
}

#[test]
fn test_complete_bare_colon_noop() {
    let vars = s(&["count"]);
    assert!(complete_at_cursor(":", 1, &[], &vars).is_none());
}

#[test]
fn test_complete_colon_case_insensitive_returns_canonical() {
    let vars = s(&["xcoord"]);
    let (new_input, _) = complete_at_cursor(":X", 2, &[], &vars).unwrap();
    assert_eq!(new_input, ":xcoord");
}

#[test]
fn test_complete_empty_token_at_start() {
    let commands = s(&["rotate"]);
    assert!(complete_at_cursor("", 0, &commands, &[]).is_none());
}

#[test]
fn test_complete_empty_token_after_whitespace() {
    let commands = s(&["rotate"]);
    assert!(complete_at_cursor("rotate ", 7, &commands, &[]).is_none());
}

#[test]
fn test_complete_token_in_middle_of_input() {
    let commands = s(&["rotate"]);
    // Cursor at end-of-input but mid-"statement": the token to complete
    // is the trailing `rot`. Extends to `rotate`.
    let (new_input, new_cursor) =
        complete_at_cursor("print rot", 9, &commands, &[]).unwrap();
    assert_eq!(new_input, "print rotate");
    assert_eq!(new_cursor, 12);
}

#[test]
fn test_complete_cursor_mid_token_noop() {
    // Cursor in the middle of `rota`, before `ta`. Next char isn't a
    // token boundary → no-op.
    let commands = s(&["rotate"]);
    assert!(complete_at_cursor("rota", 2, &commands, &[]).is_none());
}

#[test]
fn test_complete_bracket_boundary() {
    let commands = s(&["rotate"]);
    // `[` is a token boundary, so `rota` after it is its own token.
    let (new_input, _) = complete_at_cursor("[rota", 5, &commands, &[]).unwrap();
    assert_eq!(new_input, "[rotate");
}

#[test]
fn test_complete_plain_case_insensitive_returns_canonical() {
    let commands = s(&["rotate"]);
    let (new_input, _) = complete_at_cursor("ROT", 3, &commands, &[]).unwrap();
    assert_eq!(new_input, "rotate");
}

#[test]
fn test_complete_splices_before_trailing_bracket() {
    // Cursor at end of `rot` with `]` immediately after — still
    // treated as end-of-token.
    let commands = s(&["rotate"]);
    let (new_input, _) =
        complete_at_cursor("[rot]", 4, &commands, &[]).unwrap();
    assert_eq!(new_input, "[rotate]");
}

// ── Tab press — App integration ─────────────────

#[test]
fn test_tab_press_completes_unique_primitive() {
    // `onfo` shares a prefix with `onfor` and nothing else in the
    // registered primitive set, so Tab should fill it in fully.
    let mut app = make_app_with_input("onfo");
    handle_tab_press(&mut app);
    assert_eq!(app.input, "onfor");
    assert_eq!(app.cursor_position, 5);
}

#[test]
fn test_tab_press_when_busy_is_noop() {
    let mut app = make_app_with_input("rot");
    app.busy = true;
    handle_tab_press(&mut app);
    assert_eq!(app.input, "rot");
    assert_eq!(app.cursor_position, 3);
}

#[test]
fn test_tab_press_completes_procedure() {
    let mut app = make_app_with_input("");
    // Multi-line definitions need three submits: `to greet` opens
    // definition mode, the body is collected, `end` closes it.
    app.input = "to greet".to_string();
    app.submit_input();
    app.input = "print \"hi".to_string();
    app.submit_input();
    app.input = "end".to_string();
    app.submit_input();
    run_until_idle(&mut app);
    assert!(
        app.evaluator().unwrap().get_user_procedure("greet").is_some(),
        "procedure `greet` was not registered"
    );

    app.input = "gree".to_string();
    app.cursor_position = 4;
    handle_tab_press(&mut app);
    assert_eq!(app.input, "greet");
}

#[test]
fn test_tab_press_completes_variable() {
    let mut app = make_app_with_input("");
    app.input = "make \"counter 5".to_string();
    app.submit_input();
    run_until_idle(&mut app);

    app.input = ":coun".to_string();
    app.cursor_position = 5;
    handle_tab_press(&mut app);
    assert_eq!(app.input, ":counter");
}

#[test]
fn test_tab_press_no_match_is_silent() {
    let mut app = make_app_with_input("zzznomatch");
    handle_tab_press(&mut app);
    assert_eq!(app.input, "zzznomatch");
    assert_eq!(app.cursor_position, 10);
}

#[test]
fn test_tab_press_empty_input_is_noop() {
    let mut app = make_app_with_input("");
    handle_tab_press(&mut app);
    assert_eq!(app.input, "");
    assert_eq!(app.cursor_position, 0);
}
