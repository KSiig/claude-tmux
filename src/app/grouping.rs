use std::collections::HashMap;

use crate::session::Session;

pub struct SessionGroup<'a> {
    pub label: Option<String>,
    pub sessions: Vec<&'a Session>,
}

impl<'a> SessionGroup<'a> {
    pub fn has_header(&self) -> bool {
        self.label.is_some()
    }
}

pub fn group_sessions<'a>(sessions: Vec<&'a Session>) -> Vec<SessionGroup<'a>> {
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

    seen_keys
        .into_iter()
        .map(|key| {
            let sessions = group_map.remove(&key).unwrap();
            let label = if sessions.len() > 1 {
                Some(key)
            } else {
                None
            };
            SessionGroup { label, sessions }
        })
        .collect()
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

    // Find longest common prefix at a "-" boundary with any other session.
    // Must contain at least one dash (two segments) to avoid over-grouping.
    let mut best_prefix = String::new();
    for other in all_names {
        if name == *other {
            continue;
        }
        let prefix = longest_common_prefix_at_dash(name, other);
        if prefix.contains('-') && prefix.len() > best_prefix.len() {
            best_prefix = prefix;
        }
    }
    if !best_prefix.is_empty() {
        return best_prefix;
    }

    name.to_string()
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

    #[test]
    fn parent_child_grouping() {
        let sessions = vec![
            make_session("VEL-420"),
            make_session("VEL-420-556-ci"),
            make_session("VEL-420-557-arc"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs);

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
        let groups = group_sessions(refs);

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
        let groups = group_sessions(refs);

        assert_eq!(groups.len(), 4);
        assert!(groups.iter().all(|g| g.label.is_none()));
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
        let groups = group_sessions(refs);

        let multi: Vec<_> = groups.iter().filter(|g| g.has_header()).collect();
        assert_eq!(multi.len(), 1);
        assert_eq!(multi[0].label.as_deref(), Some("VEL-420"));
        assert_eq!(multi[0].sessions.len(), 3);

        let singles: Vec<_> = groups.iter().filter(|g| !g.has_header()).collect();
        assert_eq!(singles.len(), 4);
    }

    #[test]
    fn no_cross_task_grouping() {
        let sessions = vec![
            make_session("VEL-419"),
            make_session("VEL-420"),
        ];
        let refs: Vec<&Session> = sessions.iter().collect();
        let groups = group_sessions(refs);

        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|g| g.label.is_none()));
    }
}
