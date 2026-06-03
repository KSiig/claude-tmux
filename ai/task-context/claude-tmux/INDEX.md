# Task Context: claude-tmux

> Fix false Done flash on popup open, make status detection reliable, add settings.json, restructure docs.

## Status

All work complete and pushed to main. Daemon restarted with new binary.

Changes across this session:
1. Fixed false Done flash on popup open (first-observation guard)
2. Fixed Working detection in `detect_static_status` (was missing "ctrl+c to interrupt" check)
3. Fixed popup treating attached session as "focused" (prevented Done for invoking session)
4. Added Done persistence across popup sessions via state file + `apply_persisted_done()`
5. Made only daemon write status file (prevents popup/daemon race on `/tmp/claude-tmux-status`)
6. Added fast second tick (~20ms after first) for quicker initial classification
7. Added configurable `status_interval_ms` via `settings.json`
8. Restructured docs into numbered folders, thinned README
9. Cleaned up all stale branches (kept only `fix/status-bar-flickering` for upstream PR #19)
10. Killed old daemon, started new one (PID changes each restart)

## Approaches Explored

| Approach | Verdict | Detail |
|----------|---------|--------|
| Remove state file persistence entirely | Rejected — no Done detection between popup sessions | [detail](approaches/remove-state-persistence.md) |
| Persist worked_unfocused only, not done_panes | Rejected — Done disappeared on second popup open | [detail](approaches/partial-persistence.md) |
| Force Idle on first tick for all statuses | Rejected — broke Working detection for static content | [detail](approaches/force-idle-first-tick.md) |
| First-observation guard on Idle + worked_unfocused only | Adopted — prevents false Done flash while preserving detection | [detail](approaches/first-observation-guard.md) |

## Key Files

| File | Role |
|------|------|
| `src/app/mod.rs` | Core state machine. `tick_status()` with first-observation guard, `apply_persisted_done()`, `writes_status_file` flag, `is_focused` popup vs daemon logic. |
| `src/detection.rs` | `detect_static_status()` now detects Working via "ctrl+c to interrupt" — this was missing and caused Working sessions to show as Idle. |
| `src/settings.rs` | New file. Loads `status_interval_ms` from `~/.claude-tmux/settings.json` > repo `settings.json`. |
| `src/main.rs` | Passes `headless` bool to `App::new()`. Daemon sleep uses configured interval. |
| `settings.json` | Repo-level defaults (500ms). User override at `~/.claude-tmux/settings.json` (currently 100ms). |

## Decisions

| Decision | Rationale | Detail |
|----------|-----------|--------|
| Only daemon writes status file | Popup and daemon were racing on `/tmp/claude-tmux-status` | [detail](decisions/status-file-ownership.md) |
| Popup treats no session as focused | tmux `attached` flag is true for invoking session even while popup is open | [detail](decisions/popup-focus-semantics.md) |
| Settings at ~/.claude-tmux/ not ~/Library/Application Support/ | Terminal-native tool for power users; XDG-style dotdir fits better | — |

## Gotchas

- `session.attached` (tmux's `#{session_attached}`) is "1" for the session the popup was invoked FROM, even while the popup overlay is showing. This prevented the invoking session from ever entering `worked_unfocused` and thus ever becoming Done.
- The first `tick_status()` call in the popup is throttled by `last_status_tick`. Setting it to `Instant::now() - Duration::from_secs(1)` makes the first tick fire immediately. Without this, the first tick doesn't fire until 500ms after open.
- `detect_static_status()` (used when content hasn't changed between ticks) originally had NO Working detection — only `detect_status()` (first tick only) checked for "ctrl+c to interrupt". Sessions running long static operations (like `sleep`) showed as Idle.
- The `first_observation` guard must exempt panes already in `done_panes` — otherwise tick 1 overwrites the Done status set by `apply_persisted_done()` with Idle, causing a Done→Idle→Done flicker.
- The daemon must be restarted after rebuilding to pick up the new binary. It's started with `nohup claude-tmux --headless > /dev/null 2>&1 &`.
