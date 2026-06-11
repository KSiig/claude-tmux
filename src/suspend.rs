use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::session::{ClaudeCodeStatus, Session};

pub struct SuspendManager {
    suspended: HashMap<String, u32>,
    idle_since: HashMap<String, Instant>,
    grace: Duration,
}

impl SuspendManager {
    pub fn new(grace: Duration) -> Self {
        Self {
            suspended: HashMap::new(),
            idle_since: HashMap::new(),
            grace,
        }
    }

    pub fn tick(&mut self, sessions: &[Session], focused_session: Option<&str>) {
        let mut should_suspend: HashMap<String, u32> = HashMap::new();
        let mut should_resume: Vec<String> = Vec::new();

        for session in sessions {
            let Some(pane_id) = &session.claude_code_pane else {
                continue;
            };
            let Some(pane) = session.panes.iter().find(|p| p.id == *pane_id) else {
                continue;
            };
            let Some(shell_pid) = pane.pid else {
                continue;
            };

            let is_focused = focused_session.map_or(false, |f| f == session.name);
            let is_suspendable = matches!(
                session.claude_code_status,
                ClaudeCodeStatus::Idle | ClaudeCodeStatus::Done
            );

            if !is_focused && is_suspendable {
                let idle_start = self.idle_since.entry(pane_id.clone()).or_insert_with(Instant::now);
                if idle_start.elapsed() >= self.grace {
                    should_suspend.insert(pane_id.clone(), shell_pid);
                }
            } else {
                self.idle_since.remove(pane_id);
                if self.suspended.contains_key(pane_id) {
                    should_resume.push(pane_id.clone());
                }
            }
        }

        for pane_id in &should_resume {
            if let Some(shell_pid) = self.suspended.remove(pane_id) {
                resume_children(shell_pid);
            }
        }

        for (pane_id, shell_pid) in &should_suspend {
            if !self.suspended.contains_key(pane_id) {
                suspend_children(*shell_pid);
                self.suspended.insert(pane_id.clone(), *shell_pid);
            }
        }

        self.suspended.retain(|pane_id, _| {
            sessions.iter().any(|s| s.claude_code_pane.as_ref() == Some(pane_id))
        });
        self.idle_since.retain(|pane_id, _| {
            sessions.iter().any(|s| s.claude_code_pane.as_ref() == Some(pane_id))
        });
    }

    pub fn resume_all(&mut self) {
        for (_, shell_pid) in self.suspended.drain() {
            resume_children(shell_pid);
        }
    }
}

fn suspend_children(shell_pid: u32) {
    let _ = Command::new("pkill")
        .args(["-STOP", "-P", &shell_pid.to_string()])
        .status();
}

fn resume_children(shell_pid: u32) {
    let _ = Command::new("pkill")
        .args(["-CONT", "-P", &shell_pid.to_string()])
        .status();
}
