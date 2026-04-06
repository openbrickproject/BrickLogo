use std::io;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use bricklogo_tui::app::App;
use bricklogo_tui::ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
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
                if key.kind != KeyEventKind::Press { continue; }

                // Help mode
                if app.help_mode {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.help_mode = false,
                        KeyCode::Up => {
                            if app.help_scroll > 0 { app.help_scroll -= 1; }
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
                            if app.should_quit { break; }
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
                        if app.cursor_position > 0 { app.cursor_position -= 1; }
                    }
                    KeyCode::Right => {
                        if app.cursor_position < app.input.len() { app.cursor_position += 1; }
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
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
