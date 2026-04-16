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
use std::time::Duration;

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

                // Ctrl+C to quit
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                // Escape
                if key.code == KeyCode::Esc {
                    if app.busy {
                        app.request_stop();
                    } else if app.multi_line.is_some() {
                        app.cancel_definition();
                    }
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
