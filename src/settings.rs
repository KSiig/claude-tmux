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

fn default_done_delay_ms() -> u64 {
    2000
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
    #[serde(default = "default_done_delay_ms")]
    done_delay_ms: u64,
    #[serde(default = "default_true")]
    session_status_labels: bool,
    #[serde(default)]
    grouping: bool,
    #[serde(default)]
    exclude_sessions: Vec<String>,
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
    pub done_delay: Duration,
    pub session_status_labels: bool,
    pub grouping: bool,
    pub exclude_sessions: Vec<String>,
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
                done_delay: Duration::from_millis(f.done_delay_ms),
                session_status_labels: f.session_status_labels,
                grouping: f.grouping,
                exclude_sessions: f.exclude_sessions,
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
                done_delay: Duration::from_millis(default_done_delay_ms()),
                session_status_labels: true,
                grouping: false,
                exclude_sessions: Vec::new(),
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

    pub fn is_session_excluded(&self, name: &str) -> bool {
        self.exclude_sessions
            .iter()
            .any(|pattern| glob_match(pattern, name))
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

pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    let (plen, tlen) = (pat.len(), txt.len());
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0);

    while ti < tlen {
        if pi < plen && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < plen && pat[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < plen && pat[pi] == '*' {
        pi += 1;
    }
    pi == plen
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn exact_match() {
        assert!(glob_match("flush", "flush"));
        assert!(!glob_match("flush", "flush2"));
    }

    #[test]
    fn wildcard_suffix() {
        assert!(glob_match("flush*", "flush"));
        assert!(glob_match("flush*", "flush:claude"));
        assert!(!glob_match("flush*", "myflush"));
    }

    #[test]
    fn wildcard_prefix() {
        assert!(glob_match("*flush", "flush"));
        assert!(glob_match("*flush", "myflush"));
        assert!(!glob_match("*flush", "flush:claude"));
    }

    #[test]
    fn wildcard_both() {
        assert!(glob_match("*flush*", "myflush:claude"));
        assert!(glob_match("*flush*", "flush"));
    }

    #[test]
    fn wildcard_middle() {
        assert!(glob_match("fl*sh", "flush"));
        assert!(glob_match("fl*sh", "flash"));
        assert!(!glob_match("fl*sh", "flush:x"));
    }
}
