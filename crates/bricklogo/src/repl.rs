use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bricklogo_tui::app::App;
use bricklogo_tui::ui;

use crate::cli::NetArgs;

#[derive(Default)]
pub(crate) struct TerminalLifecycle {
    pub(crate) raw_mode_enabled: bool,
    pub(crate) alt_screen_entered: bool,
}

pub(crate) trait TerminalRestorer {
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn leave_alt_screen(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
}

/// One Esc keypress. Priority:
///
/// 1. Running a program → stop it and clear any pending Esc window.
/// 2. Inside a multi-line definition → cancel it and clear the window.
/// 3. Armed window is still open → clear the current input line.
/// 4. Non-empty input and no armed window → arm the window and flash
///    "Press Esc again to clear line".
/// 5. Empty input and no armed window → no-op.
pub(crate) fn handle_escape_press(
    app: &mut App,
    esc_at: &mut Option<Instant>,
    now: Instant,
) {
    if app.busy {
        app.request_stop();
        *esc_at = None;
        app.esc_message = false;
    } else if app.multi_line.is_some() {
        app.cancel_definition();
        *esc_at = None;
        app.esc_message = false;
    } else if esc_at.is_some() {
        app.input.clear();
        app.cursor_position = 0;
        *esc_at = None;
        app.esc_message = false;
    } else if !app.input.is_empty() {
        *esc_at = Some(now);
        app.esc_message = true;
    }
}

/// Expire the double-Esc window if more than 1 second has passed since
/// it was armed. Returns `true` if state changed (so the caller can
/// decide to redraw).
pub(crate) fn expire_esc_window(
    app: &mut App,
    esc_at: &mut Option<Instant>,
    now: Instant,
) -> bool {
    if let Some(at) = *esc_at {
        if now.duration_since(at) > Duration::from_secs(1) {
            *esc_at = None;
            app.esc_message = false;
            return true;
        }
    }
    false
}

/// Tab-completion over the token immediately to the left of `cursor`.
///
/// Dispatches on the first char of the token:
/// - Leading `"` → skip (argument/string context; deferred).
/// - Leading `:` → match against `vars` (global variable names) with the
///   sigil preserved on output.
/// - anything else → match against `commands` (primitives + procedures).
///
/// Returns `Some((new_input, new_cursor))` when the typed token gets
/// extended — either to the canonical spelling of a unique match, or to
/// the longest common prefix shared by all matches. Returns `None` when
/// there's nothing to do: the cursor is not at end-of-token, the token
/// is empty, no candidate matches, or the typed prefix is already as
/// long as the shared prefix.
pub(crate) fn complete_at_cursor(
    input: &str,
    cursor: usize,
    commands: &[String],
    vars: &[String],
) -> Option<(String, usize)> {
    if cursor > input.len() {
        return None;
    }
    // Cursor must be at end-of-token. Anything other than whitespace /
    // a bracket to the right of the cursor means we're mid-token and
    // leave the input alone.
    if let Some(next) = input[cursor..].chars().next() {
        if !is_token_boundary(next) {
            return None;
        }
    }

    let bytes = input.as_bytes();
    let mut start = cursor;
    while start > 0 {
        let prev = bytes[start - 1] as char;
        if is_token_boundary(prev) {
            break;
        }
        start -= 1;
    }
    if start == cursor {
        return None;
    }

    let token = &input[start..cursor];
    let (identifier, sigil_len, candidates) = match token.chars().next() {
        Some('"') => return None,
        Some(':') => (&token[1..], 1usize, vars),
        _ => (token, 0usize, commands),
    };
    if identifier.is_empty() {
        return None;
    }

    let identifier_lower = identifier.to_lowercase();
    let matches: Vec<&str> = candidates
        .iter()
        .filter(|c| c.to_lowercase().starts_with(&identifier_lower))
        .map(|s| s.as_str())
        .collect();
    if matches.is_empty() {
        return None;
    }

    let lcp = longest_common_prefix(&matches);
    if lcp.len() <= identifier.len() {
        // Either already at the shared prefix or the matched candidates
        // only agree on what the user already typed.
        return None;
    }

    // A "unique full fill" is when exactly one candidate matched and
    // we're extending to its canonical spelling. In that case, if the
    // completion lands at the very end of the input buffer, append a
    // space so the user can start typing the next token immediately.
    // Mid-buffer completions (cursor followed by `]`, existing space,
    // or further text) stay clean — don't invent separators that would
    // disturb surrounding structure.
    let full_fill = matches.len() == 1;
    let mut new_input = String::with_capacity(input.len() + lcp.len() + 1);
    new_input.push_str(&input[..start]);
    if sigil_len > 0 {
        new_input.push_str(&token[..sigil_len]);
    }
    new_input.push_str(lcp);
    new_input.push_str(&input[cursor..]);
    let mut new_cursor = start + sigil_len + lcp.len();
    if full_fill && new_cursor == new_input.len() {
        new_input.push(' ');
        new_cursor += 1;
    }
    Some((new_input, new_cursor))
}

fn is_token_boundary(c: char) -> bool {
    c.is_whitespace() || c == '[' || c == ']'
}

fn longest_common_prefix<'a>(strings: &[&'a str]) -> &'a str {
    let first = match strings.first() {
        Some(s) => *s,
        None => return "",
    };
    let mut end = first.len();
    for s in &strings[1..] {
        let shared = first
            .bytes()
            .zip(s.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        if shared < end {
            end = shared;
        }
        if end == 0 {
            break;
        }
    }
    &first[..end]
}

/// App-level Tab handler. Collects current completion candidates from
/// `app.evaluator` and delegates to `complete_at_cursor`. No-op while a
/// program is running so background evaluation isn't disturbed.
pub(crate) fn handle_tab_press(app: &mut App) {
    if app.busy {
        return;
    }
    let (commands, vars) = collect_completion_candidates(app);
    if let Some((new_input, new_cursor)) =
        complete_at_cursor(&app.input, app.cursor_position, &commands, &vars)
    {
        app.input = new_input;
        app.cursor_position = new_cursor;
    }
}

/// Gather names for Tab completion from the evaluator. Commands include
/// every registered primitive, primitive alias, and user-defined
/// procedure. Vars include every currently-bound global variable.
fn collect_completion_candidates(app: &App) -> (Vec<String>, Vec<String>) {
    let Some(eval) = app.evaluator() else {
        return (Vec::new(), Vec::new());
    };
    let commands: Vec<String> = eval.build_arity_map().keys().cloned().collect();
    let vars: Vec<String> = eval
        .global_vars_ref()
        .read()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    (commands, vars)
}

impl TerminalLifecycle {
    pub(crate) fn mark_raw_mode_enabled(&mut self) {
        self.raw_mode_enabled = true;
    }

    pub(crate) fn mark_alt_screen_entered(&mut self) {
        self.alt_screen_entered = true;
    }

    pub(crate) fn restore<T: TerminalRestorer>(&mut self, restorer: &mut T) -> io::Result<()> {
        if self.raw_mode_enabled {
            restorer.disable_raw_mode()?;
            self.raw_mode_enabled = false;
        }
        if self.alt_screen_entered {
            restorer.leave_alt_screen()?;
            self.alt_screen_entered = false;
        }
        restorer.show_cursor()?;
        Ok(())
    }
}

struct CrosstermRestorer<'a> {
    terminal: &'a mut Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalRestorer for CrosstermRestorer<'_> {
    fn disable_raw_mode(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }

    fn leave_alt_screen(&mut self) -> io::Result<()> {
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.terminal.show_cursor()?;
        Ok(())
    }
}

pub fn run(net_args: NetArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut lifecycle = TerminalLifecycle::default();

    // Setup terminal
    enable_raw_mode()?;
    lifecycle.mark_raw_mode_enabled();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    lifecycle.mark_alt_screen_entered();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = match App::new(net_args.role, env!("CARGO_PKG_VERSION"), net_args.password) {
        Ok(app) => app,
        Err(e) => {
            let mut restorer = CrosstermRestorer { terminal: &mut terminal };
            lifecycle.restore(&mut restorer)?;
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    let mut needs_draw = true;
    let mut ctrlc_at: Option<Instant> = None;
    let mut esc_at: Option<Instant> = None;

    // SIGINT: flag a quit request. The main loop polls this so cleanup runs.
    let sigint = Arc::new(AtomicBool::new(false));
    {
        let sigint = sigint.clone();
        let _ = ctrlc::set_handler(move || sigint.store(true, Ordering::SeqCst));
    }

    // Main loop
    loop {
        if sigint.load(Ordering::SeqCst) {
            break;
        }

        // Expire the Ctrl+C confirmation window
        if let Some(at) = ctrlc_at {
            if at.elapsed() > Duration::from_secs(1) {
                ctrlc_at = None;
                app.ctrlc_message = false;
                needs_draw = true;
            }
        }

        // Expire the double-Esc window
        if expire_esc_window(&mut app, &mut esc_at, Instant::now()) {
            needs_draw = true;
        }

        if needs_draw {
            terminal.draw(|frame| ui::draw(frame, &mut app))?;
            needs_draw = false;
        }

        // Poll with short timeout so we can check for background eval results
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Help mode
                if app.help_mode {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.help_mode = false,
                        KeyCode::Up => {
                            if app.help_scroll > 0 {
                                app.help_scroll -= 1;
                            }
                        }
                        KeyCode::Down => app.help_scroll += 1,
                        _ => {}
                    }
                    needs_draw = true;
                    continue;
                }

                // Ctrl+C — double-press to quit
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    if ctrlc_at.is_some() {
                        break;
                    }
                    ctrlc_at = Some(Instant::now());
                    app.ctrlc_message = true;
                    needs_draw = true;
                    continue;
                }

                // Escape — see `handle_escape_press` for the full
                // priority order.
                if key.code == KeyCode::Esc {
                    handle_escape_press(&mut app, &mut esc_at, Instant::now());
                    needs_draw = true;
                    continue;
                }

                let word_mod = key.modifiers.contains(KeyModifiers::ALT)
                    || key.modifiers.contains(KeyModifiers::CONTROL);

                match key.code {
                    KeyCode::Enter => {
                        if !app.busy {
                            app.submit_input();
                            if app.should_quit {
                                break;
                            }
                        }
                    }
                    // Tab — complete the current token against primitives,
                    // user procedures, or `:`-prefixed global variables.
                    KeyCode::Tab => {
                        handle_tab_press(&mut app);
                    }
                    // Ctrl+A — home
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.cursor_position = 0;
                    }
                    // Ctrl+E — end
                    KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.cursor_position = app.input.len();
                    }
                    // Ctrl+U — delete to start of line
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !app.busy && app.cursor_position > 0 {
                            app.input.drain(..app.cursor_position);
                            app.cursor_position = 0;
                        }
                    }
                    // Ctrl+K — delete to end of line
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !app.busy {
                            app.input.truncate(app.cursor_position);
                        }
                    }
                    // Ctrl+W — delete word backward
                    KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !app.busy && app.cursor_position > 0 {
                            let target = word_boundary_left(&app.input, app.cursor_position);
                            app.input.drain(target..app.cursor_position);
                            app.cursor_position = target;
                        }
                    }
                    // Alt+B (macOS Option+Left sends ESC+b) — move word left
                    KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
                        app.cursor_position = word_boundary_left(&app.input, app.cursor_position);
                    }
                    // Alt+F (macOS Option+Right sends ESC+f) — move word right
                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
                        app.cursor_position = word_boundary_right(&app.input, app.cursor_position);
                    }
                    // Alt+D (macOS Option+Delete sends ESC+d) — delete word forward
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::ALT) => {
                        if !app.busy && app.cursor_position < app.input.len() {
                            let target = word_boundary_right(&app.input, app.cursor_position);
                            app.input.drain(app.cursor_position..target);
                        }
                    }
                    KeyCode::Char(c) => {
                        if !app.busy {
                            app.input.insert(app.cursor_position, c);
                            app.cursor_position += 1;
                        }
                    }
                    // Alt+Backspace (macOS) / Ctrl+Backspace (Win/Linux) — delete word backward
                    KeyCode::Backspace if word_mod => {
                        if !app.busy && app.cursor_position > 0 {
                            let target = word_boundary_left(&app.input, app.cursor_position);
                            app.input.drain(target..app.cursor_position);
                            app.cursor_position = target;
                        }
                    }
                    KeyCode::Backspace => {
                        if !app.busy && app.cursor_position > 0 {
                            app.cursor_position -= 1;
                            app.input.remove(app.cursor_position);
                        }
                    }
                    // Alt+Delete (macOS) / Ctrl+Delete (Win/Linux) — delete word forward
                    KeyCode::Delete if word_mod => {
                        if !app.busy && app.cursor_position < app.input.len() {
                            let target = word_boundary_right(&app.input, app.cursor_position);
                            app.input.drain(app.cursor_position..target);
                        }
                    }
                    KeyCode::Delete => {
                        if !app.busy && app.cursor_position < app.input.len() {
                            app.input.remove(app.cursor_position);
                        }
                    }
                    // Alt+Left (macOS) / Ctrl+Left (Win/Linux) — move word left
                    KeyCode::Left if word_mod => {
                        app.cursor_position = word_boundary_left(&app.input, app.cursor_position);
                    }
                    KeyCode::Left => {
                        if app.cursor_position > 0 {
                            app.cursor_position -= 1;
                        }
                    }
                    // Alt+Right (macOS) / Ctrl+Right (Win/Linux) — move word right
                    KeyCode::Right if word_mod => {
                        app.cursor_position = word_boundary_right(&app.input, app.cursor_position);
                    }
                    KeyCode::Right => {
                        if app.cursor_position < app.input.len() {
                            app.cursor_position += 1;
                        }
                    }
                    KeyCode::Up => {
                        if !app.busy && app.multi_line.is_none() {
                            app.history_up();
                        }
                    }
                    KeyCode::Down => {
                        if !app.busy && app.multi_line.is_none() {
                            app.history_down();
                        }
                    }
                    KeyCode::Home => app.cursor_position = 0,
                    KeyCode::End => app.cursor_position = app.input.len(),
                    _ => {}
                }
                needs_draw = true;
            }
        }

        // Check for background evaluation results and new output
        if app.tick() {
            needs_draw = true;
        }
    }

    // Disconnect all hardware before tearing down the terminal so motors stop.
    app.disconnect_all_hardware();

    // Restore terminal
    let mut restorer = CrosstermRestorer {
        terminal: &mut terminal,
    };
    lifecycle.restore(&mut restorer)?;

    Ok(())
}

/// Find the start of the previous word (skip whitespace, then non-whitespace).
fn word_boundary_left(s: &str, pos: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = pos;
    // Skip whitespace going left
    while i > 0 && bytes[i - 1] == b' ' {
        i -= 1;
    }
    // Skip word characters going left
    while i > 0 && bytes[i - 1] != b' ' {
        i -= 1;
    }
    i
}

/// Find the end of the next word (skip non-whitespace, then whitespace).
fn word_boundary_right(s: &str, pos: usize) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = pos;
    // Skip word characters going right
    while i < len && bytes[i] != b' ' {
        i += 1;
    }
    // Skip whitespace going right
    while i < len && bytes[i] == b' ' {
        i += 1;
    }
    i
}

#[cfg(test)]
#[path = "tests/repl.rs"]
mod tests;
