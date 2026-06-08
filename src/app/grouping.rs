use std::collections::{HashMap, HashSet};

use crate::session::{ClaudeCodeStatus, Session};

pub struct SessionGroup<'a> {
    pub label: Option<String>,
    pub title: Option<String>,
    pub sessions: Vec<&'a Session>,
    /// Whether this group needs a plain separator line above it
    /// (headerless group that follows a headed group)
    pub separator: bool,
    /// When true, strip the group label prefix from session display names.
    /// Set for category-prefix groups (e.g. "skill-flush" under "skill" shows as "flush").
    pub strip_prefix: bool,
    /// Number of sessions hidden by group collapse (0 when visible)
    pub hidden_count: usize,
    /// Per-status counts for hidden sessions (populated when collapsed)
    pub hidden_statuses: HashMap<ClaudeCodeStatus, usize>,
}

impl<'a> SessionGroup<'a> {
    pub fn non_session_lines(&self) -> usize {
        if self.label.is_some() {
            1
        } else if self.separator {
            1
        } else {
            0
        }
    }
}

pub fn group_key_for_session(session_name: &str, all_names: &[&str]) -> String {
    compute_group_key(session_name, all_names)
}

pub fn load_hidden_groups() -> HashSet<String> {
    let Some(home) = dirs::home_dir() else {
        return HashSet::new();
    };
    let path = home.join(".claude-tmux").join("hidden-groups.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn save_hidden_groups(groups: &HashSet<String>) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let dir = home.join(".claude-tmux");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("hidden-groups.json"),
        serde_json::to_string(groups).unwrap_or_default(),
    );
}

pub fn load_titles() -> HashMap<String, String> {
    let Some(home) = dirs::home_dir() else {
        return HashMap::new();
    };
    let path = home.join(".claude-tmux").join("titles.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn group_sessions<'a>(
    sessions: Vec<&'a Session>,
    titles: &HashMap<String, String>,
) -> Vec<SessionGroup<'a>> {
    if sessions.is_empty() {
        return vec![];
    }

    let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
    let group_keys: Vec<String> = names.iter().map(|name| compute_group_key(name, &names)).collect();

    let mut seen_keys: Vec<String> = Vec::new();
    let mut group_map: HashMap<String, Vec<&'a Session>> = HashMap::new();

    for (i, key) in group_keys.iter().enumerate() {
        if !group_map.contains_key(key) {
            seen_keys.push(key.clone());
        }
        group_map.entry(key.clone()).or_default().push(sessions[i]);
    }

    let mut headerless: Vec<SessionGroup<'a>> = Vec::new();
    let mut headed: Vec<SessionGroup<'a>> = Vec::new();

    for key in seen_keys {
        let sessions = group_map.remove(&key).unwrap();
        let label = if sessions.len() > 1
            || sessions.iter().any(|s| s.name != key)
            || is_task_id(&key)
        {
            Some(key.clone())
        } else {
            None
        };
        let strip_prefix = label.as_ref().is_some_and(|l| {
            !is_task_id(l)
                && sessions.iter().all(|s| s.name != *l)
                && sessions.iter().all(|s| {
                    s.name.starts_with(l.as_str())
                        && s.name.as_bytes().get(l.len()) == Some(&b'-')
                })
        });
        let title = label.as_ref().and_then(|k| titles.get(k).cloned());
        let group = SessionGroup {
            label,
            title,
            sessions,
            separator: false,
            strip_prefix,
            hidden_count: 0,
            hidden_statuses: HashMap::new(),
        };
        if group.label.is_some() {
            headed.push(group);
        } else {
            headerless.push(group);
        }
    }

    headerless.extend(headed);
    headerless
}

/// Returns true if the name matches a task ID pattern: `WORD-DIGITS` (exactly 2 segments).
fn is_task_id(name: &str) -> bool {
    let segments: Vec<&str> = name.splitn(3, '-').collect();
    segments.len() == 2
        && !segments[0].is_empty()
        && segments[0].chars().all(|c| c.is_ascii_alphanumeric())
        && !segments[1].is_empty()
        && segments[1].chars().all(|c| c.is_ascii_digit())
}

fn compute_group_key(name: &str, all_names: &[&str]) -> String {
    // Check if this name starts with another session's name + "-"
    // Use the longest matching parent for nested hierarchies
    let mut best_parent = "";
    for other in all_names {
        if name != *other
            && name.starts_with(other)
            && name.as_bytes().get(other.len()) == Some(&b'-')
            && other.len() > best_parent.len()
        {
            best_parent = other;
        }
    }
    if !best_parent.is_empty() {
        return best_parent.to_string();
    }

    // Check if any other session starts with this name + "-" (this is a parent)
    for other in all_names {
        if name != *other
            && other.starts_with(name)
            && other.as_bytes().get(name.len()) == Some(&b'-')
        {
            return name.to_string();
        }
    }

    // If name looks like WORD-NUMBER-... (3+ dash-segments, 2nd is numeric),
    // the first two segments form a natural task ID group key.
    if let Some(prefix) = extract_task_prefix(name) {
        return prefix;
    }

    // Find longest common prefix at a "-" boundary with any other session.
    // Multi-segment prefixes (containing a dash) are always accepted.
    // Single-segment prefixes are accepted only when neither name is a task ID,
    // so that "skill-flush" and "skill-linear" group under "skill" but
    // "VEL-419" and "VEL-420" do not merge under "VEL".
    let mut best_prefix = String::new();
    for other in all_names {
        if name == *other {
            continue;
        }
        let prefix = longest_common_prefix_at_dash(name, other);
        let accept = if prefix.contains('-') {
            true
        } else if !prefix.is_empty() {
            !is_task_id(name) && !is_task_id(other)
        } else {
            false
        };
        if accept && prefix.len() > best_prefix.len() {
            best_prefix = prefix;
        }
    }
    if !best_prefix.is_empty() {
        return best_prefix;
    }

    name.to_string()
}

/// If name has 3+ dash-delimited segments and the 2nd is numeric,
/// return the first two segments as a task-ID prefix (e.g. "VEL-420").
fn extract_task_prefix(name: &str) -> Option<String> {
    let segments: Vec<&str> = name.splitn(3, '-').collect();
    if segments.len() >= 3 && segments[1].chars().all(|c| c.is_ascii_digit()) {
        Some(format!("{}-{}", segments[0], segments[1]))
    } else {
        None
    }
}

fn longest_common_prefix_at_dash(a: &str, b: &str) -> String {
    let mut last_dash = None;
    for (i, (ca, cb)) in a.chars().zip(b.chars()).enumerate() {
        if ca != cb {
            break;
        }
        if ca == '-' {
            last_dash = Some(i);
        }
    }
    match last_dash {
        Some(pos) => a[..pos].to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{ClaudeCodeStatus, Session};
    use std::path::PathBuf;

    fn make_session(name: &str) -> Session {
        Session {
            name: name.to_string(),
            created: 0,
            attached: false,
            working_directory: PathBuf::from("/tmp"),
            window_count: 1,
            panes: vec![],
            claude_code_pane: None,
            claude_code_status: ClaudeCodeStatus::Idle,
            window_label: None,
            target_window_index: None,
            git_context: None,
        }
    }

    fn no_titles() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn parent_child_grouping() {
        let sessions = vec![
            make_session("VEL-420"),
            make_session("VEL-420-556-ci"),
            make_session("VEL-420-557-arc"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label.as_deref(), Some("VEL-420"));
        assert_eq!(groups[0].sessions.len(), 3);
    }

    #[test]
    fn sibling_grouping_without_parent() {
        let sessions = vec![
            make_session("VEL-420-556-ci"),
            make_session("VEL-420-557-arc"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label.as_deref(), Some("VEL-420"));
        assert_eq!(groups[0].sessions.len(), 2);
    }

    #[test]
    fn singletons_stay_separate() {
        let sessions = vec![
            make_session("claude-tmux"),
            make_session("md"),
            make_session("VEL-419"),
            make_session("VEL-551"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 4);
        // Non-task-ID sessions have no header
        assert!(groups[0].label.is_none()); // claude-tmux
        assert!(groups[1].label.is_none()); // md
        // Task-ID sessions get a header even when solo
        assert_eq!(groups[2].label.as_deref(), Some("VEL-419"));
        assert_eq!(groups[3].label.as_deref(), Some("VEL-551"));
    }

    #[test]
    fn mixed_groups_and_singletons() {
        let sessions = vec![
            make_session("claude-tmux"),
            make_session("md"),
            make_session("VEL-419"),
            make_session("VEL-420"),
            make_session("VEL-420-556-ci"),
            make_session("VEL-420-557-arc"),
            make_session("VEL-551"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        // VEL-420 group has 3 members
        let vel420: Vec<_> = groups
            .iter()
            .filter(|g| g.label.as_deref() == Some("VEL-420"))
            .collect();
        assert_eq!(vel420.len(), 1);
        assert_eq!(vel420[0].sessions.len(), 3);

        // Solo task IDs get headers too
        assert!(groups.iter().any(|g| g.label.as_deref() == Some("VEL-419")));
        assert!(groups.iter().any(|g| g.label.as_deref() == Some("VEL-551")));

        // Non-task sessions stay headerless
        let no_header: Vec<_> = groups.iter().filter(|g| g.label.is_none()).collect();
        assert_eq!(no_header.len(), 2); // claude-tmux, md
    }

    #[test]
    fn no_cross_task_grouping() {
        let sessions = vec![
            make_session("VEL-419"),
            make_session("VEL-420"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 2);
        // Both are task IDs so both get headers, but they don't merge
        assert_eq!(groups[0].label.as_deref(), Some("VEL-419"));
        assert_eq!(groups[1].label.as_deref(), Some("VEL-420"));
    }

    #[test]
    fn solo_task_id_gets_header() {
        let sessions = vec![make_session("VEL-421")];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label.as_deref(), Some("VEL-421"));
        assert_eq!(groups[0].sessions.len(), 1);
    }

    #[test]
    fn titles_attached_to_groups() {
        let sessions = vec![
            make_session("VEL-420"),
            make_session("VEL-420-556-ci"),
        ];
        let mut titles = HashMap::new();
        titles.insert("VEL-420".to_string(), "Self-hosted runner".to_string());
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &titles);

        assert_eq!(groups[0].title.as_deref(), Some("Self-hosted runner"));
    }

    #[test]
    fn solo_sub_issue_gets_group_header() {
        let sessions = vec![make_session("VEL-418-476")];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label.as_deref(), Some("VEL-418"));
        assert_eq!(groups[0].sessions.len(), 1);
    }

    #[test]
    fn solo_sub_issue_gets_title_from_parent_key() {
        let sessions = vec![make_session("VEL-418-476")];
        let mut titles = HashMap::new();
        titles.insert("VEL-418".to_string(), "Multi-AZ Kubernetes".to_string());
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &titles);

        assert_eq!(groups[0].label.as_deref(), Some("VEL-418"));
        assert_eq!(groups[0].title.as_deref(), Some("Multi-AZ Kubernetes"));
    }

    #[test]
    fn singleton_sessions_not_absorbed_by_multi_pane_groups() {
        let sessions = vec![
            make_session("0-orc"),
            make_session("claude-tmux"),
            make_session("claude-tmux"),
            make_session("cloudsim"),
            make_session("flush"),
            make_session("flush"),
            make_session("obsidian"),
            make_session("test-sonnet"),
            make_session("VEL-422"),
            make_session("VEL-422-cron"),
            make_session("VEL-423"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        for g in &groups {
            let names: Vec<_> = g.sessions.iter().map(|s| s.name.as_str()).collect();
            eprintln!("Group {:?} -> {:?}", g.label, names);
        }

        // obsidian and test-sonnet must not land in the flush group
        let flush_group = groups.iter().find(|g| g.label.as_deref() == Some("flush"));
        if let Some(fg) = flush_group {
            let names: Vec<_> = fg.sessions.iter().map(|s| s.name.as_str()).collect();
            assert!(!names.contains(&"obsidian"), "obsidian in flush group: {:?}", names);
            assert!(!names.contains(&"test-sonnet"), "test-sonnet in flush group: {:?}", names);
        }

        // cloudsim must not land in the claude-tmux group
        let ct_group = groups.iter().find(|g| g.label.as_deref() == Some("claude-tmux"));
        if let Some(cg) = ct_group {
            let names: Vec<_> = cg.sessions.iter().map(|s| s.name.as_str()).collect();
            assert!(!names.contains(&"cloudsim"), "cloudsim in claude-tmux group: {:?}", names);
        }

        // Each singleton should be ungrouped (no header)
        for name in &["cloudsim", "obsidian"] {
            let found = groups.iter().any(|g| {
                g.label.is_none() && g.sessions.iter().any(|s| s.name == *name)
            });
            assert!(found, "{} should be a headerless singleton", name);
        }

        // Singletons come before headed groups
        let first_headed = groups.iter().position(|g| g.label.is_some()).unwrap();
        let last_headerless = groups.iter().rposition(|g| g.label.is_none()).unwrap();
        assert!(last_headerless < first_headed, "all singletons should precede headed groups");
    }

    #[test]
    fn category_prefix_grouping() {
        let sessions = vec![
            make_session("skill-flush"),
            make_session("skill-linear"),
            make_session("skill-orchestrate"),
            make_session("tool-claude-tmux"),
            make_session("tool-obsidian"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 2);

        let skill = groups.iter().find(|g| g.label.as_deref() == Some("skill")).unwrap();
        assert_eq!(skill.sessions.len(), 3);
        assert!(skill.strip_prefix);

        let tool = groups.iter().find(|g| g.label.as_deref() == Some("tool")).unwrap();
        assert_eq!(tool.sessions.len(), 2);
        assert!(tool.strip_prefix);
    }

    #[test]
    fn category_prefix_does_not_merge_task_ids() {
        let sessions = vec![
            make_session("VEL-419"),
            make_session("VEL-420"),
            make_session("skill-flush"),
            make_session("skill-linear"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        // Task IDs stay separate
        assert!(groups.iter().any(|g| g.label.as_deref() == Some("VEL-419")));
        assert!(groups.iter().any(|g| g.label.as_deref() == Some("VEL-420")));

        // Skills group together with strip_prefix
        let skill = groups.iter().find(|g| g.label.as_deref() == Some("skill")).unwrap();
        assert_eq!(skill.sessions.len(), 2);
        assert!(skill.strip_prefix);
    }

    #[test]
    fn parent_session_prevents_strip_prefix() {
        let sessions = vec![
            make_session("skill"),
            make_session("skill-flush"),
            make_session("skill-linear"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        let skill = groups.iter().find(|g| g.label.as_deref() == Some("skill")).unwrap();
        assert_eq!(skill.sessions.len(), 3);
        assert!(!skill.strip_prefix);
    }

    #[test]
    fn task_id_group_no_strip_prefix() {
        let sessions = vec![
            make_session("VEL-420"),
            make_session("VEL-420-556-ci"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs, &no_titles());

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label.as_deref(), Some("VEL-420"));
        assert!(!groups[0].strip_prefix);
    }
}
