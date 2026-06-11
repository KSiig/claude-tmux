//! UI rendering for the TUI application
//!
//! This module provides all rendering functionality:
//! - Main layout and components (header, session list, preview, status, footer)
//! - Modal dialogs for user input
//! - Help screen and message overlays

mod dialogs;
mod help;

use ansi_to_tui::IntoText;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Clear, List, ListItem, Paragraph, StatefulWidget},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::{App, Mode};
use crate::session::ClaudeCodeStatus;

/// Render the application UI
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Calculate preview height (roughly 50% of available space, min 8, max 20 lines)
    let available_height = area.height.saturating_sub(4); // minus header, status, footer
    let preview_height = (available_height * 50 / 100).clamp(8, 20);

    // Main layout: header, session list, preview, status bar, footer
    let layout = Layout::vertical([
        Constraint::Length(1),              // Header
        Constraint::Min(3),                 // Session list
        Constraint::Length(preview_height), // Preview pane
        Constraint::Length(1),              // Status bar
        Constraint::Length(1),              // Footer
    ])
    .split(area);

    render_header(frame, app, layout[0]);
    render_session_list(frame, app, layout[1]);
    render_preview(frame, app, layout[2]);
    render_status_bar(frame, app, layout[3]);
    render_footer(frame, app, layout[4]);

    // Render modal overlays
    match &app.mode {
        Mode::ConfirmAction => {
            dialogs::render_confirm_action(frame, app);
        }
        Mode::NewSession {
            name,
            path,
            field,
            path_suggestions,
            path_selected,
        } => {
            dialogs::render_new_session_dialog(
                frame,
                name,
                path,
                *field,
                path_suggestions,
                *path_selected,
            );
        }
        Mode::Rename { old_name, new_name } => {
            dialogs::render_rename_dialog(frame, old_name, new_name);
        }
        Mode::Commit { message } => {
            dialogs::render_commit_dialog(frame, message);
        }
        Mode::NewWorktree {
            branch_input,
            selected_branch,
            worktree_path,
            session_name,
            field,
            path_suggestions,
            path_selected,
            ..
        } => {
            dialogs::render_new_worktree_dialog(
                frame,
                app,
                branch_input,
                *selected_branch,
                worktree_path,
                session_name,
                *field,
                path_suggestions,
                *path_selected,
            );
        }
        Mode::Filter { input } => {
            render_filter_bar(frame, input, layout[3]);
        }
        Mode::CreatePullRequest {
            title,
            body,
            base_branch,
            field,
        } => {
            dialogs::render_create_pr_dialog(frame, title, body, base_branch, *field);
        }
        Mode::Help => {
            help::render_help(frame, app.task_show_status);
        }
        Mode::SetStatus { selected } => {
            let current = app
                .selected_session()
                .map(|s| s.claude_code_status)
                .unwrap_or_default();
            dialogs::render_set_status_dialog(frame, *selected, current);
        }
        Mode::ForkSession {
            source_session,
            branch_name,
            ..
        } => {
            dialogs::render_fork_session_dialog(frame, source_session, branch_name);
        }
        Mode::Normal | Mode::ActionMenu => {}
    }

    // Render error/message overlay
    if let Some(ref error) = app.error {
        help::render_message(frame, error, Color::Red);
    } else if let Some(ref message) = app.message {
        help::render_message(frame, message, Color::Green);
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let current = app
        .current_session
        .as_ref()
        .map(|s| format!(" attached: {} ", s))
        .unwrap_or_default();

    let title = format!(
        "─ claude-tmux ─{:─>width$}",
        current,
        width = area.width as usize - 15
    );

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    frame.render_widget(header, area);
}

fn render_session_list(frame: &mut Frame, app: &mut App, area: Rect) {
    // Compute scroll state values before borrowing for items
    let selected_index = app.compute_flat_list_index();
    let total_items = app.compute_total_list_items();
    let visible_height = area.height as usize;

    // Take scroll_state out of app to avoid borrow conflicts
    // (items building borrows app immutably, scroll_state needs mutable access)
    let mut scroll_state = std::mem::take(&mut app.scroll_state);

    let groups = app.grouped_filtered_sessions();

    let has_navigable = groups.iter().any(|g| {
        !g.sessions.is_empty() || (g.hidden_count > 0 && g.label.is_some())
    });

    if !has_navigable {
        let empty_msg = if app.filter.is_empty() {
            "No tmux sessions found. Press 'n' to create one."
        } else {
            "No sessions match the filter."
        };
        let paragraph = Paragraph::new(empty_msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        app.scroll_state = scroll_state;
        return;
    }

    let display_names: Vec<String> = groups
        .iter()
        .flat_map(|g| {
            g.sessions.iter().map(move |s| {
                let base = s.display_name();
                if g.strip_prefix {
                    if let Some(label) = &g.label {
                        if let Some(stripped) = base.strip_prefix(label.as_str()) {
                            return stripped.strip_prefix('-').unwrap_or(stripped).to_string();
                        }
                    }
                }
                base
            })
        })
        .collect();
    let max_name_len = display_names
        .iter()
        .map(|n| n.as_str().width())
        .max()
        .unwrap_or(10)
        .max(10);

    let mut items: Vec<ListItem> = Vec::new();
    let mut session_idx = 0;
    let mut nav_idx = 0;

    for group in &groups {
        let is_collapsed = group.hidden_count > 0 && group.sessions.is_empty();

        if let Some(ref label) = group.label {
            let linear_status = if app.task_show_status {
                app.linear_statuses.get(label)
            } else {
                None
            };
            let title = if app.task_show_titles {
                group.title.as_deref()
            } else {
                None
            };
            let is_selected = is_collapsed && nav_idx == app.selected;
            let header_line =
                render_group_header(label, title, linear_status, app.task_status_labels, area.width, group.hidden_count, &group.hidden_statuses, is_selected);
            let mut item = ListItem::new(header_line);
            if is_selected {
                item = item.style(Style::default().bg(Color::DarkGray));
            }
            items.push(item);
            if is_collapsed {
                nav_idx += 1;
            }
        } else if group.separator {
            let sep = "─".repeat(area.width as usize);
            items.push(ListItem::new(Line::from(
                Span::styled(sep, Style::default().fg(Color::DarkGray)),
            )));
        }

        for session in &group.sessions {
            let i = session_idx;
            session_idx += 1;
            let current_nav = nav_idx;
            nav_idx += 1;

            let is_selected = current_nav == app.selected;
            let is_current = app
                .current_session
                .as_ref()
                .is_some_and(|c| c == &session.name);

            let is_expanded = is_selected && matches!(app.mode, Mode::ActionMenu);
            let marker = if is_selected {
                if is_expanded {
                    "▾"
                } else {
                    "▸"
                }
            } else {
                " "
            };
            let status = &session.claude_code_status;

            let status_color = match (status, is_selected) {
                (ClaudeCodeStatus::Working, _) => Color::Green,
                (ClaudeCodeStatus::Done, _) => Color::Cyan,
                (ClaudeCodeStatus::WaitingInput, _) => Color::Yellow,
                (ClaudeCodeStatus::Error, _) => Color::Red,
                (ClaudeCodeStatus::Idle, true) => Color::White,
                (ClaudeCodeStatus::Idle, false) => Color::DarkGray,
                (ClaudeCodeStatus::Unknown, true) => Color::Gray,
                (ClaudeCodeStatus::Unknown, false) => Color::DarkGray,
            };

            let name_style = if is_current {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let git_spans = if app.show_git_info {
                build_git_spans(session)
            } else {
                vec![]
            };

            let is_child = group.label.as_deref().is_some_and(|l| session.name != l);
            let inline_title = if app.task_show_titles && is_child {
                app.group_titles.get(&session.name).map(|t| t.as_str())
            } else {
                None
            };
            let detail = inline_title
                .map(|t| t.to_string())
                .unwrap_or_else(|| session.display_path());
            let detail_color = if inline_title.is_some() {
                Color::White
            } else if is_selected {
                Color::White
            } else {
                Color::DarkGray
            };

            let sub_issue_status = if app.task_show_status {
                crate::linear::session_sub_issue_id(
                    &session.name,
                    app.task_issue_prefix.as_deref(),
                )
                .and_then(|id| app.linear_statuses.get(&id))
            } else {
                None
            };

            let mut line_spans = vec![
                Span::raw(format!(" {} ", marker)),
                Span::styled(
                    format!("{:<width$}", display_names[i], width = max_name_len),
                    name_style,
                ),
                Span::raw(" "),
                Span::styled(status.symbol(), Style::default().fg(status_color)),
            ];
            if app.session_status_labels {
                line_spans.push(Span::raw(" "));
                line_spans.push(Span::styled(
                    format!("{:<8}", status.label()),
                    Style::default().fg(status_color),
                ));
            }
            if let Some(&since) = session.claude_code_pane.as_ref()
                .and_then(|id| app.status_since.get(id))
            {
                line_spans.push(Span::styled(
                    format!(" {:<4}", format_elapsed(since)),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            line_spans.push(Span::raw("  "));
            if let Some(s) = sub_issue_status {
                let color = linear_state_color(&s.state_type);
                let text = if app.task_status_labels {
                    format!("{} {} ", linear_state_symbol(&s.state_type), s.state_name)
                } else {
                    format!("{} ", linear_state_symbol(&s.state_type))
                };
                line_spans.push(Span::styled(text, Style::default().fg(color)));
            }
            line_spans.push(Span::styled(detail, Style::default().fg(detail_color)));
            line_spans.extend(git_spans);

            let line = Line::from(line_spans);

            let style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            items.push(ListItem::new(line).style(style));

            if is_expanded {
                render_expanded_session_content(app, session, &mut items);
            }
        }
    }

    // Scope the list rendering so borrows are released before we restore scroll_state
    {
        let list = List::new(items);

        // Update scroll state with centered scrolling behavior
        let list_state = scroll_state.update(selected_index, total_items, visible_height);

        // Render with stateful widget for proper scrolling
        StatefulWidget::render(list, area, frame.buffer_mut(), list_state);
    }

    // Put scroll_state back into app (list borrows are now released)
    app.scroll_state = scroll_state;
}

fn render_group_header<'a>(
    label: &str,
    title: Option<&str>,
    linear_status: Option<&crate::linear::IssueStatus>,
    status_labels: bool,
    width: u16,
    hidden_count: usize,
    hidden_statuses: &std::collections::HashMap<ClaudeCodeStatus, usize>,
    is_selected: bool,
) -> Line<'a> {
    let marker = if is_selected { " ▸ " } else { "" };
    let prefix = if is_selected { "─ " } else { "── " };
    let title_part = match title {
        Some(t) => format!(" — {}", t),
        None => String::new(),
    };
    let hidden_part = if hidden_count > 0 {
        hidden_status_summary(hidden_statuses)
    } else {
        String::new()
    };
    let status_text = match linear_status {
        Some(s) if status_labels => format!("{} {} ", linear_state_symbol(&s.state_type), s.state_name),
        Some(s) => format!("{} ", linear_state_symbol(&s.state_type)),
        None => String::new(),
    };
    let used = marker.len() + prefix.len() + status_text.len() + label.len() + title_part.len() + hidden_part.len() + 2;
    let dashes_right = "─".repeat((width as usize).saturating_sub(used).max(1));

    let mut spans = Vec::new();
    if is_selected {
        spans.push(Span::styled(marker, Style::default().fg(Color::White)));
    }
    spans.push(Span::styled(prefix.to_string(), Style::default().fg(Color::DarkGray)));
    if let Some(s) = linear_status {
        let color = linear_state_color(&s.state_type);
        spans.push(Span::styled(
            status_text,
            Style::default().fg(color),
        ));
    }
    let label_color = if is_selected { Color::White } else if hidden_count > 0 { Color::DarkGray } else { Color::Cyan };
    spans.push(Span::styled(
        label.to_string(),
        Style::default()
            .fg(label_color)
            .add_modifier(Modifier::BOLD),
    ));
    if let Some(t) = title {
        spans.push(Span::styled(
            format!(" — {}", t),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if hidden_count > 0 {
        spans.extend(hidden_status_spans(hidden_statuses));
    }
    spans.push(Span::styled(
        format!(" {}", dashes_right),
        Style::default().fg(Color::DarkGray),
    ));
    Line::from(spans)
}

fn hidden_status_summary(statuses: &std::collections::HashMap<ClaudeCodeStatus, usize>) -> String {
    use ClaudeCodeStatus::*;
    let mut parts = Vec::new();
    for status in &[Working, Done, WaitingInput, Error, Idle, Unknown] {
        if let Some(&count) = statuses.get(status) {
            parts.push(format!("{}{}", status.symbol(), count));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(" "))
    }
}

fn hidden_status_spans(statuses: &std::collections::HashMap<ClaudeCodeStatus, usize>) -> Vec<Span<'static>> {
    use ClaudeCodeStatus::*;
    let order = [Working, Done, WaitingInput, Error, Idle, Unknown];
    let mut spans = Vec::new();
    let mut first = true;
    for status in &order {
        if let Some(&count) = statuses.get(status) {
            let color = match status {
                Working => Color::Green,
                Done => Color::Cyan,
                WaitingInput => Color::Yellow,
                Error => Color::Red,
                Idle => Color::DarkGray,
                Unknown => Color::DarkGray,
            };
            if first {
                spans.push(Span::styled(" (", Style::default().fg(Color::DarkGray)));
                first = false;
            } else {
                spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
            }
            spans.push(Span::styled(
                format!("{}{}", status.symbol(), count),
                Style::default().fg(color),
            ));
        }
    }
    if !first {
        spans.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
    }
    spans
}

fn linear_state_color(state_type: &str) -> Color {
    match state_type {
        "completed" => Color::Green,
        "started" => Color::Yellow,
        "unstarted" => Color::DarkGray,
        "cancelled" => Color::Red,
        "backlog" => Color::DarkGray,
        _ => Color::Gray,
    }
}

fn linear_state_symbol(state_type: &str) -> &'static str {
    match state_type {
        "completed" => "■",
        "started" => "▣",
        "unstarted" => "□",
        "cancelled" => "✕",
        "backlog" => "□",
        _ => "□",
    }
}

fn build_git_spans<'a>(session: &'a crate::session::Session) -> Vec<Span<'a>> {
    let Some(ref git) = session.git_context else {
        return vec![];
    };

    let (open, close) = if git.is_worktree {
        ("[", "]")
    } else {
        ("(", ")")
    };
    let bracket_color = if git.is_worktree {
        Color::Magenta
    } else {
        Color::Cyan
    };

    let mut status_str = String::new();
    if git.has_staged {
        status_str.push('+');
    }
    if git.has_unstaged {
        status_str.push('*');
    }
    let status_spans = if !status_str.is_empty() {
        let color = if git.has_staged && !git.has_unstaged {
            Color::Green
        } else {
            Color::Yellow
        };
        vec![Span::styled(
            format!(" {}", status_str),
            Style::default().fg(color),
        )]
    } else {
        vec![]
    };

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(open, Style::default().fg(bracket_color)),
        Span::styled(&git.branch, Style::default().fg(Color::Cyan)),
        Span::styled(close, Style::default().fg(bracket_color)),
    ];
    spans.extend(status_spans);
    spans
}

/// Render the expanded content for a session in action menu mode
fn render_expanded_session_content<'a>(
    app: &'a App,
    session: &'a crate::session::Session,
    items: &mut Vec<ListItem<'a>>,
) {
    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);

    // Session metadata row
    let attached_str = if session.attached { "yes" } else { "no" };
    let pane_count = session.panes.len();

    let meta_line = Line::from(vec![
        Span::raw("     "),
        Span::styled("windows: ", label_style),
        Span::styled(format!("{}", session.window_count), value_style),
        Span::raw("  "),
        Span::styled("panes: ", label_style),
        Span::styled(format!("{}", pane_count), value_style),
        Span::raw("  "),
        Span::styled("uptime: ", label_style),
        Span::styled(session.duration(), value_style),
        Span::raw("  "),
        Span::styled("attached: ", label_style),
        Span::styled(attached_str, value_style),
    ]);
    items.push(ListItem::new(meta_line));

    // Git metadata row (if available)
    if let Some(ref git) = session.git_context {
        let mut git_spans = vec![
            Span::raw("     "),
            Span::styled("branch: ", label_style),
            Span::styled(&git.branch, Style::default().fg(Color::Cyan)),
        ];

        if git.ahead > 0 || git.behind > 0 {
            git_spans.push(Span::raw("  "));
            if git.ahead > 0 {
                git_spans.push(Span::styled(
                    format!("↑{}", git.ahead),
                    Style::default().fg(Color::Green),
                ));
            }
            if git.behind > 0 {
                if git.ahead > 0 {
                    git_spans.push(Span::raw(" "));
                }
                git_spans.push(Span::styled(
                    format!("↓{}", git.behind),
                    Style::default().fg(Color::Red),
                ));
            }
        }

        // Show staged/unstaged status
        if git.has_staged {
            git_spans.push(Span::raw("  "));
            git_spans.push(Span::styled("staged: ", label_style));
            git_spans.push(Span::styled("yes", Style::default().fg(Color::Green)));
        }

        if git.has_unstaged {
            git_spans.push(Span::raw("  "));
            git_spans.push(Span::styled("unstaged: ", label_style));
            git_spans.push(Span::styled("yes", Style::default().fg(Color::Yellow)));
        }

        if git.is_worktree {
            git_spans.push(Span::raw("  "));
            git_spans.push(Span::styled("worktree: ", label_style));
            git_spans.push(Span::styled("yes", Style::default().fg(Color::Magenta)));
        }

        items.push(ListItem::new(Line::from(git_spans)));

        // PR status row (if available)
        if let Some(ref pr_info) = app.pr_info {
            let mut pr_spans = vec![
                Span::raw("     "),
                Span::styled("PR #", label_style),
                Span::styled(
                    format!("{}", pr_info.number),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(": "),
            ];

            // State with color
            let (state_text, state_color) = match pr_info.state.as_str() {
                "OPEN" => ("open", Color::Green),
                "CLOSED" => ("closed", Color::Red),
                "MERGED" => ("merged", Color::Magenta),
                _ => (pr_info.state.as_str(), Color::Gray),
            };
            pr_spans.push(Span::styled(state_text, Style::default().fg(state_color)));

            // Mergeable status (only show for open PRs)
            if pr_info.state == "OPEN" {
                pr_spans.push(Span::raw("  "));
                let (merge_text, merge_color) = match pr_info.mergeable.as_str() {
                    "MERGEABLE" => ("ready to merge", Color::Green),
                    "CONFLICTING" => ("has conflicts", Color::Red),
                    _ => ("merge status unknown", Color::Yellow),
                };
                pr_spans.push(Span::styled(merge_text, Style::default().fg(merge_color)));
            }

            items.push(ListItem::new(Line::from(pr_spans)));
        }
    }

    // Separator
    let sep_line = Line::from(Span::styled(
        "     ────────────────────────",
        Style::default().fg(Color::DarkGray),
    ));
    items.push(ListItem::new(sep_line));

    // Action items
    for (action_idx, action) in app.available_actions.iter().enumerate() {
        let is_action_selected = action_idx == app.selected_action;
        let action_marker = if is_action_selected { "▸" } else { " " };
        let action_style = if is_action_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let action_line = Line::from(vec![
            Span::raw("     "),
            Span::styled(format!("{} {}", action_marker, action.label()), action_style),
        ]);
        items.push(ListItem::new(action_line));
    }

    // White separator at end of submenu
    let end_sep = Line::from(Span::styled("", Style::default().fg(Color::White)));
    items.push(ListItem::new(end_sep));
}

fn render_preview(frame: &mut Frame, app: &App, area: Rect) {
    // Clear the entire preview area first to prevent stale content
    frame.render_widget(Clear, area);

    // Draw separator lines at top and bottom
    let separator = "─".repeat(area.width as usize);

    let top_sep_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    let top_sep = Paragraph::new(separator.clone()).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(top_sep, top_sep_area);

    let bottom_sep_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    let bottom_sep = Paragraph::new(separator).style(Style::default().fg(Color::White));
    frame.render_widget(bottom_sep, bottom_sep_area);

    // Content area (between separators)
    let content_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(2),
    };

    let content = match &app.preview_content {
        Some(text) if !text.is_empty() => text,
        _ => {
            let msg = Paragraph::new("  No preview available")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(msg, content_area);
            return;
        }
    };

    // Parse ANSI escape sequences into styled ratatui Text
    let styled_text = match content.into_text() {
        Ok(text) => text,
        Err(_) => {
            // Fallback to plain text if parsing fails
            Text::raw(content)
        }
    };

    // Take only the last N lines that fit in the content area
    let available_lines = content_area.height as usize;
    let total_lines = styled_text.lines.len();
    let start = total_lines.saturating_sub(available_lines);
    let visible_lines: Vec<Line> = styled_text.lines.into_iter().skip(start).collect();

    let preview = Paragraph::new(visible_lines);
    frame.render_widget(preview, content_area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let (working, waiting, _idle, done, error, _unknown) = app.status_counts();
    let total = app.sessions.len();

    let mut spans = vec![
        Span::styled(format!("  {} sessions", total), Style::default().fg(Color::DarkGray)),
    ];

    if app.session_status_labels {
        if working > 0 {
            spans.push(Span::styled(
                format!(" | {} working", working),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if done > 0 {
            spans.push(Span::styled(
                format!(" | {} done", done),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if waiting > 0 {
            spans.push(Span::styled(
                format!(" | {} awaiting input", waiting),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if error > 0 {
            spans.push(Span::styled(
                format!(" | {} error", error),
                Style::default().fg(Color::Red),
            ));
        }
    } else {
        if working > 0 {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{}{}", ClaudeCodeStatus::Working.symbol(), working),
                Style::default().fg(Color::Green),
            ));
        }
        if done > 0 {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{}{}", ClaudeCodeStatus::Done.symbol(), done),
                Style::default().fg(Color::Cyan),
            ));
        }
        if waiting > 0 {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{}{}", ClaudeCodeStatus::WaitingInput.symbol(), waiting),
                Style::default().fg(Color::Yellow),
            ));
        }
        if error > 0 {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{}{}", ClaudeCodeStatus::Error.symbol(), error),
                Style::default().fg(Color::Red),
            ));
        }
    }

    if !app.hidden_groups.is_empty() {
        spans.push(Span::styled(
            format!("  {} hidden", app.hidden_groups.len()),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if !app.filter.is_empty() {
        spans.push(Span::styled(
            format!("  filter: \"{}\"", app.filter),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let bar = Paragraph::new(Line::from(spans));
    frame.render_widget(bar, area);
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let hints = match app.mode {
        Mode::Normal => {
            "  ? help  jk navigate  l actions  ⏎ switch  n new  K kill  S status  H hide  U unhide  R reload  / filter  q quit"
        }
        Mode::ActionMenu => "  jk navigate  ⏎/l select  h/esc back  q quit",
        Mode::Filter { .. } => "  ⏎ apply  esc cancel",
        Mode::ConfirmAction => "  y/⏎ confirm  n/esc cancel",
        Mode::NewSession { .. } => "  ⏎ create  tab switch  ↑↓ select  → accept  esc cancel",
        Mode::Rename { .. } => "  ⏎ confirm  esc cancel",
        Mode::Commit { .. } => "  ⏎ commit  esc cancel",
        Mode::NewWorktree { .. } => "  ⏎ create  tab switch  ↑↓ select  → accept  esc cancel",
        Mode::CreatePullRequest { .. } => "  ⏎ create PR  tab switch  esc cancel",
        Mode::SetStatus { .. } => "  jk navigate  ⏎ confirm  esc cancel",
        Mode::ForkSession { .. } => "  ⏎ fork  esc cancel",
        Mode::Help => "  q close",
    };

    let footer = Paragraph::new(hints).style(Style::default().fg(Color::DarkGray));

    frame.render_widget(footer, area);
}

fn format_elapsed(since_epoch: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = now.saturating_sub(since_epoch);
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

fn render_filter_bar(frame: &mut Frame, input: &str, area: Rect) {
    frame.render_widget(Clear, area);
    let text = format!("  / {}", input);
    let bar = Paragraph::new(text).style(Style::default().fg(Color::Yellow));
    frame.render_widget(bar, area);
}
