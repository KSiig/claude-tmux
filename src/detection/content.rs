use crate::session::ClaudeCodeStatus;

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc.is_ascii_alphabetic() || nc == 'm' || nc == 'J' || nc == 'K' || nc == 'H' {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn has_input_prompt(content: &str) -> bool {
    content.contains("[y/n]")
        || content.contains("[Y/n]")
        || content.contains("shift+tab to approve")
        || content.contains("Esc to cancel")
}

pub fn has_input_field(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains('❯') {
            if i > 0 && lines[i - 1].contains('─') {
                return true;
            }
        }
    }
    false
}

fn is_status_line(line: &str) -> bool {
    let trimmed = line.trim();
    let after_bullet = trimmed
        .trim_start_matches('\u{00b7}')  // ·
        .trim_start_matches('\u{2726}')  // ✦
        .trim_start_matches('\u{2722}')  // ✢
        .trim();
    after_bullet.contains("\u{2026}") && after_bullet.contains("\u{2191}") && after_bullet.contains("tokens)")
}

fn is_active_tool_phase(content: &str) -> bool {
    content.lines().any(|line| is_status_line(line))
}

pub fn detect_from_content(content: &str) -> ClaudeCodeStatus {
    let content = &strip_ansi(content);
    if has_input_prompt(content) {
        return ClaudeCodeStatus::WaitingInput;
    }
    if is_active_tool_phase(content) {
        return ClaudeCodeStatus::Working;
    }
    if has_input_field(content) {
        if content.contains("ctrl+c") && content.contains("to interrupt") {
            return ClaudeCodeStatus::Working;
        }
        return ClaudeCodeStatus::Idle;
    }
    if content.contains("ctrl+c") && content.contains("to interrupt") {
        return ClaudeCodeStatus::Working;
    }
    ClaudeCodeStatus::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working() {
        let content = "* (ctrl+c to interrupt)\n─────\n❯ hello";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Working);
    }

    #[test]
    fn test_idle() {
        let content = "● Done\n─────\n❯ hello";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Idle);
    }

    #[test]
    fn test_no_border_above_prompt() {
        let content = "─────\nsome text\n❯ hello";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Unknown);
    }

    #[test]
    fn test_waiting_input() {
        let content = "Delete files? [y/n]";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::WaitingInput);
    }

    #[test]
    fn test_waiting_input_plan_approval() {
        let content = "Would you like to proceed?\n\n❯ 1. Yes, and use auto mode\n  2. Yes, manually approve edits\n     shift+tab to approve with this feedback";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::WaitingInput);
    }

    #[test]
    fn test_effecting_phase_is_working() {
        let content = "⏺ Running 1 shell command…\n  ⎿  $ git rebase --continue\n\n✢ Effecting\u{2026} (34s · ↑ 1.1k tokens)\n\n─────\n❯ ";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Working);
    }

    #[test]
    fn test_active_tool_with_checklist() {
        let content = "✢ Deploying to server\u{2026} (5m 27s · ↑ 14.1k tokens)\n  ⎿  ◼ Deploy\n─────\n❯ ";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Working);
    }

    #[test]
    fn test_sketching_phase_is_working() {
        let content = "· Sketching\u{2026} (10m 31s · \u{2191} 19.6k tokens)\n  \u{23bf}  Tip: Use /voice\n─────\n❯ ";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Working);
    }

    #[test]
    fn test_ansi_codes_stripped_before_detection() {
        let content = "\x1b[38;5;174mSketchi\x1b[38;5;216mng\u{2026}\x1b[38;5;174m \x1b[38;5;246m(9m 59s · \u{2191} \x1b[39m \x1b[38;5;246m18.2k tokens)\x1b[39m\n\x1b[38;5;37m─────\n\x1b[38;5;246m❯\x1b[39m ";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Working);
    }

    #[test]
    fn test_prose_mentioning_tokens_not_working() {
        let content = "Added detection for any line containing \u{2026} with tokens) \u{2014} this covers Deploying, Reading, Running, etc.\n─────\n❯ ";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Idle);
    }

    #[test]
    fn test_unknown() {
        let content = "random stuff";
        assert_eq!(detect_from_content(content), ClaudeCodeStatus::Unknown);
    }

    #[test]
    fn test_strip_ansi_basic() {
        assert_eq!(strip_ansi("\x1b[32mhello\x1b[0m"), "hello");
        assert_eq!(strip_ansi("no escapes"), "no escapes");
        assert_eq!(strip_ansi("\x1b[38;5;174mSketchi\x1b[38;5;216mng"), "Sketching");
    }
}
