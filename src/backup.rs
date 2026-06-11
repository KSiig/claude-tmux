use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::detection::content::detect_from_content;
use crate::session::ClaudeCodeStatus;
use crate::settings::Settings;
use crate::tmux::Tmux;

const BACKUP_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupFile {
    pub version: u32,
    pub timestamp: i64,
    pub sessions: Vec<BackupSession>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupSession {
    pub name: String,
    pub windows: Vec<BackupWindow>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupWindow {
    pub index: String,
    pub name: String,
    pub layout: String,
    pub panes: Vec<BackupPane>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BackupPane {
    pub path: PathBuf,
    pub had_claude: bool,
}

fn backup_path() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".claude-tmux").join("backup.json"))
        .context("could not determine home directory")
}

fn session_names_from_tmux() -> Result<Vec<String>> {
    let output = std::process::Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .context("Failed to execute tmux list-sessions")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no server running") || stderr.contains("no sessions") {
            return Ok(Vec::new());
        }
        anyhow::bail!("tmux list-sessions failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect())
}

pub fn capture_backup(
    settings: &Settings,
    rename_sessions: bool,
    renamed_panes: &mut HashSet<String>,
) -> Result<BackupFile> {
    let session_names = session_names_from_tmux()?;

    let excluded: HashSet<&str> = settings
        .exclude_sessions
        .iter()
        .map(|s| s.as_str())
        .collect();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut sessions = Vec::new();

    for name in &session_names {
        if excluded.iter().any(|pat| crate::settings::glob_match(pat, name)) {
            continue;
        }

        let windows = Tmux::list_windows(name).unwrap_or_default();
        let panes = Tmux::list_panes(name).unwrap_or_default();

        let mut backup_windows = Vec::new();

        for window in &windows {
            let window_panes: Vec<_> = panes
                .iter()
                .filter(|p| p.window_index == window.index)
                .collect();

            let mut backup_panes = Vec::new();

            for pane in &window_panes {
                let is_claude =
                    pane.current_command == "claude" || pane.current_command.contains("claude");

                if rename_sessions && is_claude && !renamed_panes.contains(&pane.id) {
                    if let Ok(content) = Tmux::capture_pane(&pane.id, 15, true) {
                        let status = detect_from_content(&content);
                        if status == ClaudeCodeStatus::Idle {
                            let rename_cmd = format!("/rename {}", name);
                            let _ = Tmux::send_keys(&pane.id, &[&rename_cmd, "Enter"]);
                            renamed_panes.insert(pane.id.clone());
                        }
                    }
                }

                backup_panes.push(BackupPane {
                    path: pane.current_path.clone(),
                    had_claude: is_claude,
                });
            }

            backup_windows.push(BackupWindow {
                index: window.index.clone(),
                name: window.name.clone(),
                layout: window.layout.clone(),
                panes: backup_panes,
            });
        }

        sessions.push(BackupSession {
            name: name.clone(),
            windows: backup_windows,
        });
    }

    Ok(BackupFile {
        version: BACKUP_VERSION,
        timestamp,
        sessions,
    })
}

pub fn save_backup(backup: &BackupFile) -> Result<()> {
    let path = backup_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(backup)?;
    std::fs::write(&path, json)?;
    Ok(())
}

pub fn capture_and_save(
    settings: &Settings,
    rename_sessions: bool,
    renamed_panes: &mut HashSet<String>,
) -> Result<()> {
    let backup = capture_backup(settings, rename_sessions, renamed_panes)?;
    save_backup(&backup)
}

pub fn run_backup() -> Result<()> {
    let settings = Settings::load();
    let mut renamed = HashSet::new();
    let backup = capture_backup(&settings, settings.backup_rename_sessions, &mut renamed)?;
    save_backup(&backup)?;
    println!(
        "Backed up {} sessions to {}",
        backup.sessions.len(),
        backup_path()?.display()
    );
    Ok(())
}

pub fn run_restore() -> Result<()> {
    let path = backup_path()?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("No backup file found at {}", path.display()))?;
    let backup: BackupFile =
        serde_json::from_str(&content).context("Failed to parse backup file")?;

    if backup.version > BACKUP_VERSION {
        eprintln!(
            "Warning: backup file version {} is newer than supported version {}",
            backup.version, BACKUP_VERSION
        );
    }

    let existing = session_names_from_tmux().unwrap_or_default();
    let existing_set: HashSet<&str> = existing.iter().map(|s| s.as_str()).collect();

    let mut restored = 0;
    let mut skipped = 0;

    for session in &backup.sessions {
        if existing_set.contains(session.name.as_str()) {
            eprintln!("Skipping '{}': session already exists", session.name);
            skipped += 1;
            continue;
        }

        if session.windows.is_empty() {
            continue;
        }

        let first_window = &session.windows[0];
        let first_path = first_window
            .panes
            .first()
            .map(|p| p.path.clone())
            .unwrap_or_else(|| PathBuf::from(dirs::home_dir().unwrap_or_default()));

        Tmux::new_session(&session.name, &first_path, false)?;

        if first_window.name != "0" && !first_window.name.is_empty() {
            let target = format!("{}:0", session.name);
            let _ = Tmux::rename_window(&target, &first_window.name);
        }

        restore_panes_in_window(&session.name, "0", first_window)?;

        for window in session.windows.iter().skip(1) {
            let window_path = window
                .panes
                .first()
                .map(|p| p.path.clone())
                .unwrap_or_else(|| PathBuf::from(dirs::home_dir().unwrap_or_default()));

            Tmux::new_window(&session.name, &window.name, &window_path)?;

            let target = format!("{}:{}", session.name, window.name);
            restore_panes_in_window(&session.name, &window.name, window)?;

            let _ = Tmux::select_layout(&target, &window.layout);
        }

        // Apply layout for first window after all panes are created
        if !first_window.layout.is_empty() {
            let target = format!("{}:0", session.name);
            let _ = Tmux::select_layout(&target, &first_window.layout);
        }

        // Start Claude in panes that had it
        start_claude_in_session(&session.name, &backup)?;

        restored += 1;
    }

    println!("Restored {} sessions ({} skipped)", restored, skipped);
    Ok(())
}

fn restore_panes_in_window(
    session: &str,
    window_target: &str,
    window: &BackupWindow,
) -> Result<()> {
    // First pane already exists (created by new_session or new_window)
    for pane in window.panes.iter().skip(1) {
        let target = format!("{}:{}", session, window_target);
        let _ = Tmux::split_pane(&target, &pane.path);
    }
    Ok(())
}

fn start_claude_in_session(session: &str, backup: &BackupFile) -> Result<()> {
    let backup_session = backup
        .sessions
        .iter()
        .find(|s| s.name == session)
        .context("session not found in backup")?;

    let panes = Tmux::list_panes(session).unwrap_or_default();

    let mut pane_idx = 0;
    for window in &backup_session.windows {
        for backup_pane in &window.panes {
            if backup_pane.had_claude {
                if let Some(real_pane) = panes.get(pane_idx) {
                    let _ = Tmux::send_keys(&real_pane.id, &["claude --continue", "Enter"]);
                }
            }
            pane_idx += 1;
        }
    }

    Ok(())
}
