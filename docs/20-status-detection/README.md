# Status detection

How claude-tmux detects and classifies Claude Code session statuses. Three pluggable backends are available; run `claude-tmux init` to choose one and install the daemon.

| Page | Use it for |
|------|------------|
| [detection-method.md](detection-method.md) | Detection backends (process, hooks, sidecar), content analysis, pattern matching, classification summary |
| [done-lifecycle.md](done-lifecycle.md) | How panes become Done, how Done clears, persistence, "focused" concept, first-observation guard |

## Scope

Detection algorithms and state transitions only. For the status symbols/colors displayed in the UI, see [01-reference/status-indicators.md](../01-reference/status-indicators.md). For how the tick rate is configured, see [10-configuration/settings-file.md](../10-configuration/settings-file.md).

## Related

- [01-reference/status-indicators.md](../01-reference/status-indicators.md) -- status symbols and colors
- [10-configuration/settings-file.md](../10-configuration/settings-file.md) -- tick interval and detection settings
- [15-daemon/headless-mode.md](../15-daemon/headless-mode.md) -- daemon vs popup behavior differences
