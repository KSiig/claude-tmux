use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const LINEAR_API: &str = "https://api.linear.app/graphql";
const CACHE_FILE: &str = "/tmp/claude-tmux-linear.json";

const QUERY: &str = r#"
query($ids: [ID!]!) {
  issues(filter: { id: { in: $ids } }) {
    nodes {
      identifier
      title
      state { name type }
    }
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueStatus {
    pub identifier: String,
    pub title: String,
    pub state_name: String,
    pub state_type: String,
}

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<GqlData>,
}

#[derive(Deserialize)]
struct GqlData {
    issues: GqlIssues,
}

#[derive(Deserialize)]
struct GqlIssues {
    nodes: Vec<GqlIssueNode>,
}

#[derive(Deserialize)]
struct GqlIssueNode {
    identifier: String,
    title: String,
    state: GqlState,
}

#[derive(Deserialize)]
struct GqlState {
    name: String,
    #[serde(rename = "type")]
    state_type: String,
}

pub struct LinearPoller {
    api_key: Option<String>,
    last_poll: Option<Instant>,
}

impl LinearPoller {
    pub fn new() -> Self {
        Self {
            api_key: std::env::var("LINEAR_API_KEY").ok(),
            last_poll: None,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn poll_if_due(
        &mut self,
        interval: std::time::Duration,
        identifiers: &[String],
    ) {
        if !self.is_configured() || identifiers.is_empty() {
            return;
        }
        if let Some(last) = self.last_poll {
            if last.elapsed() < interval {
                return;
            }
        }
        self.last_poll = Some(Instant::now());
        if let Err(e) = self.fetch_and_write(identifiers) {
            eprintln!("claude-tmux: linear poll failed: {}", e);
        }
    }

    fn fetch_and_write(&self, identifiers: &[String]) -> anyhow::Result<()> {
        let api_key = self.api_key.as_deref().unwrap();

        let body = serde_json::json!({
            "query": QUERY,
            "variables": { "ids": identifiers }
        });

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .new_agent();

        let resp: GqlResponse = agent
            .post(LINEAR_API)
            .header("Authorization", api_key)
            .header("Content-Type", "application/json")
            .send_json(&body)?
            .body_mut()
            .read_json()?;

        let statuses: Vec<IssueStatus> = resp
            .data
            .map(|d| {
                d.issues
                    .nodes
                    .into_iter()
                    .map(|n| IssueStatus {
                        identifier: n.identifier,
                        title: n.title,
                        state_name: n.state.name,
                        state_type: n.state.state_type,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let map: HashMap<String, IssueStatus> = statuses
            .into_iter()
            .map(|s| (s.identifier.clone(), s))
            .collect();

        let json = serde_json::to_string(&map)?;
        std::fs::write(CACHE_FILE, json)?;
        Ok(())
    }
}

pub fn load_cached() -> HashMap<String, IssueStatus> {
    let path = Path::new(CACHE_FILE);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// Extract Linear issue identifiers from session names.
/// When `prefix` is Some, only match names starting with that prefix (e.g. "VEL").
pub fn extract_identifiers(session_names: &[String], prefix: Option<&str>) -> Vec<String> {
    let mut seen = HashMap::new();
    for name in session_names {
        for id in extract_issue_ids(name, prefix) {
            seen.entry(id).or_insert(());
        }
    }
    seen.into_keys().collect()
}

/// Map a session name to its sub-issue identifier, if any.
/// "VEL-420-556-ci-migration" → Some("VEL-556")
pub fn session_sub_issue_id(name: &str, prefix: Option<&str>) -> Option<String> {
    let segments: Vec<&str> = name.splitn(4, '-').collect();
    if segments.len() >= 3
        && segments[0].chars().all(|c| c.is_ascii_alphanumeric())
        && segments[1].chars().all(|c| c.is_ascii_digit())
        && segments[2].chars().all(|c| c.is_ascii_digit())
        && !segments[2].is_empty()
    {
        if let Some(pfx) = prefix {
            if !segments[0].eq_ignore_ascii_case(pfx) {
                return None;
            }
        }
        Some(format!("{}-{}", segments[0], segments[2]))
    } else {
        None
    }
}

fn extract_issue_ids(name: &str, prefix: Option<&str>) -> Vec<String> {
    let segments: Vec<&str> = name.splitn(4, '-').collect();
    let mut ids = Vec::new();
    if segments.len() >= 2
        && segments[0].chars().all(|c| c.is_ascii_alphanumeric())
        && segments[1].chars().all(|c| c.is_ascii_digit())
        && !segments[1].is_empty()
    {
        if let Some(pfx) = prefix {
            if !segments[0].eq_ignore_ascii_case(pfx) {
                return ids;
            }
        }
        ids.push(format!("{}-{}", segments[0], segments[1]));
        if segments.len() >= 3
            && segments[2].chars().all(|c| c.is_ascii_digit())
            && !segments[2].is_empty()
        {
            ids.push(format!("{}-{}", segments[0], segments[2]));
        }
    }
    ids
}
