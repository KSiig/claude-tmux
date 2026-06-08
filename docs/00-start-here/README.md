# Start here

Routing guide for the claude-tmux documentation.

| Page | Use it for |
|------|------------|
| [repo-map.md](repo-map.md) | What this repo manages, directory layout, key source files |

## Where to look next

- **Keybindings or status indicator symbols** -- [01-reference/](../01-reference/)
- **Settings file format and options** -- [10-configuration/settings-file.md](../10-configuration/settings-file.md)
- **Headless daemon mode** -- [15-daemon/headless-mode.md](../15-daemon/headless-mode.md)
- **How status detection works** -- [20-status-detection/](../20-status-detection/)
- **Open items or gaps** -- [90-open-items/](../90-open-items/)

## For AI assistants

This repo is a Rust tmux popup TUI. The primary areas of interest:

- **Status detection**: `src/detection/` -- three pluggable backends (process, hooks, sidecar) behind a `DetectionBackend` trait. Content analysis shared in `src/detection/content.rs`.
- **Setup wizard**: `src/init.rs` -- `claude-tmux init` configures detection method, installs hooks, sets up daemon service.
- **Application state and tick loop**: `src/app/mod.rs` -- `tick_status()`, Done lifecycle, state file persistence.
- **Session types**: `src/session.rs` -- `ClaudeCodeStatus` enum (Idle, Working, Done, WaitingInput, Error, Unknown).
- **Configuration**: `settings.json` at repo root or `~/.claude-tmux/settings.json` for user overrides. Key settings: `detection_method`, `hook_staleness_secs`.
- **Daemon vs popup**: `--headless` / `-d` flag runs a background monitor that writes `/tmp/claude-tmux-status`.

Start with [repo-map.md](repo-map.md) for the full directory layout.

## Scope

This folder contains onboarding and routing material only. Detailed reference, configuration, and internals live in their own numbered folders.

## Related

- [docs/README.md](../README.md) -- top-level map
- [01-reference/](../01-reference/) -- fast lookup tables
