//! Help screen and message overlays

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render_help(frame: &mut Frame, task_integration_active: bool) {
    let mut help_text = vec![
        Line::from(Span::styled(
            "Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw("  j / ↓       Move down"),
        Line::raw("  k / ↑       Move up"),
        Line::raw("  l / →       Open action menu"),
        Line::raw("  Enter       Switch to session"),
        Line::raw(""),
        Line::from(Span::styled(
            "Actions",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw("  n           New session"),
        Line::raw("  K           Kill session"),
        Line::raw("  r           Rename session"),
        Line::raw("  S           Set status"),
        Line::raw("  /           Filter sessions"),
        Line::raw("  R           Refresh list"),
        Line::raw("  H           Hide group"),
        Line::raw("  U           Unhide group (or all)"),
        Line::raw(""),
        Line::from(Span::styled(
            "Action Menu",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw("  h / ←       Go back"),
        Line::raw("  Enter       Execute action"),
        Line::raw(""),
        Line::from(Span::styled(
            "Other",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw("  ?           Show this help"),
        Line::raw("  q / Esc     Quit"),
        Line::raw(""),
        Line::from(Span::styled(
            "Session status",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("●", Style::default().fg(Color::Green)),
            Span::raw("  Working     "),
            Span::styled("◉", Style::default().fg(Color::Cyan)),
            Span::raw("  Done"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("◐", Style::default().fg(Color::Yellow)),
            Span::raw("  Input       "),
            Span::styled("○", Style::default().fg(Color::DarkGray)),
            Span::raw("  Idle"),
        ]),
    ];

    if task_integration_active {
        help_text.push(Line::raw(""));
        help_text.push(Line::from(Span::styled(
            "Task status",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        help_text.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("▣", Style::default().fg(Color::Yellow)),
            Span::raw("  In progress "),
            Span::styled("■", Style::default().fg(Color::Green)),
            Span::raw("  Completed"),
        ]));
        help_text.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("□", Style::default().fg(Color::DarkGray)),
            Span::raw("  Not started "),
            Span::styled("✕", Style::default().fg(Color::Red)),
            Span::raw("  Cancelled"),
        ]));
        help_text.push(Line::raw(""));
        help_text.push(Line::from(Span::styled(
            "  Requires headless daemon (--headless)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let height = help_text.len() as u16 + 2; // +2 for border
    let area = centered_rect(60, height, frame.area());

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

pub fn render_message(frame: &mut Frame, message: &str, color: Color) {
    let area = frame.area();

    // Calculate height needed (at least 1, up to 3 for longer messages)
    let max_width = area.width.saturating_sub(6) as usize;
    let lines_needed = if max_width > 0 {
        (message.len() / max_width + 1).min(3)
    } else {
        1
    };
    let height = lines_needed as u16;

    let msg_area = Rect {
        x: 2,
        y: area.height.saturating_sub(2 + height),
        width: area.width.saturating_sub(4),
        height,
    };

    let text = format!(" {} ", message);
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::White).bg(color))
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, msg_area);
    frame.render_widget(paragraph, msg_area);
}

/// Create a centered rectangle of the given size within the parent area
pub fn centered_rect(width: u16, height: u16, parent: Rect) -> Rect {
    let x = parent.x + (parent.width.saturating_sub(width)) / 2;
    let y = parent.y + (parent.height.saturating_sub(height)) / 2;

    Rect {
        x,
        y,
        width: width.min(parent.width),
        height: height.min(parent.height),
    }
}
