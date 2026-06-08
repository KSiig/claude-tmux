# claude-tmux (fork)

This is a fork of [nielsgroen/claude-tmux](https://github.com/nielsgroen/claude-tmux) at [KSiig/claude-tmux](https://github.com/KSiig/claude-tmux).

## What this project is

A tmux popup TUI for managing multiple Claude Code sessions. Bound to `Ctrl-a, Shift-c` in the user's tmux config. Shows a list of all tmux sessions with Claude Code status indicators, and lets you switch between them.

## Documentation

Full docs live in `docs/` under numbered folders. See `docs/README.md` for the top-level map.

| Folder | Subject |
|--------|---------|
| `docs/00-start-here/` | Onboarding, routing, repo layout |
| `docs/01-reference/` | Keybindings, status indicators — fast lookup |
| `docs/10-configuration/` | Settings file format and options |
| `docs/15-daemon/` | Headless daemon mode, status/state files |
| `docs/20-status-detection/` | Detection method, Done lifecycle |
| `docs/90-open-items/` | Unresolved items |
| `docs/91-tech-debt/` | Accepted shortcuts with rationale |

## Key files

- `src/detection/mod.rs` — `DetectionBackend` trait, `DetectionMethod` enum, `create_backend()` factory. Three pluggable backends: process (default), hooks, sidecar.
- `src/detection/process.rs` — Process-tree detection with content-analysis fallback.
- `src/detection/hooks.rs` — Reads status from `/tmp/claude-tmux-hooks/<pane_id>`, staleness checking.
- `src/detection/sidecar.rs` — Sidecar lifecycle management, pipe-pane spawning.
- `src/detection/content.rs` — Shared content analysis: `content_above_status_bar()`, `has_input_prompt()`, `has_input_field()`.
- `src/monitor.rs` — Sidecar monitor process: reads pipe-pane stream, detects status, writes hook files.
- `src/init.rs` — `claude-tmux init` setup wizard: detection method selection, hook installation, daemon service setup (launchd/systemd).
- `src/app/mod.rs` — Core app state. `tick_status()` runs on a configurable interval to update session statuses. Done lifecycle (`worked_unfocused`, `done_panes`), state file persistence, first-observation guard.
- `src/session.rs` — `ClaudeCodeStatus` enum with Idle, Working, Done, WaitingInput, Error, Unknown.
- `src/settings.rs` — Settings file loading. Top-level options (`detection_method`, `hook_staleness_secs`, `grouping`, etc.) and optional `task_integration` block. Lookup: `~/.claude-tmux/settings.json` > repo `settings.json`.
- `src/linear.rs` — Linear API polling (`LinearPoller`), identifier extraction with optional prefix filter, cache file at `/tmp/claude-tmux-linear.json`.
- `src/app/grouping.rs` — Session grouping by shared name prefix. `extract_task_prefix()`, `load_titles()` from `~/.claude-tmux/titles.json`.
- `src/ui/mod.rs` — Rendering. Status colors: Working=Green, Done=Cyan, WaitingInput=Yellow, Idle=DarkGray, Unknown=Gray.
- `src/tmux.rs` — tmux command wrappers. `capture_pane()` captures last N lines of a pane.
- `src/main.rs` — Entry point. Subcommands: `init`, `monitor --pane <id>`, `--headless`/`-d`, default popup.
- `settings.json` — Default settings (repo-level). User override at `~/.claude-tmux/settings.json`.

## Building

```
cargo test && cargo build --release
```

The release binary lands at `target/release/claude-tmux`. The user's tmux keybinding already points here — no install step needed after rebuilding.

## Related files outside this repo

- `~/.tmux.conf.local` (symlinked from `~/config/roles/tmux/files/.tmux.conf.local`) — contains the `bind-key C` keybinding
- `~/.claude/settings.json` — Claude Code settings; hooks backend registers event hooks here via `claude-tmux init`
- `~/.claude/statusline-command.sh` — Claude Code statusline script that reads `/tmp/claude-tmux-status`
- `~/.claude-tmux/settings.json` — User settings override (including `detection_method`)
- `~/.claude-tmux/hooks/status.sh` — Hook script created by `claude-tmux init` (hooks backend)
