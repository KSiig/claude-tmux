# Keybindings

All keyboard shortcuts for the claude-tmux popup TUI.

## tmux binding

Add to `~/.tmux.conf` to launch the popup:

```bash
bind-key C display-popup -E -w 80 -h 30 "/path/to/claude-tmux"
```

`-E` closes popup on exit. `-w`/`-h` set dimensions.

## Normal mode

| Key | Action |
|-----|--------|
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `l` / `Right` | Open action menu for selected session |
| `Enter` | Switch to selected session |
| `n` | Create new session |
| `K` | Kill selected session (with confirmation) |
| `r` | Rename selected session |
| `/` | Filter sessions by name/path |
| `Ctrl+c` | Clear filter |
| `R` | Refresh session list |
| `?` | Show help |
| `q` / `Esc` | Quit |

## Action menu

| Key | Action |
|-----|--------|
| `j` / `Down` | Next action |
| `k` / `Up` | Previous action |
| `Enter` / `l` / `Right` | Execute action |
| `h` / `Left` / `Esc` | Back to session list |
| `q` | Quit |

Available actions depend on context: switch, rename, kill, git stage/commit/push/pull/fetch, new worktree, create/view/close/merge PR.

## Related

- [status-indicators.md](status-indicators.md) -- status symbols displayed next to sessions
- [00-start-here/repo-map.md](../00-start-here/repo-map.md) -- full repo layout
