# Repo map

## What this repo manages

claude-tmux is a tmux popup TUI for managing multiple Claude Code sessions. It is a fork of [nielsgroen/claude-tmux](https://github.com/nielsgroen/claude-tmux) maintained at [KSiig/claude-tmux](https://github.com/KSiig/claude-tmux).

The fork adds:

- **Pluggable detection backends** (process-tree, hooks, sidecar) with `claude-tmux init` setup wizard
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
    main.rs                  # Entry point -- popup, headless, init, monitor subcommands
    init.rs                  # `claude-tmux init` setup wizard
    monitor.rs               # Sidecar monitor process (pipe-pane stream reader)
    app/
      mod.rs                 # App state, tick_status(), Done logic, state persistence
      grouping.rs            # Session grouping by shared name prefix
      mode.rs                # UI modes and actions
      helpers.rs             # Path/name utilities
    detection/
      mod.rs                 # DetectionBackend trait, DetectionMethod enum, create_backend()
      process.rs             # Process-tree detection with content-analysis fallback
      hooks.rs               # Hook file reading and staleness checking
      sidecar.rs             # Sidecar lifecycle management, pipe-pane spawning
      content.rs             # Shared content analysis (status bar stripping, pattern matching)
    ui/
      mod.rs                 # Ratatui rendering, status colors
      dialogs.rs             # Dialog rendering (new session, worktree, PR, etc.)
      help.rs                # Help overlay rendering
    git/
      mod.rs                 # Git operations (worktree, PR, stage, commit, push)
    tmux.rs                  # tmux command wrapper (capture_pane, list_sessions, etc.)
    session.rs               # Session/Pane/ClaudeCodeStatus types
    settings.rs              # Settings file loading
    linear.rs                # Linear API polling, identifier extraction
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
| `src/detection/mod.rs` | `DetectionBackend` trait, `DetectionMethod` enum, `create_backend()` factory |
| `src/detection/process.rs` | Process-tree detection with content-analysis fallback |
| `src/detection/hooks.rs` | Hook file reading (`/tmp/claude-tmux-hooks/`), staleness checking |
| `src/detection/sidecar.rs` | Sidecar lifecycle management, pipe-pane spawning |
| `src/detection/content.rs` | `content_above_status_bar()`, `has_input_prompt()`, `has_input_field()` |
| `src/monitor.rs` | Sidecar monitor process -- stream reader, ring buffer, status writer |
| `src/init.rs` | `claude-tmux init` -- detection method selection, hook installation, daemon service setup |
| `src/app/mod.rs` | `tick_status()`, Done lifecycle, `worked_unfocused`/`done_panes` sets, state file read/write, `write_status_file()` |
| `src/session.rs` | `ClaudeCodeStatus` enum: Idle, Working, Done, WaitingInput, Error, Unknown |
| `src/ui/mod.rs` | Ratatui rendering. Status colors: Working=Green, Done=Cyan, WaitingInput=Yellow, Idle=DarkGray, Unknown=Gray |
| `src/tmux.rs` | tmux command wrappers. `capture_pane()` captures last N lines of a pane |
| `src/settings.rs` | Settings file lookup and parsing (`detection_method`, `hook_staleness_secs`, etc.) |

## Files outside this repo

| File | Purpose |
|------|---------|
| `~/.tmux.conf.local` | Contains the `bind-key C` keybinding that launches claude-tmux |
| `~/.claude-tmux/settings.json` | User-level settings override (including `detection_method`) |
| `~/.claude-tmux/hooks/status.sh` | Hook script created by `claude-tmux init` (hooks backend only) |
| `~/.claude/settings.json` | Claude Code settings -- hooks backend registers event hooks here |
| `~/.claude/statusline-command.sh` | Claude Code statusline script that reads `/tmp/claude-tmux-status` |
| `/tmp/claude-tmux-status` | Session status counts, written by daemon mode |
| `/tmp/claude-tmux-state` | Per-pane Done/working-unfocused state, shared between daemon and popup |
| `/tmp/claude-tmux-hooks/` | Per-pane status files written by hooks and sidecar backends |

## Building

```bash
cargo test && cargo build --release
```

Binary: `target/release/claude-tmux`. The user's tmux keybinding already points here.

## Related

- [01-reference/keybindings.md](../01-reference/keybindings.md) -- all keybindings
- [01-reference/status-indicators.md](../01-reference/status-indicators.md) -- status symbols and colors
- [10-configuration/settings-file.md](../10-configuration/settings-file.md) -- settings file format
