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

- `src/detection.rs` — Status detection logic. `content_above_status_bar()` for stripping status bar from diffs, `has_input_prompt()` for recognizing all prompt types, `detect_status()` / `detect_static_status()` for classifying pane content.
- `src/app/mod.rs` — Core app state. `tick_status()` runs on a configurable interval to update session statuses. Done lifecycle (`worked_unfocused`, `done_panes`), state file persistence, first-observation guard.
- `src/session.rs` — `ClaudeCodeStatus` enum with Idle, Working, Done, WaitingInput, Unknown.
- `src/settings.rs` — Settings file loading. Lookup: `~/.claude-tmux/settings.json` > repo `settings.json`.
- `src/ui/mod.rs` — Rendering. Status colors: Working=Green, Done=Cyan, WaitingInput=Yellow, Idle=DarkGray, Unknown=Gray.
- `src/tmux.rs` — tmux command wrappers. `capture_pane()` captures last N lines of a pane.
- `src/main.rs` — Entry point. Popup mode (default) vs headless daemon mode (`--headless`).
- `settings.json` — Default settings (repo-level). User override at `~/.claude-tmux/settings.json`.

## Building

```
cargo test && cargo build --release
```

The release binary lands at `target/release/claude-tmux`. The user's tmux keybinding already points here — no install step needed after rebuilding.

## Related files outside this repo

- `~/.tmux.conf.local` (symlinked from `~/config/roles/tmux/files/.tmux.conf.local`) — contains the `bind-key C` keybinding
- `~/.claude/statusline-command.sh` — Claude Code statusline script that reads `/tmp/claude-tmux-status`
- `~/.claude-tmux/settings.json` — User settings override
