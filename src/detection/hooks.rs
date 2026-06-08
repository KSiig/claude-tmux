use std::collections::HashSet;

use crate::session::ClaudeCodeStatus;
use super::{DetectionBackend, DetectionContext};

const HOOK_DIR: &str = "/tmp/claude-tmux-hooks";

pub struct HooksBackend;

impl HooksBackend {
    pub fn new(_staleness_secs: u64) -> Self {
        Self
    }
}

impl DetectionBackend for HooksBackend {
    fn needs_content(&self) -> bool {
        false
    }

    fn detect(&mut self, pane_id: &str, _ctx: &DetectionContext) -> ClaudeCodeStatus {
        match read_hook_status(pane_id) {
            ClaudeCodeStatus::Unknown => ClaudeCodeStatus::Idle,
            status => status,
        }
    }
}

fn read_hook_status(pane_id: &str) -> ClaudeCodeStatus {
    let path = format!("{}/{}", HOOK_DIR, pane_id);
    let Ok(content) = std::fs::read_to_string(path) else {
        return ClaudeCodeStatus::Unknown;
    };

    let status_str = content.trim().split(' ').next().unwrap_or("");

    match status_str {
        "working" => ClaudeCodeStatus::Working,
        "idle" => ClaudeCodeStatus::Idle,
        "waiting_input" => ClaudeCodeStatus::WaitingInput,
        "error" => ClaudeCodeStatus::Error,
        _ => ClaudeCodeStatus::Unknown,
    }
}

pub fn cleanup_hook_files(live_panes: &HashSet<&str>) {
    let Ok(entries) = std::fs::read_dir(HOOK_DIR) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !live_panes.contains(name_str) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_hook_dir() {
        let _ = fs::create_dir_all(HOOK_DIR);
    }

    fn write_hook_file(pane_id: &str, content: &str) {
        setup_hook_dir();
        fs::write(format!("{}/{}", HOOK_DIR, pane_id), content).unwrap();
    }

    fn remove_hook_file(pane_id: &str) {
        let _ = fs::remove_file(format!("{}/{}", HOOK_DIR, pane_id));
    }

    #[test]
    fn test_read_hook_status_working() {
        let pane = "%99_test_working";
        write_hook_file(pane, "working 1780000000\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, ClaudeCodeStatus::Working);
    }

    #[test]
    fn test_read_hook_status_idle() {
        let pane = "%99_test_idle";
        write_hook_file(pane, "idle 1780000000\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, ClaudeCodeStatus::Idle);
    }

    #[test]
    fn test_read_hook_status_waiting() {
        let pane = "%99_test_waiting";
        write_hook_file(pane, "waiting_input 1780000000\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, ClaudeCodeStatus::WaitingInput);
    }

    #[test]
    fn test_read_hook_status_error() {
        let pane = "%99_test_error";
        write_hook_file(pane, "error 1780000000\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, ClaudeCodeStatus::Error);
    }

    #[test]
    fn test_old_hook_still_trusted() {
        let pane = "%99_test_old";
        write_hook_file(pane, "working 1000000000\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, ClaudeCodeStatus::Working);
    }

    #[test]
    fn test_missing_file_returns_unknown() {
        let result = read_hook_status("%99_nonexistent");
        assert_eq!(result, ClaudeCodeStatus::Unknown);
    }

    #[test]
    fn test_invalid_content_returns_unknown() {
        let pane = "%99_test_invalid";
        write_hook_file(pane, "garbage");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, ClaudeCodeStatus::Unknown);
    }

    #[test]
    fn test_status_without_timestamp() {
        let pane = "%99_test_no_ts";
        write_hook_file(pane, "idle");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, ClaudeCodeStatus::Idle);
    }

    #[test]
    fn test_cleanup_hook_files() {
        let live = "%99_test_live";
        let dead = "%99_test_dead_cleanup";
        write_hook_file(live, "working 1780000000");
        write_hook_file(dead, "idle 1780000000");

        let live_set: HashSet<&str> = [live].into_iter().collect();
        cleanup_hook_files(&live_set);

        assert!(std::path::Path::new(&format!("{}/{}", HOOK_DIR, live)).exists());
        assert!(!std::path::Path::new(&format!("{}/{}", HOOK_DIR, dead)).exists());

        remove_hook_file(live);
    }
}
