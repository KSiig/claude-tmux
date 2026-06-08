# Detection method

claude-tmux supports three pluggable detection backends, selected via `detection_method` in [settings-file.md](../10-configuration/settings-file.md). The default is `"process"` (no setup required). Run `claude-tmux init` to switch backends and install the background daemon.

Detection runs on a configurable tick interval (default 500 ms, set via `status_interval_ms`).

## Backends

### Process (default)

Inspects the process tree of each Claude Code pane and falls back to pane content analysis.

1. Finds the `claude` or `node` process in the pane's process tree via `ps`.
2. Checks if that process has non-node child processes (tool subprocesses).
   - If yes -> **Working** (actively executing tools).
   - If no -> Falls back to content analysis (see [Content analysis](#content-analysis) below).

No setup required. Works out of the box.

### Hooks

Claude Code hooks write status to `/tmp/claude-tmux-hooks/<pane_id>` on each lifecycle event.

**Setup**: `claude-tmux init` creates a hook script at `~/.claude-tmux/hooks/status.sh` and registers hooks in `~/.claude/settings.json` for these events:

| Event | Status written |
|-------|---------------|
| `UserPromptSubmit` | `working` |
| `Stop` | `idle` |
| `StopFailure` | `error` |
| `PermissionRequest` | `waiting_input` |
| `Elicitation` | `waiting_input` |

The hook file format is `<status> <unix_timestamp>`, e.g. `working 1717500000`.

If a hook file is older than `hook_staleness_secs` (default 90), the pane is treated as **Unknown**. Stale hook files are cleaned up on each tick.

### Sidecar (experimental)

Real-time stream analysis via tmux `pipe-pane`. The daemon spawns `claude-tmux monitor --pane <id>` as a sidecar process for each Claude Code pane.

The sidecar:
1. Establishes a baseline from current pane content.
2. Reads the stdin stream from `pipe-pane` into a ring buffer (8 KB).
3. Strips ANSI escape codes and detects status from stream content.
4. Writes detected status to `/tmp/claude-tmux-hooks/<pane_id>` (same format as hooks backend).

**Setup**: `claude-tmux init` sets `detection_method` to `"sidecar"`. The daemon automatically starts and manages sidecar processes.

## Content analysis

Shared by the process backend (as fallback) and used internally by the sidecar backend. Analyzes the last 15 lines of pane content for known patterns.

### Input prompt detection (`has_input_field`)

A Claude Code input field is detected when a line contains `❯` and the line directly above it contains `─` (the border).

### WaitingInput patterns (`has_input_prompt`)

Any of these strings in the pane content triggers WaitingInput:

- `[y/n]` -- permission prompts
- `[Y/n]` -- permission prompts (default yes)
- `shift+tab to approve` -- plan approval and selection prompts
- `Esc to cancel` -- hook and tool confirmation prompts

WaitingInput takes priority over all other statuses.

### Working detection (static)

When `ctrl+c` and `to interrupt` are both present in the content, the session is classified as Working.

### Status bar stripping

Claude Code renders a status bar below the input prompt (showing model name, cost, elapsed time, etc.). This status bar updates every second, which would cause false Working detection if compared directly.

`content_above_status_bar()` finds the input prompt boundary -- a line containing `❯` with a `─` border line directly above it -- and returns only the content up to and including the prompt line. Everything below (status bar lines) is excluded from content comparison.

## Classification summary (process backend)

| Condition | Status |
|-----------|--------|
| Tool subprocesses running | Working |
| Content contains WaitingInput pattern | WaitingInput |
| `Effecting…` present | Working |
| Input field present + `ctrl+c to interrupt` | Working |
| Input field present, no interrupt message | Idle |
| No input field + `ctrl+c to interrupt` | Working |
| None of the above | Unknown |

## Key source files

| File | Contains |
|------|----------|
| `src/detection/mod.rs` | `DetectionBackend` trait, `DetectionMethod` enum, `create_backend()` |
| `src/detection/process.rs` | Process-tree detection with content-analysis fallback |
| `src/detection/hooks.rs` | Hook file reading, staleness checking |
| `src/detection/sidecar.rs` | Sidecar lifecycle management, pipe-pane spawning |
| `src/detection/content.rs` | `content_above_status_bar()`, `has_input_prompt()`, `has_input_field()` |
| `src/monitor.rs` | Sidecar monitor process (stream reader, ring buffer, status writer) |
| `src/session.rs` | `ClaudeCodeStatus` enum (Idle, Working, Done, WaitingInput, Error, Unknown) |

## Related

- [done-lifecycle.md](done-lifecycle.md) -- how Done transitions work (not detected from pane content)
- [../01-reference/status-indicators.md](../01-reference/status-indicators.md) -- symbols and colors for each status
- [../10-configuration/settings-file.md](../10-configuration/settings-file.md) -- `status_interval_ms`, `detection_method`, `hook_staleness_secs`
- [../15-daemon/headless-mode.md](../15-daemon/headless-mode.md) -- daemon vs popup detection differences
