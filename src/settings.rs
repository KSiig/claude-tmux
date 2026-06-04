use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

fn default_status_interval_ms() -> u64 {
    500
}

fn default_true() -> bool {
    true
}

fn default_hook_override_delay_ms() -> u64 {
    5000
}

#[derive(Deserialize)]
struct SettingsFile {
    #[serde(default = "default_status_interval_ms")]
    status_interval_ms: u64,
    #[serde(default = "default_true")]
    show_git_info: bool,
    #[serde(default = "default_hook_override_delay_ms")]
    hook_override_delay_ms: u64,
}

pub struct Settings {
    pub status_interval: Duration,
    pub show_git_info: bool,
    pub hook_override_delay: Duration,
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
            },
            None => Settings {
                status_interval: Duration::from_millis(default_status_interval_ms()),
                show_git_info: true,
                hook_override_delay: Duration::from_millis(default_hook_override_delay_ms()),
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
