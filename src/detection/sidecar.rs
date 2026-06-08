use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::session::ClaudeCodeStatus;
use super::{DetectionBackend, DetectionContext};

const HOOK_DIR: &str = "/tmp/claude-tmux-hooks";

pub struct SidecarBackend {
    staleness_threshold: Duration,
}

impl SidecarBackend {
    pub fn new(staleness_secs: u64) -> Self {
        Self {
            staleness_threshold: Duration::from_secs(staleness_secs),
        }
    }

    pub fn is_sidecar_running(pane_id: &str) -> bool {
        let path = format!("{}/{}", HOOK_DIR, pane_id);
        if let Ok(metadata) = std::fs::metadata(&path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    return elapsed < Duration::from_secs(30);
                }
            }
        }
        false
    }

    pub fn start_sidecar_for_pane(pane_id: &str) {
        let binary = match std::env::current_exe() {
            Ok(b) => b,
            Err(_) => return,
        };
        // tmux pipe-pane passes the command through sh -c. The % in pane IDs
        // (e.g. %361) can be consumed by the shell or tmux's parser. Pass the
        // raw numeric ID and let the monitor normalize it.
        let raw_id = pane_id.strip_prefix('%').unwrap_or(pane_id);
        let cmd = format!("{} monitor --pane {}", binary.display(), raw_id);
        let _ = std::process::Command::new("tmux")
            .args(["pipe-pane", "-t", pane_id, "-o", &cmd])
            .output();
    }
}

impl DetectionBackend for SidecarBackend {
    fn detect(&mut self, pane_id: &str, _ctx: &DetectionContext) -> ClaudeCodeStatus {
        let path = format!("{}/{}", HOOK_DIR, pane_id);
        let Ok(content) = std::fs::read_to_string(path) else {
            return ClaudeCodeStatus::Unknown;
        };

        let mut parts = content.trim().splitn(2, ' ');
        let Some(status_str) = parts.next() else {
            return ClaudeCodeStatus::Unknown;
        };
        let Some(timestamp) = parts.next().and_then(|s| s.parse::<u64>().ok()) else {
            return ClaudeCodeStatus::Unknown;
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if now.saturating_sub(timestamp) > self.staleness_threshold.as_secs() {
            return ClaudeCodeStatus::Unknown;
        }

        match status_str {
            "working" => ClaudeCodeStatus::Working,
            "idle" => ClaudeCodeStatus::Idle,
            "waiting_input" => ClaudeCodeStatus::WaitingInput,
            "error" => ClaudeCodeStatus::Error,
            _ => ClaudeCodeStatus::Unknown,
        }
    }
}

pub fn ensure_sidecars(pane_ids: &[String], tracked: &mut HashSet<String>) {
    for pane_id in pane_ids {
        if !tracked.contains(pane_id) {
            if !SidecarBackend::is_sidecar_running(pane_id) {
                SidecarBackend::start_sidecar_for_pane(pane_id);
            }
            tracked.insert(pane_id.clone());
        }
    }
    tracked.retain(|id| pane_ids.contains(id));
}
