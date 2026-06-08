# claude-tmux

A tmux popup TUI for managing multiple Claude Code sessions. Shows all tmux sessions with live status indicators and lets you switch between them.

Fork of [nielsgroen/claude-tmux](https://github.com/nielsgroen/claude-tmux) at [KSiig/claude-tmux](https://github.com/KSiig/claude-tmux) with pluggable status detection, a Done status, and a headless daemon mode.

<img src="docs/images/screenshot.png" alt="claude-tmux screenshot" width="400">

## Installation

```bash
git clone https://github.com/KSiig/claude-tmux.git
cd claude-tmux
cargo build --release
```

Binary: `target/release/claude-tmux`. Add a tmux keybinding to launch it -- see [docs/01-reference/keybindings.md](docs/01-reference/keybindings.md).

### Quick start

Out of the box, claude-tmux uses **process-tree detection** -- no setup required. Open the popup and session statuses appear immediately.

To switch to a more accurate detection method and install a background daemon, run:

```bash
claude-tmux init
```

This interactive wizard lets you choose between three detection backends:

| Backend | How it works | Setup |
|---------|-------------|-------|
| **Process** (default) | Inspects the process tree + pane content patterns | None |
| **Hooks** | Claude Code hooks write status on each lifecycle event | `claude-tmux init` adds hooks to `~/.claude/settings.json` |
| **Sidecar** (experimental) | Real-time stream analysis via tmux `pipe-pane` | `claude-tmux init` enables it; daemon spawns sidecars automatically |

The wizard also installs a **background daemon** (via launchd on macOS, systemd on Linux) that continuously monitors sessions and writes status counts to `/tmp/claude-tmux-status` for statusline integration.

See [docs/20-status-detection/](docs/20-status-detection/) for details on each backend.

## Documentation

| Section | Covers |
|---------|--------|
| [docs/00-start-here/](docs/00-start-here/) | Onboarding, routing, repo layout |
| [docs/01-reference/](docs/01-reference/) | Keybindings, status indicator symbols |
| [docs/10-configuration/](docs/10-configuration/) | Settings file format and options |
| [docs/15-daemon/](docs/15-daemon/) | Headless daemon mode, status file format |
| [docs/20-status-detection/](docs/20-status-detection/) | Detection backends, Done lifecycle |

## License

AGPL-3.0-only. See upstream [nielsgroen/claude-tmux](https://github.com/nielsgroen/claude-tmux) for original work.
