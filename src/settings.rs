use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

fn default_status_interval_ms() -> u64 {
    500
}

fn default_task_poll_interval_ms() -> u64 {
    10_000
}

fn default_true() -> bool {
    true
}

fn default_hook_override_delay_ms() -> u64 {
    5000
}

#[derive(Deserialize)]
struct TaskIntegrationFile {
    provider: String,
    #[serde(default)]
    issue_prefix: Option<String>,
    #[serde(default = "default_task_poll_interval_ms")]
    poll_interval_ms: u64,
    #[serde(default = "default_true")]
    show_titles: bool,
    #[serde(default = "default_true")]
    show_status: bool,
    #[serde(default)]
    status_labels: bool,
}

#[derive(Deserialize)]
struct SettingsFile {
    #[serde(default = "default_status_interval_ms")]
    status_interval_ms: u64,
    #[serde(default = "default_true")]
    show_git_info: bool,
    #[serde(default = "default_hook_override_delay_ms")]
    hook_override_delay_ms: u64,
    #[serde(default = "default_true")]
    session_status_labels: bool,
    #[serde(default)]
    grouping: bool,
    #[serde(default)]
    task_integration: Option<TaskIntegrationFile>,
}

pub struct TaskIntegration {
    pub provider: String,
    pub issue_prefix: Option<String>,
    pub poll_interval: Duration,
    pub show_titles: bool,
    pub show_status: bool,
    pub status_labels: bool,
}

pub struct Settings {
    pub status_interval: Duration,
    pub show_git_info: bool,
    pub hook_override_delay: Duration,
    pub session_status_labels: bool,
    pub grouping: bool,
    pub task_integration: Option<TaskIntegration>,
}

impl Settings {
    pub fn load() -> Self {
        let user_path = Self::user_path();
        let repo_path = Self::repo_path();

        let file = user_path
            .and_then(|p| Self::parse_file(&p))
            .or_else(|| repo_path.and_then(|p| Self::parse_file(&p)));

        match file {
            Some(f) => Settings {
                status_interval: Duration::from_millis(f.status_interval_ms.max(20)),
                show_git_info: f.show_git_info,
                hook_override_delay: Duration::from_millis(f.hook_override_delay_ms),
                session_status_labels: f.session_status_labels,
                grouping: f.grouping,
                task_integration: f.task_integration.map(|t| TaskIntegration {
                    provider: t.provider,
                    issue_prefix: t.issue_prefix,
                    poll_interval: Duration::from_millis(t.poll_interval_ms.max(1000)),
                    show_titles: t.show_titles,
                    show_status: t.show_status,
                    status_labels: t.status_labels,
                }),
            },
            None => Settings {
                status_interval: Duration::from_millis(default_status_interval_ms()),
                show_git_info: true,
                hook_override_delay: Duration::from_millis(default_hook_override_delay_ms()),
                session_status_labels: true,
                grouping: false,
                task_integration: None,
            },
        }
    }

    fn parse_file(path: &Path) -> Option<SettingsFile> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
    }

    fn user_path() -> Option<PathBuf> {
        dirs::home_dir().map(|d| d.join(".claude-tmux").join("settings.json"))
    }

    fn repo_path() -> Option<PathBuf> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("settings.json");
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }
}
