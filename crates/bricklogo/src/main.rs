use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;
use std::time::Duration;

use bricklogo_tui::app::App;
use bricklogo_tui::ui;

#[derive(Default)]
struct TerminalLifecycle {
    raw_mode_enabled: bool,
    alt_screen_entered: bool,
}

trait TerminalRestorer {
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn leave_alt_screen(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
}

impl TerminalLifecycle {
    fn mark_raw_mode_enabled(&mut self) {
        self.raw_mode_enabled = true;
    }

    fn mark_alt_screen_entered(&mut self) {
        self.alt_screen_entered = true;
    }

    fn restore<T: TerminalRestorer>(&mut self, restorer: &mut T) -> io::Result<()> {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut lifecycle = TerminalLifecycle::default();

    // Setup terminal
    enable_raw_mode()?;
    lifecycle.mark_raw_mode_enabled();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    lifecycle.mark_alt_screen_entered();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut needs_draw = true;

    // Main loop
    loop {
        if needs_draw {
            terminal.draw(|frame| ui::draw(frame, &app))?;
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
                    } else if app.def_buffer.is_some() {
                        app.cancel_definition();
                    }
                    needs_draw = true;
                    continue;
                }

                match key.code {
                    KeyCode::Enter => {
                        if !app.busy {
                            app.submit_input();
                            if app.should_quit {
                                break;
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        if !app.busy {
                            app.input.insert(app.cursor_position, c);
                            app.cursor_position += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        if !app.busy && app.cursor_position > 0 {
                            app.cursor_position -= 1;
                            app.input.remove(app.cursor_position);
                        }
                    }
                    KeyCode::Delete => {
                        if !app.busy && app.cursor_position < app.input.len() {
                            app.input.remove(app.cursor_position);
                        }
                    }
                    KeyCode::Left => {
                        if app.cursor_position > 0 {
                            app.cursor_position -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if app.cursor_position < app.input.len() {
                            app.cursor_position += 1;
                        }
                    }
                    KeyCode::Up => {
                        if !app.busy && app.def_buffer.is_none() {
                            app.history_up();
                        }
                    }
                    KeyCode::Down => {
                        if !app.busy && app.def_buffer.is_none() {
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

    // Restore terminal
    let mut restorer = CrosstermRestorer {
        terminal: &mut terminal,
    };
    lifecycle.restore(&mut restorer)?;

    Ok(())
}

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;
