# Repo map

## What this repo manages

claude-tmux is a tmux popup TUI for managing multiple Claude Code sessions. It is a fork of [nielsgroen/claude-tmux](https://github.com/nielsgroen/claude-tmux) maintained at [KSiig/claude-tmux](https://github.com/KSiig/claude-tmux).

The fork adds:

- **Status bar stripping** to fix flickering between Working/Idle (upstream PR #19)
- **Done status** to track sessions that finished while unfocused
- **Plan approval detection** to recognize plan/hook/selection prompts as WaitingInput
- **Headless daemon mode** for continuous background monitoring
- **State persistence** across popup open/close cycles

## Directory layout

```
claude-tmux/
  Cargo.toml
  settings.json              # Default settings (repo-shipped)
  src/
    main.rs                  # Entry point -- popup vs headless mode
    app/
      mod.rs                 # App state, tick_status(), Done logic, state persistence
      mode.rs                # UI modes and actions
      helpers.rs             # Path/name utilities
    ui/
      mod.rs                 # Ratatui rendering, status colors
      dialogs.rs             # Dialog rendering (new session, worktree, PR, etc.)
      help.rs                # Help overlay rendering
    git/
      mod.rs                 # Git operations (worktree, PR, stage, commit, push)
    tmux.rs                  # tmux command wrapper (capture_pane, list_sessions, etc.)
    session.rs               # Session/Pane/ClaudeCodeStatus types
    detection.rs             # Status detection from pane content
    settings.rs              # Settings file loading
    input.rs                 # Keyboard event handling
    completion.rs            # Path/branch tab completion
    scroll_state.rs          # Scroll position tracking
  docs/                      # This documentation tree
  ai/
    chat-notes/              # Session-derived working notes (per topic)
```

## Key source files

| File | Responsibility |
|------|----------------|
| `src/detection.rs` | `content_above_status_bar()`, `detect_status()`, `detect_static_status()`, `has_input_prompt()`, `has_input_field()` |
| `src/app/mod.rs` | `tick_status()`, Done lifecycle, `worked_unfocused`/`done_panes` sets, state file read/write, `write_status_file()` |
| `src/session.rs` | `ClaudeCodeStatus` enum: Idle, Working, Done, WaitingInput, Unknown |
| `src/ui/mod.rs` | Ratatui rendering. Status colors: Working=Green, Done=Cyan, WaitingInput=Yellow, Idle=DarkGray, Unknown=Gray |
| `src/tmux.rs` | tmux command wrappers. `capture_pane()` captures last N lines of a pane |
| `src/settings.rs` | Settings file lookup and parsing |

## Files outside this repo

| File | Purpose |
|------|---------|
| `~/.tmux.conf.local` | Contains the `bind-key C` keybinding that launches claude-tmux |
| `~/.claude-tmux/settings.json` | User-level settings override |
| `~/.claude/statusline-command.sh` | Claude Code statusline script that reads `/tmp/claude-tmux-status` |
| `/tmp/claude-tmux-status` | Session status counts, written by daemon mode |
| `/tmp/claude-tmux-state` | Per-pane Done/working-unfocused state, shared between daemon and popup |

## Building

```bash
cargo test && cargo build --release
```

Binary: `target/release/claude-tmux`. The user's tmux keybinding already points here.

## Related

- [01-reference/keybindings.md](../01-reference/keybindings.md) -- all keybindings
- [01-reference/status-indicators.md](../01-reference/status-indicators.md) -- status symbols and colors
- [10-configuration/settings-file.md](../10-configuration/settings-file.md) -- settings file format
