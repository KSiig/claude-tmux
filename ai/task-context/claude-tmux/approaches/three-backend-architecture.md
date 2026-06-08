# Approach: Three-Backend Detection Architecture

**Verdict**: Implemented
**Why**: The current hybrid hook+parsing system has a reconciliation layer (`parse_disagree_since`, `hook_override_delay`) that produces cascading bugs. Each signal source is individually imperfect, and merging them makes it worse. Instead, offer three clean, independent backends.

## Design

### Backend trait

A `DetectionBackend` trait with a single method: `detect(&self, pane_id: &str, ...) -> ClaudeCodeStatus`. Each backend is self-contained — no mixing signals between backends.

### Backend 1: Process-tree (default)

- Uses tmux `pane_pid` → walks process tree → checks for child processes and network connections
- Working: active children OR active TCP connections (API streaming)
- Idle: no children, no connections, has input prompt (minimal content check: last line only)
- WaitingInput: no children, last line contains `[y/n]`/`shift+tab`/`Esc to cancel`
- Weakness: "thinking" phase (no children, connection may be briefly absent) can briefly show Idle
- Platform-specific: macOS uses `ps`, Linux uses `/proc`

### Backend 2: Hooks-only

- Claude Code hooks write status to `/tmp/claude-tmux-hooks/{pane_id}`
- Monitor reads files, that's it. No pane capture for status detection.
- Stale hook file (>90s since last write) → Unknown
- Missing hook file → Unknown (hooks not configured)
- Requires `claude-tmux init` to set up hooks in Claude Code settings

### Backend 3: PTY sidecar (experimental)

- `claude-tmux monitor --pane {pane_id}` subcommand
- Attached via `tmux pipe-pane`, reads raw byte stream
- Simple state machine: "ctrl+c to interrupt" → Working, "❯" after "─" → Idle, "[y/n]" → WaitingInput
- Writes status to same `/tmp/claude-tmux-hooks/{pane_id}` format
- Real-time transitions (no polling interval)
- `claude-tmux init` sets up pipe-pane tmux hooks

### Done lifecycle (shared)

All three backends feed into the same Done lifecycle in `App::tick_status()`:
- Backend reports Working while unfocused → `worked_unfocused.insert(pane_id)`
- Backend reports Idle + pane in `worked_unfocused` → start `idle_since` timer
- Timer expires (`done_delay`) → `done_panes.insert(pane_id)`, status = Done
- User focuses pane → clear `worked_unfocused` and `done_panes`

### Configuration

`settings.json`:
```json
{
  "detection_method": "process"  // "process" | "hooks" | "sidecar"
}
```

`claude-tmux init` offers to configure hooks or sidecar, updating this setting.

## Key takeaway

The root cause of detection bugs is not in any single detection method — it's in the reconciliation layer that tries to merge two imperfect signals. Each backend should be self-contained and produce a single, clean status.
