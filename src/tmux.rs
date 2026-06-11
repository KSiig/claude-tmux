use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use crate::detection::content::detect_from_content;
use crate::git::GitContext;
use crate::session::{ClaudeCodeStatus, Pane, Session};

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub index: String,
    pub name: String,
    pub layout: String,
}

/// Wrapper for tmux command execution
pub struct Tmux;

impl Tmux {
    /// List all tmux sessions with their metadata and detect Claude status
    pub fn list_sessions() -> Result<Vec<Session>> {
        Self::list_sessions_inner(true)
    }

    /// List all tmux sessions without running status detection or git context.
    /// Used by the headless daemon where tick_status() handles detection separately.
    pub fn list_sessions_fast() -> Result<Vec<Session>> {
        Self::list_sessions_inner(false)
    }

    fn list_sessions_inner(detect: bool) -> Result<Vec<Session>> {
        let output = Command::new("tmux")
            .args([
                "list-sessions",
                "-F",
                "#{session_name}|||#{session_created}|||#{session_attached}|||#{session_windows}|||#{session_activity}",
            ])
            .output()
            .context("Failed to execute tmux list-sessions")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no server running") || stderr.contains("no sessions") {
                return Ok(Vec::new());
            }
            anyhow::bail!("tmux list-sessions failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split("|||").collect();
            if parts.len() >= 4 {
                let name = parts[0].to_string();
                let created = parts[1].parse().unwrap_or(0);
                let attached = parts[2] == "1";
                let window_count = parts[3].parse().unwrap_or(1);
                let last_activity = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);

                // Get panes for this session
                let panes = Self::list_panes(&name).unwrap_or_default();

                // Find every non-excluded pane running claude
                let claude_panes: Vec<&Pane> = panes
                    .iter()
                    .filter(|p| !p.excluded && (p.current_command == "claude" || p.current_command.contains("claude")))
                    .collect();

                // Emit one Session row per claude pane. Sessions with zero
                // claude panes still produce a single row with no claude info.
                let multi = claude_panes.len() > 1;

                if claude_panes.is_empty() {
                    let working_directory = panes
                        .first()
                        .map(|p| p.current_path.clone())
                        .unwrap_or_default();
                    let git_context = if detect {
                        GitContext::detect(&working_directory)
                    } else {
                        None
                    };

                    sessions.push(Session {
                        name: name.clone(),
                        created,
                        last_activity,
                        attached,
                        working_directory,
                        window_count,
                        panes: panes.clone(),
                        claude_code_pane: None,
                        claude_code_status: ClaudeCodeStatus::Unknown,
                        window_label: None,
                        target_window_index: None,
                        git_context,
                    });
                } else {
                    for claude_pane in claude_panes {
                        let status = if detect {
                            Self::capture_pane(&claude_pane.id, 15, true)
                                .map(|content| detect_from_content(&content))
                                .unwrap_or(ClaudeCodeStatus::Unknown)
                        } else {
                            ClaudeCodeStatus::Unknown
                        };

                        let working_directory = claude_pane.current_path.clone();
                        let git_context = if detect {
                            GitContext::detect(&working_directory)
                        } else {
                            None
                        };

                        let (window_label, target_window_index) = if multi {
                            (
                                Some(claude_pane.window_name.clone()),
                                Some(claude_pane.window_index.clone()),
                            )
                        } else {
                            (None, None)
                        };

                        sessions.push(Session {
                            name: name.clone(),
                            created,
                            last_activity,
                            attached,
                            working_directory,
                            window_count,
                            panes: panes.clone(),
                            claude_code_pane: Some(claude_pane.id.clone()),
                            claude_code_status: status,
                            window_label,
                            target_window_index,
                            git_context,
                        });
                    }
                }
            }
        }

        // Sort by attached status, then name, then window label so the rows
        // for a multi-claude session stay grouped in a stable order.
        sessions.sort_by(|a, b| {
            b.attached
                .cmp(&a.attached)
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.window_label.cmp(&b.window_label))
        });

        Ok(sessions)
    }

    /// List all panes in a session, across every window
    pub fn list_panes(session: &str) -> Result<Vec<Pane>> {
        let output = Command::new("tmux")
            .args([
                "list-panes",
                "-s",
                "-t",
                session,
                "-F",
                "#{pane_id}|||#{pane_current_command}|||#{pane_current_path}|||#{window_index}|||#{window_name}|||#{@claude-tmux-exclude}|||#{pane_pid}",
            ])
            .output()
            .context("Failed to execute tmux list-panes")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut panes = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split("|||").collect();
            if parts.len() >= 5 {
                let excluded = parts.get(5).map_or(false, |v| !v.is_empty() && *v != "0");
                let pid = parts.get(6).and_then(|s| s.parse::<u32>().ok());
                panes.push(Pane {
                    id: parts[0].to_string(),
                    current_command: parts[1].to_string(),
                    current_path: PathBuf::from(parts[2]),
                    window_index: parts[3].to_string(),
                    window_name: parts[4].to_string(),
                    excluded,
                    pid,
                });
            }
        }

        Ok(panes)
    }

    /// Capture the last N lines of a pane's content
    ///
    /// If `strip_empty` is true, empty lines are filtered out before taking the last N.
    /// This is useful for status detection. For preview display, use `strip_empty: false`
    /// to preserve the visual layout.
    ///
    /// ANSI escape sequences are always included - the UI handles rendering them.
    pub fn capture_pane(pane_id: &str, lines: usize, strip_empty: bool) -> Result<String> {
        let output = Command::new("tmux")
            .args([
                "capture-pane",
                "-t",
                pane_id,
                "-p", // Print to stdout
                "-J", // Join wrapped lines
                "-e", // Include escape sequences
            ])
            .output()
            .context("Failed to capture pane")?;

        if !output.status.success() {
            anyhow::bail!("Failed to capture pane {}", pane_id);
        }

        let content = String::from_utf8_lossy(&output.stdout);

        if strip_empty {
            // Filter out empty lines, then get last N (for status detection)
            let non_empty: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
            let start = non_empty.len().saturating_sub(lines);
            let last_lines = &non_empty[start..];
            Ok(last_lines.join("\n"))
        } else {
            // Preserve internal empty lines but trim trailing ones (for preview display)
            let all_lines: Vec<&str> = content.lines().collect();

            // Find last non-empty line
            let last_non_empty = all_lines
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .map(|i| i + 1)
                .unwrap_or(0);

            let trimmed = &all_lines[..last_non_empty];
            let start = trimmed.len().saturating_sub(lines);
            let last_lines = &trimmed[start..];
            Ok(last_lines.join("\n"))
        }
    }

    /// Switch the current client to the specified session
    pub fn switch_to_session(session: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["switch-client", "-t", session])
            .status()
            .context("Failed to switch session")?;

        if !status.success() {
            anyhow::bail!("Failed to switch to session {}", session);
        }

        Ok(())
    }

    /// Create a new tmux session
    pub fn new_session(name: &str, path: &std::path::Path, start_claude: bool) -> Result<()> {
        let path_str = path.to_string_lossy();

        let status = Command::new("tmux")
            .args(["new-session", "-d", "-s", name, "-c", &path_str])
            .status()
            .context("Failed to create new session")?;

        if !status.success() {
            anyhow::bail!("Failed to create session {}", name);
        }

        if start_claude {
            // Send claude command to the new session
            let _ = Command::new("tmux")
                .args(["send-keys", "-t", name, "claude", "Enter"])
                .status();
        }

        Ok(())
    }

    /// Kill a tmux session
    pub fn kill_session(session: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["kill-session", "-t", session])
            .status()
            .context("Failed to kill session")?;

        if !status.success() {
            anyhow::bail!("Failed to kill session {}", session);
        }

        Ok(())
    }

    /// Rename a tmux session
    pub fn rename_session(old_name: &str, new_name: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["rename-session", "-t", old_name, new_name])
            .status()
            .context("Failed to rename session")?;

        if !status.success() {
            anyhow::bail!("Failed to rename session {} to {}", old_name, new_name);
        }

        Ok(())
    }

    /// Get the first pane ID of a session
    pub fn first_pane_id(session: &str) -> Result<Option<String>> {
        let panes = Self::list_panes(session)?;
        Ok(panes.first().map(|p| p.id.clone()))
    }

    /// Send keys to a tmux session/pane
    pub fn send_keys(target: &str, keys: &[&str]) -> Result<()> {
        let mut args = vec!["send-keys", "-t", target];
        args.extend_from_slice(keys);
        let status = Command::new("tmux")
            .args(&args)
            .status()
            .context("Failed to send keys")?;

        if !status.success() {
            anyhow::bail!("Failed to send keys to {}", target);
        }

        Ok(())
    }

    /// List all windows in a session with their layout strings
    pub fn list_windows(session: &str) -> Result<Vec<WindowInfo>> {
        let output = Command::new("tmux")
            .args([
                "list-windows",
                "-t",
                session,
                "-F",
                "#{window_index}|||#{window_name}|||#{window_layout}",
            ])
            .output()
            .context("Failed to execute tmux list-windows")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split("|||").collect();
            if parts.len() >= 3 {
                windows.push(WindowInfo {
                    index: parts[0].to_string(),
                    name: parts[1].to_string(),
                    layout: parts[2].to_string(),
                });
            }
        }

        Ok(windows)
    }

    /// Create a new window in an existing session
    pub fn new_window(session: &str, name: &str, path: &std::path::Path) -> Result<()> {
        let path_str = path.to_string_lossy();

        let status = Command::new("tmux")
            .args(["new-window", "-t", session, "-n", name, "-c", &path_str])
            .status()
            .context("Failed to create new window")?;

        if !status.success() {
            anyhow::bail!("Failed to create window {} in session {}", name, session);
        }

        Ok(())
    }

    /// Split a pane and return the new pane ID
    pub fn split_pane(target: &str, path: &std::path::Path) -> Result<String> {
        let path_str = path.to_string_lossy();

        let output = Command::new("tmux")
            .args([
                "split-window",
                "-t",
                target,
                "-c",
                &path_str,
                "-P",
                "-F",
                "#{pane_id}",
            ])
            .output()
            .context("Failed to split pane")?;

        if !output.status.success() {
            anyhow::bail!("Failed to split pane at {}", target);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Apply a saved layout string to a window
    pub fn select_layout(target: &str, layout: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["select-layout", "-t", target, layout])
            .status()
            .context("Failed to select layout")?;

        if !status.success() {
            anyhow::bail!("Failed to apply layout to {}", target);
        }

        Ok(())
    }

    /// Rename a window
    pub fn rename_window(target: &str, new_name: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["rename-window", "-t", target, new_name])
            .status()
            .context("Failed to rename window")?;

        if !status.success() {
            anyhow::bail!("Failed to rename window at {}", target);
        }

        Ok(())
    }

    /// Get the name of the currently attached session
    pub fn current_session() -> Result<Option<String>> {
        let output = Command::new("tmux")
            .args(["display-message", "-p", "#{session_name}"])
            .output()
            .context("Failed to get current session")?;

        if !output.status.success() {
            return Ok(None);
        }

        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(name))
        }
    }
}
