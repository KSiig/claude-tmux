# Status indicators

Symbols and colors displayed next to each session in the claude-tmux TUI.

## Status table

| Symbol | Status | Color | Meaning |
|--------|--------|-------|---------|
| `●` | Working | Green | Claude is actively processing |
| `◉` | Done | Cyan | Finished while you were in another session |
| `◐` | WaitingInput | Yellow | Permission, plan approval, or hook prompt |
| `○` | Idle | Dark gray | Ready for input |
| `?` | Unknown | Gray | Not a Claude Code session or status unclear |

## Source types

| Type | Defined in |
|------|------------|
| `ClaudeCodeStatus` enum | `src/session.rs` |
| Color assignments | `src/ui/mod.rs` |

## Related

- [keybindings.md](keybindings.md) -- keyboard shortcuts
- [../20-status-detection/detection-method.md](../20-status-detection/detection-method.md) -- how each status is detected
- [../20-status-detection/done-lifecycle.md](../20-status-detection/done-lifecycle.md) -- how Done transitions work
