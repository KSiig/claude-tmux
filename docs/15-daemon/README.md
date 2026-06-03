# Daemon

Headless daemon mode for continuous background monitoring.

| Page | Use it for |
|------|------------|
| [headless-mode.md](headless-mode.md) | Starting the daemon, what it does, status/state file formats, statusline integration |

## Scope

Daemon-specific behavior only: how to run it, what files it writes, daemon-vs-popup differences. General status detection logic lives in [20-status-detection/](../20-status-detection/).

## Related

- [10-configuration/settings-file.md](../10-configuration/settings-file.md) -- `status_interval_ms` setting
- [20-status-detection/](../20-status-detection/) -- detection method and Done lifecycle
