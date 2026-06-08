# Headless daemon mode

The headless daemon is a long-running background process that continuously monitors all tmux sessions for Claude Code status changes. It writes status counts to a file that external consumers (statusline scripts, tmux status bars) can read.

## Starting the daemon

The recommended way to install and start the daemon is via `claude-tmux init`, which sets up a system service (launchd on macOS, systemd on Linux) that starts automatically on login.

To start manually:

```bash
claude-tmux --headless
# or
claude-tmux -d
```

Runs indefinitely until killed.

## What the daemon does

On each tick (default every 500 ms, configurable via `status_interval_ms` in [settings-file.md](../10-configuration/settings-file.md)):

1. Re-lists all tmux sessions (picks up newly created sessions).
2. Runs status detection via the configured backend (process, hooks, or sidecar -- see [detection-method.md](../20-status-detection/detection-method.md)).
3. Tracks Done transitions for unfocused panes.
4. Optionally polls Linear API for task titles/statuses.
5. Writes `/tmp/claude-tmux-status` with aggregate counts.
6. Writes `/tmp/claude-tmux-state` with per-pane Done/working-unfocused state.

## Daemon vs popup

| Behavior | Popup (default) | Daemon (`--headless`) |
|----------|----------------|----------------------|
| Writes `/tmp/claude-tmux-status` | No | Yes |
| Writes `/tmp/claude-tmux-state` | Yes | Yes |
| Considers attached session as "focused" | No (user is viewing popup overlay) | Yes (attached session is directly visible) |
| Re-lists sessions each tick | No (manual refresh only) | Yes |
| Renders UI | Yes | No |

The popup does not write the status file because it is short-lived. The daemon is the canonical source of status counts.

Both popup and daemon read/write the state file (`/tmp/claude-tmux-state`), which persists Done pane IDs across popup open/close cycles.

## Status file format

`/tmp/claude-tmux-status` contains key=value pairs, one per line:

```
working=2
done=1
idle=3
waiting=0
unknown=1
total=7
```

All values are integers. `total` is the total number of tmux sessions (including non-Claude sessions).

## State file format

`/tmp/claude-tmux-state` contains pane IDs with status prefixes, one per line:

```
w:%5
d:%3
d:%8
```

- `w:<pane_id>` -- pane was Working while unfocused (candidate for Done transition)
- `d:<pane_id>` -- pane has transitioned to Done

Stale pane IDs (from killed sessions) are pruned on each tick.

## Statusline integration example

A shell script that reads the status file for a tmux statusline or Claude Code statusline:

```bash
#!/bin/bash
STATUS_FILE="/tmp/claude-tmux-status"
if [ ! -f "$STATUS_FILE" ]; then
    echo ""
    exit 0
fi

working=$(grep '^working=' "$STATUS_FILE" | cut -d= -f2)
done_count=$(grep '^done=' "$STATUS_FILE" | cut -d= -f2)
waiting=$(grep '^waiting=' "$STATUS_FILE" | cut -d= -f2)

parts=()
[ "$working" -gt 0 ] 2>/dev/null && parts+=("${working}w")
[ "$done_count" -gt 0 ] 2>/dev/null && parts+=("${done_count}d")
[ "$waiting" -gt 0 ] 2>/dev/null && parts+=("${waiting}?")

if [ ${#parts[@]} -gt 0 ]; then
    echo "[$(IFS=/; echo "${parts[*]}")]"
fi
```

## Related

- [../10-configuration/settings-file.md](../10-configuration/settings-file.md) -- `status_interval_ms` controls the tick rate
- [../20-status-detection/detection-method.md](../20-status-detection/detection-method.md) -- how status classification works
- [../20-status-detection/done-lifecycle.md](../20-status-detection/done-lifecycle.md) -- Done transitions and "focused" concept
