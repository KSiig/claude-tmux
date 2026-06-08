# Decision: Install daemon as launchd/systemd service

**Alternatives considered**: Manual `nohup ... &` in shell profile; tmux-based auto-start; cron @reboot
**Chosen**: Native service manager (launchd on macOS, systemd on Linux)
**Why**: KeepAlive/Restart handles crashes automatically. Re-running `claude-tmux init` reloads the service, handling binary upgrades. No shell profile pollution.

## Context

The daemon previously required manual `nohup claude-tmux --headless &` after each reboot or crash. The user assumed `claude-tmux init` started the daemon, but it only configured hooks. When the daemon hung for 7 hours on a blocked HTTP call, there was no auto-restart.

Implementation:
- **macOS**: `~/Library/LaunchAgents/com.claude-tmux.daemon.plist` with `RunAtLoad` + `KeepAlive`. Loaded via `launchctl load`.
- **Linux**: `~/.config/systemd/user/claude-tmux.service` with `Restart=on-failure`, `RestartSec=5`. Enabled via `systemctl --user enable --now`.

Both capture the current `PATH` (so tmux is findable) and `LINEAR_API_KEY` if set. Logs go to `/tmp/claude-tmux-daemon.log`. Re-running init unloads/reloads the service to pick up binary changes.
