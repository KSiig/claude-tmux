use std::collections::HashSet;

use crate::session::ClaudeCodeStatus;

const HOOK_DIR: &str = "/tmp/claude-tmux-hooks";

/// Read the hook-written status file for a pane.
/// Returns the status and the unix timestamp when it was written.
pub fn read_hook_status(pane_id: &str) -> Option<(ClaudeCodeStatus, u64)> {
    let path = format!("{}/{}", HOOK_DIR, pane_id);
    let content = std::fs::read_to_string(path).ok()?;
    let mut parts = content.trim().splitn(2, ' ');
    let status_str = parts.next()?;
    let timestamp: u64 = parts.next()?.parse().ok()?;

    let status = match status_str {
        "working" => ClaudeCodeStatus::Working,
        "idle" => ClaudeCodeStatus::Idle,
        "waiting_input" => ClaudeCodeStatus::WaitingInput,
        "error" => ClaudeCodeStatus::Error,
        _ => return None,
    };

    Some((status, timestamp))
}

/// Remove hook files for panes that no longer exist.
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
        write_hook_file(pane, "working 1717500000\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, Some((ClaudeCodeStatus::Working, 1717500000)));
    }

    #[test]
    fn test_read_hook_status_idle() {
        let pane = "%99_test_idle";
        write_hook_file(pane, "idle 1717500001\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, Some((ClaudeCodeStatus::Idle, 1717500001)));
    }

    #[test]
    fn test_read_hook_status_waiting() {
        let pane = "%99_test_waiting";
        write_hook_file(pane, "waiting_input 1717500002\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, Some((ClaudeCodeStatus::WaitingInput, 1717500002)));
    }

    #[test]
    fn test_read_hook_status_error() {
        let pane = "%99_test_error";
        write_hook_file(pane, "error 1717500003\n");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, Some((ClaudeCodeStatus::Error, 1717500003)));
    }

    #[test]
    fn test_read_hook_status_missing_file() {
        assert_eq!(read_hook_status("%99_nonexistent"), None);
    }

    #[test]
    fn test_read_hook_status_invalid_content() {
        let pane = "%99_test_invalid";
        write_hook_file(pane, "garbage");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_hook_status_unknown_status() {
        let pane = "%99_test_unknown";
        write_hook_file(pane, "bogus 1717500000");
        let result = read_hook_status(pane);
        remove_hook_file(pane);
        assert_eq!(result, None);
    }

    #[test]
    fn test_cleanup_hook_files() {
        let live = "%99_test_live";
        let stale = "%99_test_stale";
        write_hook_file(live, "working 1717500000");
        write_hook_file(stale, "idle 1717500000");

        let live_set: HashSet<&str> = [live].into_iter().collect();
        cleanup_hook_files(&live_set);

        assert!(std::path::Path::new(&format!("{}/{}", HOOK_DIR, live)).exists());
        assert!(!std::path::Path::new(&format!("{}/{}", HOOK_DIR, stale)).exists());

        remove_hook_file(live);
    }
}
