# claude-tmux

A tmux popup TUI for managing multiple Claude Code sessions. Shows all tmux sessions with live status indicators and lets you switch between them.

Fork of [nielsgroen/claude-tmux](https://github.com/nielsgroen/claude-tmux) at [KSiig/claude-tmux](https://github.com/KSiig/claude-tmux) with improved status detection, a Done status, and a headless daemon mode.

<img src="docs/images/screenshot.png" alt="claude-tmux screenshot" width="400">

## Installation

```bash
git clone https://github.com/KSiig/claude-tmux.git
cd claude-tmux
cargo build --release
```

Binary: `target/release/claude-tmux`. Add a tmux keybinding to launch it -- see [docs/01-reference/keybindings.md](docs/01-reference/keybindings.md).

## Documentation

| Section | Covers |
|---------|--------|
| [docs/00-start-here/](docs/00-start-here/) | Onboarding, routing, repo layout |
| [docs/01-reference/](docs/01-reference/) | Keybindings, status indicator symbols |
| [docs/10-configuration/](docs/10-configuration/) | Settings file format and options |
| [docs/15-daemon/](docs/15-daemon/) | Headless daemon mode, status file format |
| [docs/20-status-detection/](docs/20-status-detection/) | Detection method, Done lifecycle |

## License

AGPL-3.0-only. See upstream [nielsgroen/claude-tmux](https://github.com/nielsgroen/claude-tmux) for original work.
