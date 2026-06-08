use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

const HOOK_DIR: &str = "/tmp/claude-tmux-hooks";
const RING_SIZE: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Unknown,
    Working,
    Idle,
    WaitingInput,
}

impl Status {
    fn as_str(&self) -> &'static str {
        match self {
            Status::Unknown => "unknown",
            Status::Working => "working",
            Status::Idle => "idle",
            Status::WaitingInput => "waiting_input",
        }
    }
}

struct RingBuffer {
    buf: Vec<u8>,
    pos: usize,
    len: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0; capacity],
            pos: 0,
            len: 0,
        }
    }

    fn push(&mut self, data: &[u8]) {
        for &byte in data {
            self.buf[self.pos] = byte;
            self.pos = (self.pos + 1) % self.buf.len();
            if self.len < self.buf.len() {
                self.len += 1;
            }
        }
    }

    fn as_string(&self) -> String {
        let cap = self.buf.len();
        let mut result = Vec::with_capacity(self.len);
        if self.len < cap {
            result.extend_from_slice(&self.buf[..self.len]);
        } else {
            result.extend_from_slice(&self.buf[self.pos..]);
            result.extend_from_slice(&self.buf[..self.pos]);
        }
        String::from_utf8_lossy(&result).to_string()
    }
}

use crate::detection::content::strip_ansi;

fn detect_from_stream_content(content: &str) -> Status {
    if content.contains("[y/n]")
        || content.contains("[Y/n]")
        || content.contains("shift+tab to approve")
        || content.contains("Esc to cancel")
    {
        return Status::WaitingInput;
    }

    if content.contains("Effecting\u{2026}") {
        return Status::Working;
    }

    if content.contains("ctrl+c") && content.contains("to interrupt") {
        return Status::Working;
    }

    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains('❯') && i > 0 && lines[i - 1].contains('─') {
            return Status::Idle;
        }
    }

    Status::Unknown
}

fn write_status(pane_id: &str, status: Status) -> Result<()> {
    let _ = std::fs::create_dir_all(HOOK_DIR);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let content = format!("{} {}\n", status.as_str(), ts);
    std::fs::write(format!("{}/{}", HOOK_DIR, pane_id), content)
        .context("failed to write sidecar status")?;
    Ok(())
}

fn normalize_pane_id(raw: &str) -> String {
    if raw.starts_with('%') {
        raw.to_string()
    } else {
        format!("%{}", raw)
    }
}

fn initial_capture(pane_id: &str) -> Option<String> {
    let output = std::process::Command::new("tmux")
        .args(["capture-pane", "-t", pane_id, "-p", "-J", "-e"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let content = String::from_utf8_lossy(&output.stdout);
    let non_empty: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = non_empty.len().saturating_sub(15);
    Some(non_empty[start..].join("\n"))
}

pub fn run_monitor(pane_id: &str) -> Result<()> {
    let pane_id = normalize_pane_id(pane_id);

    // Establish baseline status from current pane content before entering
    // the stream loop. pipe-pane only forwards new output, so without this
    // an already-idle session would stay at Unknown indefinitely.
    let mut current_status = Status::Unknown;
    if let Some(content) = initial_capture(&pane_id) {
        let clean = strip_ansi(&content);
        let initial = detect_from_stream_content(&clean);
        if initial != Status::Unknown {
            current_status = initial;
            let _ = write_status(&pane_id, current_status);
        }
    }

    let mut ring = RingBuffer::new(RING_SIZE);
    let mut buf = [0u8; 4096];
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    loop {
        let n = handle.read(&mut buf)?;
        if n == 0 {
            break;
        }

        ring.push(&buf[..n]);
        let raw = ring.as_string();
        let clean = strip_ansi(&raw);
        let new_status = detect_from_stream_content(&clean);

        if new_status != Status::Unknown && new_status != current_status {
            current_status = new_status;
            let _ = write_status(&pane_id, current_status);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut ring = RingBuffer::new(10);
        ring.push(b"hello");
        assert_eq!(ring.as_string(), "hello");
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut ring = RingBuffer::new(5);
        ring.push(b"hello world");
        assert_eq!(ring.as_string(), "world");
    }

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\x1b[32mhello\x1b[0m"), "hello");
        assert_eq!(strip_ansi("no escapes"), "no escapes");
    }

    #[test]
    fn test_detect_working() {
        assert_eq!(
            detect_from_stream_content("some output (ctrl+c to interrupt)"),
            Status::Working
        );
    }

    #[test]
    fn test_detect_idle() {
        assert_eq!(
            detect_from_stream_content("● Done\n─────\n❯ "),
            Status::Idle
        );
    }

    #[test]
    fn test_detect_waiting() {
        assert_eq!(
            detect_from_stream_content("Delete files? [y/n]"),
            Status::WaitingInput
        );
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(
            detect_from_stream_content("random text"),
            Status::Unknown
        );
    }
}
