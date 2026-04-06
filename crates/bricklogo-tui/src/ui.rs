use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use crate::app::{App, OutputLineType};

const PINK: Color = Color::Rgb(255, 20, 147);
const BLUE: Color = Color::Rgb(0, 191, 255);

const LOGO_TOP: &str = r#" ____       _      _    _
| __ ) _ __(_) ___| | _| |    ___   __ _  ___
|  _ \| '__| |/ __| |/ / |   / _ \ / _` |/ _ \"#;

const LOGO_BOTTOM: &str = r#"| |_) | |  | | (__|   <| |__| (_) | (_| | (_) |
|____/|_|  |_|\___|_|\_\_____\___/ \__, |\___/
                                   |___/"#;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PINK))
        .style(Style::default().bg(Color::Black));

    frame.render_widget(outer_block, size);

    let inner = Rect {
        x: size.x + 2,
        y: size.y + 1,
        width: size.width.saturating_sub(4),
        height: size.height.saturating_sub(2),
    };

    if app.help_mode {
        draw_help(frame, app, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // header
            Constraint::Length(1), // status bar
            Constraint::Min(1),   // repl
        ])
        .split(inner);

    draw_header(frame, chunks[0]);
    draw_status_bar(frame, app, chunks[1]);
    draw_repl(frame, app, chunks[2]);
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for line in LOGO_TOP.lines() {
        lines.push(Line::from(Span::styled(line, Style::default().fg(BLUE).add_modifier(Modifier::BOLD))));
    }
    for line in LOGO_BOTTOM.lines() {
        lines.push(Line::from(Span::styled(line, Style::default().fg(PINK).add_modifier(Modifier::BOLD))));
    }
    lines.push(Line::from(Span::styled(
        "A modern LEGO TC Logo REPL, by the Open Brick Project",
        Style::default().fg(Color::DarkGray),
    )));

    let header = Paragraph::new(lines);
    frame.render_widget(header, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status = if !app.connected_devices.is_empty() {
        let names = app.connected_devices.join(", ");
        Line::from(vec![
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled("● ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(names, Style::default().fg(Color::Green)),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled("● ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Disconnected", Style::default().fg(Color::Red)),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
        ])
    };

    frame.render_widget(Paragraph::new(status), area);
}

fn draw_repl(frame: &mut Frame, app: &App, area: Rect) {
    let available_height = area.height as usize;
    if available_height < 2 { return; }

    let max_lines = available_height - 1; // reserve 1 for prompt
    let start = app.output_lines.len().saturating_sub(max_lines);
    let visible = &app.output_lines[start..];

    let mut lines: Vec<Line> = visible.iter().map(|ol| {
        let color = match ol.line_type {
            OutputLineType::Input => Color::DarkGray,
            OutputLineType::Output => Color::White,
            OutputLineType::Error => Color::Red,
            OutputLineType::System => Color::Cyan,
        };
        Line::from(Span::styled(&ol.text, Style::default().fg(color)))
    }).collect();

    // Add prompt line
    let prompt = app.get_prompt();
    let prompt_line = Line::from(vec![
        Span::styled(prompt, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(&app.input, Style::default().fg(Color::White)),
    ]);
    lines.push(prompt_line);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);

    // Position cursor on the prompt line (after output lines)
    let cursor_x = area.x + prompt.len() as u16 + app.cursor_position as u16;
    let cursor_y = area.y + visible.len() as u16;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let lines_data = app.help_lines();
    let available = area.height.saturating_sub(1) as usize; // 1 for status line
    let max_scroll = lines_data.len().saturating_sub(available);
    let scroll = app.help_scroll.min(max_scroll);

    let visible = &lines_data[scroll..];
    let lines: Vec<Line> = visible.iter().take(available).map(|l| {
        Line::from(Span::styled(l.as_str(), Style::default().fg(Color::Cyan)))
    }).collect();

    let mut all_lines = lines;
    // Status line
    let status = format!(
        "  {}Press q to close, ↑↓ to scroll{}",
        if scroll > 0 { "↑ " } else { "  " },
        if scroll < max_scroll { " ↓" } else { "" },
    );
    all_lines.push(Line::from(Span::styled(status, Style::default().fg(Color::DarkGray))));

    let paragraph = Paragraph::new(all_lines);
    frame.render_widget(paragraph, area);
}
