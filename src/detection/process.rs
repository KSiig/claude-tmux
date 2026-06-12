use std::collections::HashMap;

use crate::session::ClaudeCodeStatus;
use super::{DetectionBackend, DetectionContext};
use super::content;

#[allow(dead_code)]
struct ProcessInfo {
    pid: u32,
    ppid: u32,
    comm: String,
}

struct ProcessTree {
    by_pid: HashMap<u32, ProcessInfo>,
    children: HashMap<u32, Vec<u32>>,
}

impl ProcessTree {
    fn build() -> Option<Self> {
        let output = std::process::Command::new("ps")
            .args(["-e", "-o", "pid=,ppid=,comm="])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut by_pid = HashMap::new();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.splitn(3, char::is_whitespace);
            let pid: u32 = match parts.next().and_then(|s| s.trim().parse().ok()) {
                Some(p) => p,
                None => continue,
            };
            let ppid: u32 = match parts.next().and_then(|s| s.trim().parse().ok()) {
                Some(p) => p,
                None => continue,
            };
            let comm = parts.next().unwrap_or("").trim().to_string();

            children.entry(ppid).or_default().push(pid);
            by_pid.insert(pid, ProcessInfo { pid, ppid, comm });
        }

        Some(ProcessTree { by_pid, children })
    }

    fn descendants(&self, root: u32) -> Vec<u32> {
        let mut result = Vec::new();
        let mut stack = vec![root];
        while let Some(pid) = stack.pop() {
            if pid != root {
                result.push(pid);
            }
            if let Some(kids) = self.children.get(&pid) {
                stack.extend(kids);
            }
        }
        result
    }

    fn has_tool_children(&self, root: u32) -> bool {
        let descs = self.descendants(root);
        for pid in descs {
            if let Some(info) = self.by_pid.get(&pid) {
                let comm = info.comm.rsplit('/').next().unwrap_or(&info.comm);
                if !is_node_internal(comm) {
                    return true;
                }
            }
        }
        false
    }
}

fn is_node_internal(comm: &str) -> bool {
    comm == "node" || comm == "npm" || comm == "npx" || comm == "claude"
}

fn is_claude_process(comm: &str) -> bool {
    let base = comm.rsplit('/').next().unwrap_or(comm);
    base == "claude" || base == "node"
}

pub struct ProcessBackend {
    tree: Option<ProcessTree>,
}

impl ProcessBackend {
    pub fn new() -> Self {
        Self { tree: None }
    }

    fn find_claude_in_pane(&self, pane_pid: u32) -> Option<u32> {
        let tree = self.tree.as_ref()?;
        let descs = tree.descendants(pane_pid);

        // First pass: look for a process named "claude"
        for &pid in &descs {
            if let Some(info) = tree.by_pid.get(&pid) {
                let base = info.comm.rsplit('/').next().unwrap_or(&info.comm);
                if base == "claude" {
                    return Some(pid);
                }
            }
        }

        // Second pass: look for the first node process that's a direct child
        // of the pane shell (likely the Claude Code entry point)
        if let Some(kids) = tree.children.get(&pane_pid) {
            for &kid in kids {
                if let Some(info) = tree.by_pid.get(&kid) {
                    let base = info.comm.rsplit('/').next().unwrap_or(&info.comm);
                    if base == "node" || is_claude_process(base) {
                        return Some(kid);
                    }
                }
            }
        }

        None
    }

    fn find_all_claude_in_pane(&self, pane_pid: u32) -> Vec<u32> {
        let Some(tree) = self.tree.as_ref() else {
            return vec![];
        };
        tree.descendants(pane_pid)
            .into_iter()
            .filter(|&pid| {
                tree.by_pid
                    .get(&pid)
                    .map(|info| {
                        let base = info.comm.rsplit('/').next().unwrap_or(&info.comm);
                        base == "claude"
                    })
                    .unwrap_or(false)
            })
            .collect()
    }
}

impl DetectionBackend for ProcessBackend {
    fn tick_start(&mut self) {
        self.tree = ProcessTree::build();
    }

    fn needs_content(&self) -> bool {
        true
    }

    fn detect(&mut self, _pane_id: &str, ctx: &DetectionContext) -> ClaudeCodeStatus {
        let Some(pane_pid) = ctx.pane_pid else {
            // No PID available — fall back to content-only detection
            return ctx.pane_content
                .map(content::detect_from_content)
                .unwrap_or(ClaudeCodeStatus::Unknown);
        };

        let Some(claude_pid) = self.find_claude_in_pane(pane_pid) else {
            return ClaudeCodeStatus::Unknown;
        };

        let tree = self.tree.as_ref().unwrap();

        // Check if Claude has non-node children (tool subprocesses)
        if tree.has_tool_children(claude_pid) {
            return ClaudeCodeStatus::Working;
        }

        // No tool children — use minimal content check for finer distinction.
        // The process tree alone can't distinguish Idle from WaitingInput or
        // from the "thinking" phase (API call in flight, no local children).
        // Content parsing handles these cases with targeted string checks.
        ctx.pane_content
            .map(content::detect_from_content)
            .unwrap_or(ClaudeCodeStatus::Unknown)
    }

    fn cleanup_stale_processes(&mut self, pane_pids: &[u32]) {
        for &pane_pid in pane_pids {
            let claude_pids = self.find_all_claude_in_pane(pane_pid);
            if claude_pids.len() <= 1 {
                continue;
            }

            let tree = self.tree.as_ref().unwrap();

            let active = claude_pids
                .iter()
                .find(|&&pid| tree.has_tool_children(pid))
                .copied()
                .or_else(|| claude_pids.iter().copied().max())
                .unwrap();

            for &pid in &claude_pids {
                if pid != active {
                    let _ = std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .output();
                    eprintln!(
                        "claude-tmux: killed stale claude process {} (keeping active {})",
                        pid, active
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_node_internal() {
        assert!(is_node_internal("node"));
        assert!(is_node_internal("npm"));
        assert!(is_node_internal("npx"));
        assert!(is_node_internal("claude"));
        assert!(!is_node_internal("bash"));
        assert!(!is_node_internal("git"));
        assert!(!is_node_internal("cargo"));
    }

    #[test]
    fn test_is_claude_process() {
        assert!(is_claude_process("claude"));
        assert!(is_claude_process("/usr/local/bin/claude"));
        assert!(is_claude_process("node"));
        assert!(!is_claude_process("bash"));
    }

    #[test]
    fn test_fallback_to_content_when_no_pid() {
        let mut backend = ProcessBackend::new();
        let ctx = DetectionContext {
            pane_pid: None,
            pane_content: Some("● Done\n─────\n❯ hello"),
        };
        assert_eq!(backend.detect("%0", &ctx), ClaudeCodeStatus::Idle);
    }

    #[test]
    fn test_fallback_to_content_waiting_input() {
        let mut backend = ProcessBackend::new();
        let ctx = DetectionContext {
            pane_pid: None,
            pane_content: Some("Delete files? [y/n]"),
        };
        assert_eq!(backend.detect("%0", &ctx), ClaudeCodeStatus::WaitingInput);
    }
}
