//! Application state and business logic
//!
//! This module contains the core application state machine:
//! - `App` struct: main application state
//! - Mode handling and transitions
//! - Session actions and execution
//! - Dialog flows (rename, new session, worktree, PR)

mod helpers;
mod mode;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::detection::{self, DetectionBackend, DetectionContext, DetectionMethod};
use crate::git::{self, GitContext, PullRequestInfo};
use crate::scroll_state::ScrollState;
use crate::session::{ClaudeCodeStatus, Session};
use crate::settings::{glob_match, Settings, SortMethod};
use crate::tmux::Tmux;

mod grouping;

// Re-export types that are part of the public API
pub use mode::{
    CreatePullRequestField, Mode, NewSessionField, NewWorktreeField, SessionAction,
};

// Use helpers internally
use helpers::{default_worktree_path, expand_path, sanitize_for_session_name};

/// An item in the navigable list: either a session or a collapsed group header.
#[derive(Debug, Clone)]
pub enum NavItem {
    Session(usize),
    CollapsedGroup { label: String },
}

/// Main application state
pub struct App {
    /// All discovered sessions
    pub sessions: Vec<Session>,
    /// Currently selected index
    pub selected: usize,
    /// Current UI mode
    pub mode: Mode,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Name of the currently attached session (if any)
    pub current_session: Option<String>,
    /// Filter text for filtering sessions
    pub filter: String,
    /// Error message to display (clears on next action)
    pub error: Option<String>,
    /// Success message to display (clears on next action)
    pub message: Option<String>,
    /// Cached preview content for the selected session's pane
    pub preview_content: Option<String>,
    /// Available actions for the selected session (computed when entering action menu)
    pub available_actions: Vec<SessionAction>,
    /// Currently highlighted action in ActionMenu mode
    pub selected_action: usize,
    /// Action pending confirmation
    pub pending_action: Option<SessionAction>,
    /// PR info for the selected session (computed when entering action menu)
    pub pr_info: Option<PullRequestInfo>,
    /// Scroll state for the session list
    pub scroll_state: ScrollState,
    /// Detection backend (process-tree, hooks, or sidecar)
    backend: Box<dyn DetectionBackend>,
    /// Which detection method is active
    detection_method: DetectionMethod,
    /// Pane IDs that have been Working while unfocused, used to detect Done transitions
    worked_unfocused: HashSet<String>,
    /// Pane IDs that have transitioned to Done (persisted across popup sessions)
    done_panes: HashSet<String>,
    /// Timestamp of the last status tick
    last_status_tick: Instant,
    /// Whether this instance owns the status file (only headless daemon should)
    writes_status_file: bool,
    /// Interval between status detection ticks
    status_interval: Duration,
    /// Cached group titles loaded from ~/.claude-tmux/titles.json
    pub group_titles: HashMap<String, String>,
    /// Whether to show git branch and dirty-star in the session list
    pub show_git_info: bool,
    /// Tracks when each pane first became Idle after being in worked_unfocused
    idle_since: HashMap<String, Instant>,
    /// How long a pane must remain Idle before transitioning to Done
    done_delay: Duration,
    /// Whether to show text labels next to session status icons
    pub session_status_labels: bool,
    /// How sessions are sorted (status+alpha vs status+recent)
    sort_method: SortMethod,
    /// Whether session grouping by shared name prefix is enabled
    pub grouping_enabled: bool,
    /// Whether to show task titles (from titles.json / API)
    pub task_show_titles: bool,
    /// Whether to show task status icons
    pub task_show_status: bool,
    /// Whether to show text labels next to task status icons
    pub task_status_labels: bool,
    /// Issue prefix for task identifier extraction (e.g. "VEL")
    pub task_issue_prefix: Option<String>,
    /// Cached Linear issue statuses (loaded from /tmp/claude-tmux-linear.json)
    pub linear_statuses: HashMap<String, crate::linear::IssueStatus>,
    /// Glob patterns for session names to exclude from the list
    exclude_sessions: Vec<String>,
    /// Pane IDs with active sidecars (sidecar backend only)
    sidecar_tracked: HashSet<String>,
    /// When each pane entered its current status (pane_id -> unix timestamp)
    pub status_since: HashMap<String, u64>,
    /// Group labels that are hidden (collapsed) in the session list
    pub hidden_groups: HashSet<String>,
    /// Receiver for background fork session results
    fork_rx: mpsc::Receiver<Result<String, String>>,
    /// Sender kept alive so the channel doesn't close prematurely
    fork_tx: mpsc::Sender<Result<String, String>>,
}

impl App {
    // =========================================================================
    // Initialization and core lifecycle
    // =========================================================================

    const STATE_FILE: &'static str = "/tmp/claude-tmux-state";

    /// Create a new App instance. When `headless` is true, this instance
    /// writes the status file for external consumers (statusline script).
    pub fn new(headless: bool) -> Result<Self> {
        let settings = Settings::load();
        let sessions = Tmux::list_sessions()?;
        let current_session = Tmux::current_session()?;
        let (worked_unfocused, done_panes, status_since) = Self::read_state_file();

        let (task_show_titles, task_show_status, task_status_labels, task_issue_prefix) =
            match &settings.task_integration {
                Some(t) => (
                    t.show_titles,
                    t.show_status,
                    t.status_labels,
                    t.issue_prefix.clone(),
                ),
                None => (false, false, false, None),
            };

        let group_titles = if task_show_titles {
            grouping::load_titles()
        } else {
            HashMap::new()
        };
        let linear_statuses = if task_show_status {
            crate::linear::load_cached()
        } else {
            HashMap::new()
        };

        let hidden_groups = grouping::load_hidden_groups();
        let exclude_sessions = settings.exclude_sessions.clone();
        let sessions = sessions
            .into_iter()
            .filter(|s| !settings.is_session_excluded(&s.name))
            .collect();

        let backend = detection::create_backend(
            settings.detection_method,
            settings.hook_staleness_secs,
        );

        let (fork_tx, fork_rx) = mpsc::channel();

        let mut app = Self {
            sessions,
            selected: 0,
            mode: Mode::Normal,
            should_quit: false,
            current_session,
            filter: String::new(),
            error: None,
            message: None,
            preview_content: None,
            available_actions: Vec::new(),
            selected_action: 0,
            pending_action: None,
            pr_info: None,
            scroll_state: ScrollState::new(),
            backend,
            detection_method: settings.detection_method,
            worked_unfocused,
            done_panes,
            last_status_tick: Instant::now() - Duration::from_secs(1),
            writes_status_file: headless,
            status_interval: settings.status_interval,
            group_titles,
            show_git_info: settings.show_git_info,
            idle_since: HashMap::new(),
            done_delay: settings.done_delay,
            session_status_labels: settings.session_status_labels,
            sort_method: settings.sort_method,
            grouping_enabled: settings.grouping,
            task_show_titles,
            task_show_status,
            task_status_labels,
            task_issue_prefix,
            linear_statuses,
            exclude_sessions,
            sidecar_tracked: HashSet::new(),
            status_since,
            hidden_groups,
            fork_rx,
            fork_tx,
        };

        app.apply_persisted_done();
        app.update_preview();
        Ok(app)
    }

    /// Apply persisted Done state to sessions on startup. These panes were
    /// verified as Done in a previous session, so they're safe to show
    /// immediately without waiting for content-diff confirmation.
    fn apply_persisted_done(&mut self) {
        for session in &mut self.sessions {
            if let Some(ref pane_id) = session.claude_code_pane {
                if self.done_panes.contains(pane_id)
                    && session.claude_code_status == ClaudeCodeStatus::Idle
                {
                    session.claude_code_status = ClaudeCodeStatus::Done;
                }
            }
        }
    }

    /// Update the preview content for the currently selected session
    pub fn update_preview(&mut self) {
        const PREVIEW_LINES: usize = 15;

        let pane_id = self.selected_session().and_then(|session| {
            // Prefer Claude pane, fall back to first pane
            session
                .claude_code_pane
                .clone()
                .or_else(|| session.panes.first().map(|p| p.id.clone()))
        });

        self.preview_content = pane_id.and_then(|id| {
            // Don't strip empty lines - preserve visual layout for preview
            Tmux::capture_pane(&id, PREVIEW_LINES, false).ok()
        });
    }

    /// Refresh Claude Code status for all panes using the configured backend.
    ///
    /// Called on every main-loop iteration but self-throttles to run at most
    /// every `status_interval` (default 500ms). Delegates detection to the
    /// configured backend (process-tree, hooks, or sidecar) and layers the
    /// Done lifecycle on top.
    pub fn tick_status(&mut self) {
        if self.last_status_tick.elapsed() < self.status_interval {
            return;
        }
        self.last_status_tick = Instant::now();

        self.backend.tick_start();

        let needs_content = self.backend.needs_content();

        // Collect targets: (session_index, pane_id, pane_pid)
        let targets: Vec<(usize, String, Option<u32>)> = self
            .sessions
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.claude_code_pane.as_ref().map(|id| {
                    let pid = s.panes.iter()
                        .find(|p| p.id == *id)
                        .and_then(|p| p.pid);
                    (i, id.clone(), pid)
                })
            })
            .collect();

        // For sidecar backend: ensure sidecars are running for all claude panes
        if self.detection_method == DetectionMethod::Sidecar {
            let pane_ids: Vec<String> = targets.iter().map(|(_, id, _)| id.clone()).collect();
            detection::sidecar::ensure_sidecars(&pane_ids, &mut self.sidecar_tracked);
        }

        for (idx, pane_id, pane_pid) in targets {
            let content = if needs_content {
                Tmux::capture_pane(&pane_id, 15, true).ok()
            } else {
                None
            };

            let ctx = DetectionContext {
                pane_pid,
                pane_content: content.as_deref(),
            };

            let raw_status = self.backend.detect(&pane_id, &ctx);

            // Done lifecycle: shared across all backends
            let is_focused = self.writes_status_file && self.sessions[idx].attached;

            if raw_status == ClaudeCodeStatus::Working && !is_focused {
                self.worked_unfocused.insert(pane_id.clone());
            }
            if is_focused {
                self.worked_unfocused.remove(&pane_id);
                self.done_panes.remove(&pane_id);
            }
            if raw_status != ClaudeCodeStatus::Idle {
                self.idle_since.remove(&pane_id);
                self.done_panes.remove(&pane_id);
            }

            let status = if raw_status == ClaudeCodeStatus::Idle
                && self.worked_unfocused.contains(&pane_id)
            {
                let idle_start = self
                    .idle_since
                    .entry(pane_id.clone())
                    .or_insert_with(Instant::now);
                if idle_start.elapsed() >= self.done_delay {
                    self.worked_unfocused.remove(&pane_id);
                    self.idle_since.remove(&pane_id);
                    self.done_panes.insert(pane_id.clone());
                    ClaudeCodeStatus::Done
                } else {
                    ClaudeCodeStatus::Idle
                }
            } else if raw_status == ClaudeCodeStatus::Idle
                && self.done_panes.contains(&pane_id)
                && !is_focused
            {
                ClaudeCodeStatus::Done
            } else {
                raw_status
            };

            let old_status = self.sessions[idx].claude_code_status;
            let now_epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if status != old_status {
                self.status_since.insert(pane_id.clone(), now_epoch);
            } else {
                self.status_since.entry(pane_id.clone()).or_insert(now_epoch);
            }
            self.sessions[idx].claude_code_status = status;
        }

        self.prune_stale_panes();
        self.write_state_file();
        if self.writes_status_file {
            self.write_status_file();
        }
    }

    /// Remove pane IDs from worked_unfocused/done_panes that no longer correspond
    /// to any known pane. Prevents unbounded growth of the state file when
    /// sessions are killed.
    fn prune_stale_panes(&mut self) {
        let live: HashSet<&str> = self
            .sessions
            .iter()
            .flat_map(|s| s.panes.iter().map(|p| p.id.as_str()))
            .collect();
        self.worked_unfocused.retain(|id| live.contains(id.as_str()));
        self.done_panes.retain(|id| live.contains(id.as_str()));
        self.idle_since.retain(|id, _| live.contains(id.as_str()));
        self.status_since.retain(|id, _| live.contains(id.as_str()));
        detection::hooks::cleanup_hook_files(&live);
    }

    fn read_state_file() -> (HashSet<String>, HashSet<String>, HashMap<String, u64>) {
        let mut worked = HashSet::new();
        let mut done = HashSet::new();
        let mut since = HashMap::new();
        if let Ok(content) = std::fs::read_to_string(Self::STATE_FILE) {
            for line in content.lines() {
                if let Some(id) = line.strip_prefix("w:") {
                    worked.insert(id.to_string());
                } else if let Some(id) = line.strip_prefix("d:") {
                    done.insert(id.to_string());
                } else if let Some(rest) = line.strip_prefix("s:") {
                    if let Some((id, ts)) = rest.split_once(' ') {
                        if let Ok(t) = ts.parse::<u64>() {
                            since.insert(id.to_string(), t);
                        }
                    }
                }
            }
        }
        (worked, done, since)
    }

    fn write_state_file(&self) {
        let mut content = String::new();
        for id in &self.worked_unfocused {
            content.push_str("w:");
            content.push_str(id);
            content.push('\n');
        }
        for id in &self.done_panes {
            content.push_str("d:");
            content.push_str(id);
            content.push('\n');
        }
        for (id, ts) in &self.status_since {
            content.push_str("s:");
            content.push_str(id);
            content.push(' ');
            content.push_str(&ts.to_string());
            content.push('\n');
        }
        let _ = std::fs::write(Self::STATE_FILE, content);
    }

    /// Write session status counts to a file for external consumers (e.g. status lines).
    fn write_status_file(&self) {
        let (working, waiting, idle, done, error, unknown) = self.status_counts();
        let content = format!(
            "working={}\ndone={}\nidle={}\nwaiting={}\nerror={}\nunknown={}\ntotal={}\n",
            working, done, idle, waiting, error, unknown, self.sessions.len()
        );
        let _ = std::fs::write("/tmp/claude-tmux-status", content);
    }

    /// Clear any displayed messages
    pub fn clear_messages(&mut self) {
        self.error = None;
        self.message = None;
    }

    /// Refresh the session list (shows "Refreshed" message)
    pub fn refresh(&mut self) {
        self.clear_messages();
        if self.task_show_titles {
            self.group_titles = grouping::load_titles();
        }
        if self.task_show_status {
            self.linear_statuses = crate::linear::load_cached();
        }
        if self.refresh_sessions() {
            self.message = Some("Refreshed".to_string());
        }
    }

    /// Refresh sessions and current attached session for headless daemon mode.
    /// Re-lists tmux sessions on every call so newly spawned sessions are picked up.
    pub fn refresh_for_daemon(&mut self) -> Result<()> {
        self.sessions = self.filter_excluded(Tmux::list_sessions_fast()?);
        self.current_session = Tmux::current_session()?;
        Ok(())
    }

    fn filter_excluded(&self, sessions: Vec<Session>) -> Vec<Session> {
        if self.exclude_sessions.is_empty() {
            return sessions;
        }
        sessions
            .into_iter()
            .filter(|s| !self.exclude_sessions.iter().any(|p| glob_match(p, &s.name)))
            .collect()
    }

    pub fn session_names(&self) -> Vec<String> {
        self.sessions.iter().map(|s| s.name.clone()).collect()
    }

    /// Refresh sessions without affecting messages (for use after git operations)
    fn refresh_sessions(&mut self) -> bool {
        match Tmux::list_sessions() {
            Ok(sessions) => {
                self.sessions = self.filter_excluded(sessions);
                // Ensure selected index is still valid against navigable items
                let nav_count = self.navigable_items().len();
                if nav_count > 0 && self.selected >= nav_count {
                    self.selected = nav_count - 1;
                }
                self.update_preview();
                true
            }
            Err(e) => {
                self.error = Some(format!("Failed to refresh: {}", e));
                false
            }
        }
    }

    // =========================================================================
    // Session selection and navigation
    // =========================================================================

    /// Get filtered sessions based on current filter
    pub fn filtered_sessions(&self) -> Vec<&Session> {
        let mut sessions: Vec<&Session> = if self.filter.is_empty() {
            self.sessions.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.sessions
                .iter()
                .filter(|s| {
                    s.name.to_lowercase().contains(&filter_lower)
                        || s.display_path().to_lowercase().contains(&filter_lower)
                })
                .collect()
        };
        match self.sort_method {
            SortMethod::StatusRecent => {
                sessions.sort_by(|a, b| {
                    a.claude_code_status
                        .sort_priority_recent()
                        .cmp(&b.claude_code_status.sort_priority_recent())
                        .then_with(|| b.last_activity.cmp(&a.last_activity))
                });
            }
            SortMethod::StatusAlpha => {
                sessions.sort_by(|a, b| {
                    a.claude_code_status
                        .sort_priority()
                        .cmp(&b.claude_code_status.sort_priority())
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
        }
        sessions
    }

    /// Flat session list in the same order as the visual display.
    /// When grouping is enabled, singletons come first, then headed groups.
    pub fn display_ordered_sessions(&self) -> Vec<&Session> {
        self.grouped_filtered_sessions()
            .into_iter()
            .flat_map(|g| g.sessions)
            .collect()
    }

    /// Get filtered sessions grouped by shared name prefix.
    /// Multi-member groups get a label (rendered as a header); singletons do not.
    pub fn grouped_filtered_sessions(&self) -> Vec<grouping::SessionGroup<'_>> {
        if !self.grouping_enabled {
            let filtered = self.filtered_sessions();
            return filtered
                .into_iter()
                .filter(|s| {
                    !self.hidden_groups.contains(&s.name)
                })
                .map(|s| grouping::SessionGroup {
                    label: None,
                    title: None,
                    sessions: vec![s],
                    separator: false,
                    strip_prefix: false,
                    hidden_count: 0,
                    hidden_statuses: std::collections::HashMap::new(),
                })
                .collect();
        }
        let mut groups = grouping::group_sessions(self.filtered_sessions(), &self.group_titles);
        if !self.hidden_groups.is_empty() {
            for group in &mut groups {
                let key = group.label.as_deref().unwrap_or_else(|| {
                    group.sessions.first().map(|s| s.name.as_str()).unwrap_or("")
                });
                if self.hidden_groups.contains(&key.to_ascii_lowercase()) {
                    group.hidden_count = group.sessions.len();
                    for s in &group.sessions {
                        *group.hidden_statuses.entry(s.claude_code_status).or_insert(0) += 1;
                    }
                    group.sessions.clear();
                }
            }
        }
        groups
    }

    /// Build the ordered list of navigable items (sessions + collapsed group headers).
    pub fn navigable_items(&self) -> Vec<NavItem> {
        let groups = self.grouped_filtered_sessions();
        let mut items = Vec::new();
        let mut session_idx = 0;
        for group in &groups {
            if group.hidden_count > 0 && group.sessions.is_empty() {
                if let Some(ref label) = group.label {
                    items.push(NavItem::CollapsedGroup { label: label.clone() });
                }
            }
            for _ in &group.sessions {
                items.push(NavItem::Session(session_idx));
                session_idx += 1;
            }
        }
        items
    }

    /// Count non-navigable lines (expanded group headers, separators) before a nav index.
    fn non_navigable_lines_before(&self, nav_selected: usize) -> usize {
        let groups = self.grouped_filtered_sessions();
        let mut nav_idx = 0;
        let mut extra = 0;
        for group in &groups {
            if nav_idx > nav_selected {
                break;
            }
            let is_collapsed = group.hidden_count > 0 && group.sessions.is_empty();
            if is_collapsed {
                // Collapsed header is a nav item, not an extra line
                nav_idx += 1;
            } else {
                extra += group.non_session_lines();
                nav_idx += group.sessions.len();
            }
        }
        extra
    }

    fn total_non_navigable_lines(&self) -> usize {
        self.grouped_filtered_sessions()
            .iter()
            .map(|g| {
                let is_collapsed = g.hidden_count > 0 && g.sessions.is_empty();
                if is_collapsed { 0 } else { g.non_session_lines() }
            })
            .sum()
    }

    /// Get the currently selected session (None if a collapsed header is selected)
    pub fn selected_session(&self) -> Option<&Session> {
        let nav = self.navigable_items();
        match nav.get(self.selected) {
            Some(NavItem::Session(i)) => {
                let ordered = self.display_ordered_sessions();
                ordered.get(*i).copied()
            }
            _ => None,
        }
    }

    /// Get the label of the selected collapsed group header, if any
    pub fn selected_collapsed_group(&self) -> Option<String> {
        let nav = self.navigable_items();
        match nav.get(self.selected) {
            Some(NavItem::CollapsedGroup { label }) => Some(label.clone()),
            _ => None,
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        let count = self.navigable_items().len();
        if count > 0 && self.selected > 0 {
            self.selected -= 1;
            self.update_preview();
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let count = self.navigable_items().len();
        if count > 0 && self.selected < count - 1 {
            self.selected += 1;
            self.update_preview();
        }
    }

    /// Switch to the selected session
    pub fn switch_to_selected(&mut self) {
        self.clear_messages();
        if let Some(idx) = self.selected_session_index() {
            self.sessions[idx].claude_code_status = match self.sessions[idx].claude_code_status {
                ClaudeCodeStatus::Done => ClaudeCodeStatus::Idle,
                other => other,
            };
            if let Some(ref pane_id) = self.sessions[idx].claude_code_pane {
                self.worked_unfocused.remove(pane_id);
                self.done_panes.remove(pane_id);
            }
            self.write_state_file();
            let target = self.sessions[idx].switch_target();
            match Tmux::switch_to_session(&target) {
                Ok(_) => {
                    self.should_quit = true;
                }
                Err(e) => {
                    self.error = Some(format!("Failed to switch: {}", e));
                }
            }
        }
    }

    /// Get the index into self.sessions for the currently selected filtered session
    fn selected_session_index(&self) -> Option<usize> {
        let session = self.selected_session()?;
        self.sessions.iter().position(|s| std::ptr::eq(s, session))
    }

    // =========================================================================
    // Action menu
    // =========================================================================

    /// Enter the action menu for the selected session
    pub fn enter_action_menu(&mut self) {
        self.clear_messages();
        if self.selected_session().is_some() {
            self.compute_actions();
            self.mode = Mode::ActionMenu;
        }
    }

    /// Move to next action in the action menu
    pub fn select_next_action(&mut self) {
        if !self.available_actions.is_empty() {
            self.selected_action = (self.selected_action + 1) % self.available_actions.len();
        }
    }

    /// Move to previous action in the action menu
    pub fn select_prev_action(&mut self) {
        if !self.available_actions.is_empty() {
            if self.selected_action == 0 {
                self.selected_action = self.available_actions.len() - 1;
            } else {
                self.selected_action -= 1;
            }
        }
    }

    /// Execute the currently selected action from the action menu
    pub fn execute_selected_action(&mut self) {
        if let Some(action) = self.available_actions.get(self.selected_action).cloned() {
            if action.requires_confirmation() {
                self.pending_action = Some(action);
                self.mode = Mode::ConfirmAction;
            } else {
                // execute_action handles its own mode transitions
                self.execute_action(action);
            }
        }
    }

    /// Compute available actions for the selected session
    fn compute_actions(&mut self) {
        // Extract data we need from the session first to avoid borrow conflicts
        let session_data = self.selected_session().map(|s| {
            (s.working_directory.clone(), s.git_context.clone())
        });

        let Some((working_dir, git_context)) = session_data else {
            self.available_actions = vec![];
            self.pr_info = None;
            return;
        };

        let mut actions = vec![SessionAction::SwitchTo, SessionAction::Rename];

        // Reset PR info
        self.pr_info = None;

        // Add git actions if applicable
        if let Some(ref git) = git_context {
            // New worktree: available for any git repo
            actions.push(SessionAction::NewWorktree);

            // Stage: if there are unstaged changes
            if git.has_unstaged {
                actions.push(SessionAction::Stage);
            }
            // Commit: if there are staged changes
            if git.has_staged {
                actions.push(SessionAction::Commit);
            }

            // Fetch: always available if there's a remote (safe operation)
            if git.has_remote {
                actions.push(SessionAction::Fetch);
            }

            if git.has_upstream {
                // Push: ahead > 0 (dirty state doesn't prevent pushing commits)
                if git.ahead > 0 {
                    actions.push(SessionAction::Push);
                }
                // Pull: behind > 0 and clean (dirty state can cause merge conflicts)
                if git.behind > 0 && !git.is_dirty() {
                    actions.push(SessionAction::Pull);
                }

                // PR actions: upstream exists, gh available, GitHub remote, not on default branch
                if git::is_gh_available() && git::is_github_remote(&working_dir) {
                    // Check if not on default branch
                    if let Some(default_branch) = git::get_default_branch(&working_dir) {
                        if git.branch != default_branch {
                            // Check if PR already exists for this branch
                            let pr_info = git::get_pull_request_info(&working_dir);
                            if let Some(ref info) = pr_info {
                                if info.state == "OPEN" {
                                    actions.push(SessionAction::ViewPullRequest);
                                    actions.push(SessionAction::ClosePullRequest);
                                    actions.push(SessionAction::MergePullRequest);
                                    actions.push(SessionAction::MergePullRequestAndClose);
                                } else {
                                    // PR exists but is CLOSED or MERGED - can create a new one
                                    actions.push(SessionAction::CreatePullRequest);
                                }
                            } else {
                                // No PR exists, offer to create one
                                actions.push(SessionAction::CreatePullRequest);
                            }
                            // Store PR info for UI display
                            self.pr_info = pr_info;
                        }
                    }
                }
            } else if git.has_remote {
                // No upstream but remote exists - offer to push and set upstream
                actions.push(SessionAction::PushSetUpstream);
            }
        }

        actions.push(SessionAction::Kill);

        // Add worktree deletion option if this is a worktree
        if let Some(ref git) = git_context {
            if git.is_worktree {
                actions.push(SessionAction::KillAndDeleteWorktree);
            }
        }

        self.available_actions = actions;
        self.selected_action = 0;
    }

    // =========================================================================
    // Action execution
    // =========================================================================

    /// Start the kill confirmation flow (direct kill without action menu)
    pub fn start_kill(&mut self) {
        self.clear_messages();
        if self.selected_session().is_some() {
            self.pending_action = Some(SessionAction::Kill);
            self.mode = Mode::ConfirmAction;
        }
    }

    /// Confirm and execute the pending action
    pub fn confirm_action(&mut self) {
        if let Some(action) = self.pending_action.take() {
            self.execute_action(action);
        }
        self.mode = Mode::Normal;
    }

    /// Execute an action on the selected session
    fn execute_action(&mut self, action: SessionAction) {
        let Some(session) = self.selected_session() else {
            self.mode = Mode::Normal;
            return;
        };
        let session_name = session.name.clone();
        let switch_target = session.switch_target();

        match action {
            SessionAction::SwitchTo => {
                match Tmux::switch_to_session(&switch_target) {
                    Ok(_) => self.should_quit = true,
                    Err(e) => self.error = Some(format!("Failed to switch: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::Rename => {
                self.mode = Mode::Rename {
                    old_name: session_name.clone(),
                    new_name: session_name,
                };
            }
            SessionAction::Stage => {
                let path = session.working_directory.clone();
                match GitContext::stage_all(&path) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some("Staged all changes".to_string());
                    }
                    Err(e) => self.error = Some(format!("Stage failed: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::Commit => {
                self.mode = Mode::Commit {
                    message: String::new(),
                };
            }
            SessionAction::Push => {
                let path = session.working_directory.clone();
                match GitContext::push(&path) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some("Pushed to remote".to_string());
                    }
                    Err(e) => self.error = Some(format!("Push failed: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::PushSetUpstream => {
                let path = session.working_directory.clone();
                match GitContext::push_set_upstream(&path) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some("Pushed and set upstream".to_string());
                    }
                    Err(e) => self.error = Some(format!("Push failed: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::Fetch => {
                let path = session.working_directory.clone();
                match GitContext::fetch(&path) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some("Fetched from remote".to_string());
                    }
                    Err(e) => self.error = Some(format!("Fetch failed: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::Pull => {
                let path = session.working_directory.clone();
                match GitContext::pull(&path) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some("Pulled from remote".to_string());
                    }
                    Err(e) => self.error = Some(format!("Pull failed: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::CreatePullRequest => {
                self.start_create_pull_request();
            }
            SessionAction::ViewPullRequest => {
                let path = session.working_directory.clone();
                match git::view_pull_request(&path) {
                    Ok(_) => {
                        self.message = Some("Opened PR in browser".to_string());
                    }
                    Err(e) => self.error = Some(format!("Failed to open PR: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::ClosePullRequest => {
                let path = session.working_directory.clone();
                match git::close_pull_request(&path) {
                    Ok(_) => {
                        self.message = Some("Closed pull request".to_string());
                    }
                    Err(e) => self.error = Some(format!("Failed to close PR: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::MergePullRequest => {
                let path = session.working_directory.clone();
                match git::merge_pull_request(&path, false) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some("Merged pull request".to_string());
                    }
                    Err(e) => self.error = Some(format!("Failed to merge PR: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::MergePullRequestAndClose => {
                let path = session.working_directory.clone();
                let is_worktree = session
                    .git_context
                    .as_ref()
                    .map(|g| g.is_worktree)
                    .unwrap_or(false);

                // Step 1: Merge PR
                match git::merge_pull_request(&path, false) {
                    Ok(_) => {
                        // Step 2: Delete worktree if applicable
                        if is_worktree {
                            if let Err(e) = GitContext::delete_worktree(&path, true) {
                                self.error =
                                    Some(format!("PR merged but failed to delete worktree: {}", e));
                                self.mode = Mode::Normal;
                                return;
                            }
                        }

                        // Step 3: Kill the session
                        match Tmux::kill_session(&session_name) {
                            Ok(_) => {
                                self.refresh_sessions();
                                self.message = Some(if is_worktree {
                                    "Merged PR, removed worktree, and closed session".to_string()
                                } else {
                                    "Merged PR and closed session".to_string()
                                });
                            }
                            Err(e) => {
                                self.refresh_sessions();
                                self.error = Some(format!(
                                    "PR merged but failed to kill session: {}",
                                    e
                                ));
                            }
                        }
                    }
                    Err(e) => self.error = Some(format!("Failed to merge PR: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::Kill => {
                match Tmux::kill_session(&session_name) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some(format!("Killed session '{}'", session_name));
                    }
                    Err(e) => self.error = Some(format!("Failed to kill: {}", e)),
                }
                self.mode = Mode::Normal;
            }
            SessionAction::NewWorktree => {
                self.start_new_worktree();
            }
            SessionAction::KillAndDeleteWorktree => {
                let worktree_path = session.working_directory.clone();
                // First delete the worktree (while session still provides git context)
                match GitContext::delete_worktree(&worktree_path, false) {
                    Ok(_) => {
                        // Then kill the session
                        match Tmux::kill_session(&session_name) {
                            Ok(_) => {
                                self.refresh_sessions();
                                self.message = Some(format!(
                                    "Deleted worktree and killed session '{}'",
                                    session_name
                                ));
                            }
                            Err(e) => {
                                self.refresh_sessions();
                                self.error = Some(format!(
                                    "Worktree deleted but failed to kill session: {}",
                                    e
                                ));
                            }
                        }
                    }
                    Err(e) => self.error = Some(format!("Failed to delete worktree: {}", e)),
                }
                self.mode = Mode::Normal;
            }
        }
    }

    // =========================================================================
    // Dialog flows: Rename
    // =========================================================================

    /// Start the rename flow
    pub fn start_rename(&mut self) {
        self.clear_messages();
        if let Some(session) = self.selected_session() {
            self.mode = Mode::Rename {
                old_name: session.name.clone(),
                new_name: session.name.clone(),
            };
        }
    }

    /// Confirm and execute session rename
    pub fn confirm_rename(&mut self) {
        if let Mode::Rename {
            ref old_name,
            ref new_name,
        } = self.mode
        {
            let old = old_name.clone();
            let new = new_name.clone();

            if old == new {
                self.mode = Mode::Normal;
                return;
            }

            match Tmux::rename_session(&old, &new) {
                Ok(_) => {
                    self.refresh_sessions();
                    self.message = Some(format!("Renamed '{}' to '{}'", old, new));
                }
                Err(e) => {
                    self.error = Some(format!("Failed to rename: {}", e));
                }
            }
        }
        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Dialog flows: Commit
    // =========================================================================

    /// Confirm and execute the commit
    pub fn confirm_commit(&mut self) {
        if let Mode::Commit { ref message } = self.mode {
            if message.trim().is_empty() {
                self.error = Some("Commit message cannot be empty".to_string());
                self.mode = Mode::Normal;
                return;
            }

            if let Some(session) = self.selected_session() {
                let path = session.working_directory.clone();
                let msg = message.clone();
                match GitContext::commit(&path, &msg) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some("Committed changes".to_string());
                    }
                    Err(e) => self.error = Some(format!("Commit failed: {}", e)),
                }
            }
        }
        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Dialog flows: New Session
    // =========================================================================

    /// Start the new session flow
    pub fn start_new_session(&mut self) {
        self.clear_messages();
        // Default to current directory
        let default_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "~".to_string());

        // Get initial path suggestions
        let completion = crate::completion::complete_path(&default_path);

        self.mode = Mode::NewSession {
            name: String::new(),
            path: default_path,
            field: NewSessionField::Name,
            path_suggestions: completion.suggestions,
            path_selected: None,
        };
    }

    /// Create the new session
    pub fn confirm_new_session(&mut self, start_claude: bool) {
        if let Mode::NewSession {
            ref name, ref path, ..
        } = self.mode
        {
            if name.is_empty() {
                self.error = Some("Session name cannot be empty".to_string());
                self.mode = Mode::Normal;
                return;
            }

            let session_name = name.clone();
            let session_path = expand_path(path);

            match Tmux::new_session(&session_name, &session_path, start_claude) {
                Ok(_) => {
                    self.refresh_sessions();
                    self.message = Some(format!("Created session '{}'", session_name));
                }
                Err(e) => {
                    self.error = Some(format!("Failed to create session: {}", e));
                }
            }
        }
        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Dialog flows: New Worktree
    // =========================================================================

    /// Start the new worktree flow
    pub fn start_new_worktree(&mut self) {
        self.clear_messages();
        let Some(session) = self.selected_session() else {
            return;
        };

        // Get the repo path (use main repo if this is a worktree)
        let source_repo = if let Some(ref git) = session.git_context {
            if git.is_worktree {
                git.main_repo_path
                    .clone()
                    .unwrap_or_else(|| session.working_directory.clone())
            } else {
                session.working_directory.clone()
            }
        } else {
            return; // Not a git repo
        };

        // Get list of branches
        let all_branches = match GitContext::list_branches(&source_repo) {
            Ok(branches) => branches,
            Err(e) => {
                self.error = Some(format!("Failed to list branches: {}", e));
                return;
            }
        };

        self.mode = Mode::NewWorktree {
            source_repo,
            all_branches,
            branch_input: String::new(),
            selected_branch: None,
            worktree_path: String::new(),
            session_name: String::new(),
            field: NewWorktreeField::Branch,
            path_suggestions: Vec::new(),
            path_selected: None,
        };
    }

    /// Get filtered branches based on current input
    pub fn filtered_branches(&self) -> Vec<&str> {
        if let Mode::NewWorktree {
            ref all_branches,
            ref branch_input,
            ..
        } = self.mode
        {
            if branch_input.is_empty() {
                all_branches.iter().map(|s| s.as_str()).collect()
            } else {
                let input_lower = branch_input.to_lowercase();
                all_branches
                    .iter()
                    .filter(|b| b.to_lowercase().contains(&input_lower))
                    .map(|s| s.as_str())
                    .collect()
            }
        } else {
            vec![]
        }
    }

    /// Update suggestions when branch input changes
    pub fn update_worktree_suggestions(&mut self) {
        if let Mode::NewWorktree {
            ref source_repo,
            ref all_branches,
            ref branch_input,
            ref mut selected_branch,
            ref mut worktree_path,
            ref mut session_name,
            ..
        } = self.mode
        {
            // Filter branches
            let filtered: Vec<&str> = if branch_input.is_empty() {
                all_branches.iter().map(|s| s.as_str()).collect()
            } else {
                let input_lower = branch_input.to_lowercase();
                all_branches
                    .iter()
                    .filter(|b| b.to_lowercase().contains(&input_lower))
                    .map(|s| s.as_str())
                    .collect()
            };

            // Update selected branch
            if filtered.is_empty() {
                *selected_branch = None;
            } else if let Some(idx) = *selected_branch {
                if idx >= filtered.len() {
                    *selected_branch = Some(filtered.len() - 1);
                }
            }

            // Auto-update path and session name based on branch input
            let branch_for_path = if let Some(idx) = *selected_branch {
                filtered.get(idx).copied().unwrap_or(branch_input.as_str())
            } else {
                branch_input.as_str()
            };

            if !branch_for_path.is_empty() {
                *worktree_path = default_worktree_path(source_repo, branch_for_path)
                    .to_string_lossy()
                    .to_string();
                // Session name: repo-name + branch suffix
                let repo_name = source_repo
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("repo");
                let branch_suffix = sanitize_for_session_name(branch_for_path);
                *session_name = format!("{}-{}", repo_name, branch_suffix);
            }
        }
    }

    /// Create the new worktree and session
    pub fn confirm_new_worktree(&mut self) {
        let (source_repo, all_branches, branch_input, selected_branch, worktree_path, session_name) =
            if let Mode::NewWorktree {
                ref source_repo,
                ref all_branches,
                ref branch_input,
                selected_branch,
                ref worktree_path,
                ref session_name,
                ..
            } = self.mode
            {
                (
                    source_repo.clone(),
                    all_branches.clone(),
                    branch_input.clone(),
                    selected_branch,
                    worktree_path.clone(),
                    session_name.clone(),
                )
            } else {
                return;
            };

        // Validate inputs
        if branch_input.is_empty() && selected_branch.is_none() {
            self.error = Some("Branch name cannot be empty".to_string());
            self.mode = Mode::Normal;
            return;
        }

        if session_name.is_empty() {
            self.error = Some("Session name cannot be empty".to_string());
            self.mode = Mode::Normal;
            return;
        }

        if worktree_path.is_empty() {
            self.error = Some("Worktree path cannot be empty".to_string());
            self.mode = Mode::Normal;
            return;
        }

        // Determine if this is a new branch or existing
        let filtered: Vec<&str> = if branch_input.is_empty() {
            all_branches.iter().map(|s| s.as_str()).collect()
        } else {
            let input_lower = branch_input.to_lowercase();
            all_branches
                .iter()
                .filter(|b| b.to_lowercase().contains(&input_lower))
                .map(|s| s.as_str())
                .collect()
        };

        let (branch_name, is_new_branch) = if let Some(idx) = selected_branch {
            // User selected an existing branch
            (
                filtered
                    .get(idx)
                    .copied()
                    .unwrap_or(&branch_input)
                    .to_string(),
                false,
            )
        } else if all_branches.iter().any(|b| b == &branch_input) {
            // Exact match with existing branch
            (branch_input.clone(), false)
        } else {
            // New branch
            (branch_input.clone(), true)
        };

        let worktree_path_buf = expand_path(&worktree_path);

        // Create the worktree
        match GitContext::create_worktree(
            &source_repo,
            &worktree_path_buf,
            &branch_name,
            is_new_branch,
        ) {
            Ok(_) => {
                // Create the session
                match Tmux::new_session(&session_name, &worktree_path_buf, true) {
                    Ok(_) => {
                        self.refresh_sessions();
                        self.message = Some(format!(
                            "Created worktree '{}' and session '{}'",
                            branch_name, session_name
                        ));
                    }
                    Err(e) => {
                        self.error = Some(format!(
                            "Worktree created but session creation failed: {}",
                            e
                        ));
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("Failed to create worktree: {}", e));
            }
        }

        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Dialog flows: Create Pull Request
    // =========================================================================

    /// Start the create pull request flow
    pub fn start_create_pull_request(&mut self) {
        self.clear_messages();
        let Some(session) = self.selected_session() else {
            return;
        };

        let path = &session.working_directory;
        let base_branch = git::get_default_branch(path).unwrap_or_else(|| "main".to_string());

        self.mode = Mode::CreatePullRequest {
            title: String::new(),
            body: String::new(),
            base_branch,
            field: CreatePullRequestField::Title,
        };
    }

    /// Confirm and execute PR creation
    pub fn confirm_create_pull_request(&mut self) {
        let (title, body, base_branch) = if let Mode::CreatePullRequest {
            ref title,
            ref body,
            ref base_branch,
            ..
        } = self.mode
        {
            (title.clone(), body.clone(), base_branch.clone())
        } else {
            self.mode = Mode::Normal;
            return;
        };

        if title.trim().is_empty() {
            self.error = Some("PR title cannot be empty".to_string());
            self.mode = Mode::Normal;
            return;
        }

        if let Some(session) = self.selected_session() {
            let path = session.working_directory.clone();
            match git::create_pull_request(&path, &title, &body, &base_branch) {
                Ok(result) => {
                    self.message = Some(format!("Created PR: {}", result.url));
                }
                Err(e) => {
                    self.error = Some(format!("Failed to create PR: {}", e));
                }
            }
        }

        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Filter mode
    // =========================================================================

    /// Start filter mode
    pub fn start_filter(&mut self) {
        self.clear_messages();
        self.mode = Mode::Filter {
            input: self.filter.clone(),
        };
    }

    /// Apply filter and return to normal mode
    pub fn apply_filter(&mut self) {
        if let Mode::Filter { ref input } = self.mode {
            self.filter = input.clone();
            self.selected = 0; // Reset selection when filter changes
        }
        self.mode = Mode::Normal;
        self.update_preview();
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.selected = 0;
    }

    /// Toggle hiding the group of the currently selected session
    pub fn toggle_hide_group(&mut self) {
        self.clear_messages();
        let names: Vec<String> = self.session_names();
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();

        let Some(session) = self.selected_session() else {
            return;
        };
        let group_key = grouping::group_key_for_session(&session.name, &name_refs);
        let group_key_lower = group_key.to_ascii_lowercase();

        if self.hidden_groups.contains(&group_key_lower) {
            self.hidden_groups.remove(&group_key_lower);
            self.message = Some(format!("Unhid group '{}'", group_key));
        } else {
            self.hidden_groups.insert(group_key_lower);
            let count = self.navigable_items().len();
            if self.selected >= count && count > 0 {
                self.selected = count - 1;
            }
            self.message = Some(format!("Hid group '{}'", group_key));
        }
        grouping::save_hidden_groups(&self.hidden_groups);
        self.update_preview();
    }

    /// Unhide selected collapsed group, or all hidden groups if on a session
    pub fn unhide_groups(&mut self) {
        self.clear_messages();
        if self.hidden_groups.is_empty() {
            return;
        }
        if let Some(label) = self.selected_collapsed_group() {
            self.hidden_groups.remove(&label.to_ascii_lowercase());
            grouping::save_hidden_groups(&self.hidden_groups);
            self.message = Some(format!("Unhid group '{}'", label));
        } else {
            let count = self.hidden_groups.len();
            self.hidden_groups.clear();
            grouping::save_hidden_groups(&self.hidden_groups);
            self.message = Some(format!("Unhid {} group(s)", count));
        }
        self.update_preview();
    }

    /// Open the set-status picker for the selected session
    pub fn start_set_status(&mut self) {
        self.clear_messages();
        if self.selected_session().is_some() {
            let current = self
                .selected_session()
                .map(|s| s.claude_code_status)
                .unwrap_or_default();
            let idx = ClaudeCodeStatus::ALL
                .iter()
                .position(|s| *s == current)
                .unwrap_or(0);
            self.mode = Mode::SetStatus { selected: idx };
        }
    }

    pub fn select_next_status(&mut self) {
        if let Mode::SetStatus { ref mut selected } = self.mode {
            *selected = (*selected + 1) % ClaudeCodeStatus::ALL.len();
        }
    }

    pub fn select_prev_status(&mut self) {
        if let Mode::SetStatus { ref mut selected } = self.mode {
            if *selected == 0 {
                *selected = ClaudeCodeStatus::ALL.len() - 1;
            } else {
                *selected -= 1;
            }
        }
    }

    pub fn confirm_set_status(&mut self) {
        let chosen = if let Mode::SetStatus { selected } = self.mode {
            ClaudeCodeStatus::ALL[selected]
        } else {
            return;
        };

        if let Some(idx) = self.selected_session_index() {
            let pane_id = self.sessions[idx].claude_code_pane.clone();
            self.sessions[idx].claude_code_status = chosen;
            if let Some(pane_id) = pane_id {
                if chosen == ClaudeCodeStatus::Done {
                    self.done_panes.insert(pane_id.clone());
                    self.worked_unfocused.remove(&pane_id);
                } else {
                    self.done_panes.remove(&pane_id);
                    self.worked_unfocused.remove(&pane_id);
                }
                self.idle_since.remove(&pane_id);
            }
        }
        self.write_state_file();
        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Dialog flows: Fork Session
    // =========================================================================

    /// Start the fork session flow
    pub fn start_fork_session(&mut self) {
        self.clear_messages();
        let Some(session) = self.selected_session() else {
            return;
        };
        if session.claude_code_pane.is_none() {
            self.error = Some("No Claude pane in this session".to_string());
            return;
        }

        self.mode = Mode::ForkSession {
            source_session: session.name.clone(),
            source_dir: session.working_directory.clone(),
            branch_name: session.name.clone(),
        };
    }

    /// Confirm and execute the fork (non-blocking)
    pub fn confirm_fork_session(&mut self) {
        let (source_session, source_dir, branch_name) =
            if let Mode::ForkSession {
                ref source_session,
                ref source_dir,
                ref branch_name,
            } = self.mode
            {
                (
                    source_session.clone(),
                    source_dir.clone(),
                    branch_name.clone(),
                )
            } else {
                return;
            };

        if branch_name.is_empty() {
            self.error = Some("Branch name cannot be empty".to_string());
            self.mode = Mode::Normal;
            return;
        }

        let existing_names: HashSet<&str> =
            self.sessions.iter().map(|s| s.name.as_str()).collect();
        let new_session_name = if existing_names.contains(branch_name.as_str()) {
            format!("{}-fork", branch_name)
        } else {
            branch_name.clone()
        };

        match Tmux::new_session(&new_session_name, &source_dir, false) {
            Ok(_) => {
                if let Err(e) = Tmux::send_keys(
                    &new_session_name,
                    &["claude --continue", "Enter"],
                ) {
                    self.error = Some(format!("Session created but claude failed to start: {}", e));
                    self.mode = Mode::Normal;
                    return;
                }

                self.message = Some(format!("Forking '{}' — waiting for Claude...", source_session));
                self.refresh_sessions();

                let tx = self.fork_tx.clone();
                let session_name = new_session_name.clone();
                let branch = branch_name.clone();
                let source = source_session.clone();

                std::thread::spawn(move || {
                    let pane_id = Tmux::first_pane_id(&session_name).ok().flatten();

                    let ready = if let Some(ref pid) = pane_id {
                        let mut found = false;
                        for _ in 0..30 {
                            std::thread::sleep(Duration::from_millis(500));
                            if let Ok(content) = Tmux::capture_pane(pid, 15, true) {
                                let status =
                                    crate::detection::content::detect_from_content(&content);
                                if status == ClaudeCodeStatus::Idle {
                                    found = true;
                                    break;
                                }
                            }
                        }
                        found
                    } else {
                        std::thread::sleep(Duration::from_secs(5));
                        true
                    };

                    if !ready {
                        let _ = tx.send(Err(
                            "Timed out waiting for Claude to start — /branch not sent".to_string(),
                        ));
                        return;
                    }

                    let branch_cmd = format!("/branch {}", branch);
                    if let Err(e) = Tmux::send_keys(&session_name, &[&branch_cmd, "Enter"]) {
                        let _ = tx.send(Err(format!("Session forked but /branch failed: {}", e)));
                    } else {
                        let _ = tx.send(Ok(format!(
                            "Forked '{}' → '{}'",
                            source, session_name
                        )));
                    }
                });
            }
            Err(e) => {
                self.error = Some(format!("Failed to create fork session: {}", e));
            }
        }

        self.mode = Mode::Normal;
    }

    /// Check for completed background fork results (called from tick_status)
    pub fn check_fork_result(&mut self) {
        if let Ok(result) = self.fork_rx.try_recv() {
            match result {
                Ok(msg) => {
                    self.refresh_sessions();
                    self.message = Some(msg);
                }
                Err(msg) => {
                    self.refresh_sessions();
                    self.error = Some(msg);
                }
            }
        }
    }

    /// Show help
    pub fn show_help(&mut self) {
        self.clear_messages();
        self.mode = Mode::Help;
    }

    /// Cancel current mode and return to normal
    pub fn cancel(&mut self) {
        self.pending_action = None;
        self.pr_info = None;
        self.mode = Mode::Normal;
    }

    // =========================================================================
    // Status and statistics
    // =========================================================================

    /// Count sessions by status
    pub fn status_counts(&self) -> (usize, usize, usize, usize, usize, usize) {
        use crate::session::ClaudeCodeStatus;

        let mut working = 0;
        let mut waiting = 0;
        let mut idle = 0;
        let mut done = 0;
        let mut error = 0;
        let mut unknown = 0;

        for session in &self.sessions {
            match session.claude_code_status {
                ClaudeCodeStatus::Working => working += 1,
                ClaudeCodeStatus::Done => done += 1,
                ClaudeCodeStatus::WaitingInput => waiting += 1,
                ClaudeCodeStatus::Idle => idle += 1,
                ClaudeCodeStatus::Error => error += 1,
                ClaudeCodeStatus::Unknown => unknown += 1,
            }
        }

        (working, waiting, idle, done, error, unknown)
    }

    // =========================================================================
    // Path completion methods
    // =========================================================================

    /// Update path suggestions for NewSession mode
    pub fn update_new_session_path_suggestions(&mut self) {
        if let Mode::NewSession {
            ref path,
            ref mut path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            let completion = crate::completion::complete_path(path);
            *path_suggestions = completion.suggestions;
            // Reset selection if it's out of bounds
            if let Some(idx) = *path_selected {
                if idx >= path_suggestions.len() {
                    *path_selected = if path_suggestions.is_empty() {
                        None
                    } else {
                        Some(path_suggestions.len() - 1)
                    };
                }
            }
        }
    }

    /// Update path suggestions for NewWorktree mode
    pub fn update_worktree_path_suggestions(&mut self) {
        if let Mode::NewWorktree {
            ref worktree_path,
            ref mut path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            let completion = crate::completion::complete_path(worktree_path);
            *path_suggestions = completion.suggestions;
            // Reset selection if it's out of bounds
            if let Some(idx) = *path_selected {
                if idx >= path_suggestions.len() {
                    *path_selected = if path_suggestions.is_empty() {
                        None
                    } else {
                        Some(path_suggestions.len() - 1)
                    };
                }
            }
        }
    }

    /// Select previous path suggestion in NewSession mode
    pub fn select_prev_new_session_path(&mut self) {
        if let Mode::NewSession {
            ref path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            if path_suggestions.is_empty() {
                return;
            }
            *path_selected = Some(
                path_selected
                    .map(|i| {
                        if i == 0 {
                            path_suggestions.len() - 1
                        } else {
                            i - 1
                        }
                    })
                    .unwrap_or(path_suggestions.len() - 1),
            );
        }
    }

    /// Select next path suggestion in NewSession mode
    pub fn select_next_new_session_path(&mut self) {
        if let Mode::NewSession {
            ref path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            if path_suggestions.is_empty() {
                return;
            }
            *path_selected = Some(
                path_selected
                    .map(|i| (i + 1) % path_suggestions.len())
                    .unwrap_or(0),
            );
        }
    }

    /// Accept the current path completion in NewSession mode
    pub fn accept_new_session_path_completion(&mut self) {
        if let Mode::NewSession {
            ref mut path,
            ref path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            // If a suggestion is selected, use it
            if let Some(idx) = *path_selected {
                if let Some(suggestion) = path_suggestions.get(idx) {
                    *path = suggestion.clone();
                    *path_selected = None;
                }
            } else if let Some(first) = path_suggestions.first() {
                // Otherwise use the first suggestion (ghost text)
                *path = first.clone();
            }
        }
        // Update suggestions after accepting
        self.update_new_session_path_suggestions();
    }

    /// Select previous path suggestion in NewWorktree mode
    pub fn select_prev_worktree_path(&mut self) {
        if let Mode::NewWorktree {
            ref path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            if path_suggestions.is_empty() {
                return;
            }
            *path_selected = Some(
                path_selected
                    .map(|i| {
                        if i == 0 {
                            path_suggestions.len() - 1
                        } else {
                            i - 1
                        }
                    })
                    .unwrap_or(path_suggestions.len() - 1),
            );
        }
    }

    /// Select next path suggestion in NewWorktree mode
    pub fn select_next_worktree_path(&mut self) {
        if let Mode::NewWorktree {
            ref path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            if path_suggestions.is_empty() {
                return;
            }
            *path_selected = Some(
                path_selected
                    .map(|i| (i + 1) % path_suggestions.len())
                    .unwrap_or(0),
            );
        }
    }

    /// Accept the current path completion in NewWorktree mode
    pub fn accept_worktree_path_completion(&mut self) {
        if let Mode::NewWorktree {
            ref mut worktree_path,
            ref path_suggestions,
            ref mut path_selected,
            ..
        } = self.mode
        {
            // If a suggestion is selected, use it
            if let Some(idx) = *path_selected {
                if let Some(suggestion) = path_suggestions.get(idx) {
                    *worktree_path = suggestion.clone();
                    *path_selected = None;
                }
            } else if let Some(first) = path_suggestions.first() {
                // Otherwise use the first suggestion (ghost text)
                *worktree_path = first.clone();
            }
        }
        // Update suggestions after accepting
        self.update_worktree_path_suggestions();
    }

    /// Accept the current branch completion in NewWorktree mode
    pub fn accept_branch_completion(&mut self) {
        let selected_branch_name = if let Mode::NewWorktree {
            ref all_branches,
            ref branch_input,
            selected_branch,
            ..
        } = self.mode
        {
            // Get filtered branches
            let filtered: Vec<&str> = if branch_input.is_empty() {
                all_branches.iter().map(|s| s.as_str()).collect()
            } else {
                let input_lower = branch_input.to_lowercase();
                all_branches
                    .iter()
                    .filter(|b| b.to_lowercase().contains(&input_lower))
                    .map(|s| s.as_str())
                    .collect()
            };

            // Get the branch to accept
            if let Some(idx) = selected_branch {
                filtered.get(idx).map(|s| s.to_string())
            } else {
                filtered.first().map(|s| s.to_string())
            }
        } else {
            None
        };

        // Now update the branch_input with the selected branch
        if let Some(branch_name) = selected_branch_name {
            if let Mode::NewWorktree {
                ref mut branch_input,
                ref mut selected_branch,
                ..
            } = self.mode
            {
                *branch_input = branch_name;
                *selected_branch = None;
            }
            self.update_worktree_suggestions();
        }
    }

    // =========================================================================
    // Scroll/list computation
    // =========================================================================

    /// Compute the flat list index for the current selection.
    ///
    /// The list has a complex structure where the selected session expands
    /// to show metadata and action items. This method computes the index
    /// into the flat list of rendered items.
    pub fn compute_flat_list_index(&self) -> usize {
        let nav_count = self.navigable_items().len();
        if nav_count == 0 {
            return 0;
        }

        let header_offset = self.non_navigable_lines_before(self.selected);

        match self.mode {
            Mode::ActionMenu => {
                let mut index = self.selected + header_offset;
                index += 1; // selected session row itself
                index += 1; // metadata row

                if self
                    .selected_session()
                    .is_some_and(|s| s.git_context.is_some())
                {
                    index += 1;
                    if self.pr_info.is_some() {
                        index += 1;
                    }
                }

                index += 1; // separator
                index += self.selected_action;
                index
            }
            _ => self.selected + header_offset,
        }
    }

    /// Compute the total number of items in the rendered list.
    ///
    /// This accounts for the expanded content when in ActionMenu mode.
    pub fn compute_total_list_items(&self) -> usize {
        let nav_count = self.navigable_items().len();
        if nav_count == 0 {
            return 0;
        }

        let non_nav_lines = self.total_non_navigable_lines();

        match self.mode {
            Mode::ActionMenu => {
                let mut total = nav_count + non_nav_lines;

                total += 1; // metadata row

                if self
                    .selected_session()
                    .is_some_and(|s| s.git_context.is_some())
                {
                    total += 1; // git info row
                    if self.pr_info.is_some() {
                        total += 1; // PR info row
                    }
                }

                total += 1; // separator
                total += self.available_actions.len(); // action rows
                total += 1; // end separator

                total
            }
            _ => nav_count + non_nav_lines,
        }
    }
}
