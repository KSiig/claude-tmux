# Settings file

claude-tmux uses a JSON settings file. No configuration is required -- defaults work out of the box.

## Settings file locations

Lookup order (first found wins):

1. **User override**: `~/.claude-tmux/settings.json`
2. **Repo default**: `settings.json` in the directory where the binary was compiled (`CARGO_MANIFEST_DIR`)

If neither file exists or is parseable, built-in defaults are used.

## Repo default example

Shipped in `settings.json` at the repo root. Intentionally simple:

```json
{
  "status_interval_ms": 500,
  "show_git_info": true,
  "session_status_labels": true,
  "grouping": false
}
```

## Power-user example

`~/.claude-tmux/settings.json` with grouping and Linear integration:

```json
{
  "status_interval_ms": 150,
  "show_git_info": false,
  "session_status_labels": false,
  "grouping": true,
  "task_integration": {
    "provider": "linear",
    "issue_prefix": "VEL",
    "poll_interval_ms": 10000,
    "show_titles": true,
    "show_status": true,
    "status_labels": false
  }
}
```

## Top-level options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `status_interval_ms` | integer | `500` | Milliseconds between status detection ticks. Min: `20` (values below are clamped). |
| `show_git_info` | boolean | `true` | Show git branch and dirty indicators in the session list. |
| `session_status_labels` | boolean | `true` | Show text labels next to session status icons (e.g. `* working` vs just `*`). |
| `grouping` | boolean | `false` | Group sessions by shared name prefix (e.g. `VEL-420` and `VEL-420-556-ci` group together). |
| `exclude_sessions` | array of strings | `[]` | Glob patterns for session names to hide from the list. Supports `*` and `?` wildcards. |
| `task_integration` | object or null | `null` | Optional task tracker integration. See [`task_integration` options](#task_integration-options) below. |

### `status_interval_ms`

How frequently claude-tmux captures pane content and runs status detection. Lower values mean faster detection of Working/Done transitions but use more CPU for tmux pane captures.

- Default `500` ms is a good balance for normal use.
- Minimum enforced at `20` ms (values below are clamped to 20).
- The first-observation guard fires a second tick ~20 ms after the first regardless of this setting, so initial classification is always fast.

### `grouping`

Groups sessions that share a name prefix into collapsible groups in the session list.

Grouping is **independent of task integration**. Sessions group by shared name prefix even without any API connection. Adding `task_integration` enriches the groups with titles and status indicators, but is not required.

### `exclude_sessions`

Hides tmux sessions whose name matches any of the given glob patterns. Useful for hiding helper sessions (e.g. sessions spawned by `/flush`).

```json
{
  "exclude_sessions": ["flush*", "scratch"]
}
```

Supported wildcards: `*` (any sequence of characters), `?` (any single character). Patterns are matched against the full session name.

## `task_integration` options

When `task_integration` is present, claude-tmux fetches issue metadata from an external task tracker and displays it alongside session groups.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `provider` | string | *(required)* | Task tracker provider. Currently only `"linear"` is supported. |
| `issue_prefix` | string or null | `null` | Only extract identifiers matching this prefix (e.g. `"VEL"`). When null, matches any `WORD-NUMBER` pattern. |
| `poll_interval_ms` | integer | `10000` | Milliseconds between API polls. Min: `1000` (values below are clamped). |
| `show_titles` | boolean | `true` | Show task titles on group headers and child session rows. |
| `show_status` | boolean | `true` | Show task status indicators (colored symbols). |
| `status_labels` | boolean | `false` | Show text labels next to task status icons (e.g. `In Progress` vs just the symbol). |

### Daemon requirement

Task integration requires the **headless daemon** (`claude-tmux --headless`) running in the background. The daemon polls the API and writes a cache file at `/tmp/claude-tmux-linear.json`. The popup reads this cache. Without the daemon running, task status and titles from the API will not appear.

See [headless-mode.md](../15-daemon/headless-mode.md) for daemon setup.

### Linear setup

1. Set the `LINEAR_API_KEY` environment variable in the shell where the daemon runs. The API key is **not** stored in `settings.json`.
2. Set `"provider": "linear"` in `task_integration`.
3. Optionally set `"issue_prefix"` to limit matching to your team's identifier prefix.

## Titles file

Task titles can also come from a manual titles file at `~/.claude-tmux/titles.json`, independent of API integration. This file maps session name prefixes to human-readable titles:

```json
{
  "VEL-420": "Self-hosted runner",
  "VEL-418": "Multi-AZ Kubernetes"
}
```

When both the manual titles file and API-fetched titles are available, both sources are used. The titles file is loaded on each tick when `show_titles` is enabled.

## File locations summary

| File | Path | Purpose |
|------|------|---------|
| User settings | `~/.claude-tmux/settings.json` | User overrides |
| Repo settings | `<repo>/settings.json` | Defaults shipped with the repo |
| Titles file | `~/.claude-tmux/titles.json` | Manual session-prefix-to-title mapping |
| Status file | `/tmp/claude-tmux-status` | Session counts, written by daemon only. See [headless-mode.md](../15-daemon/headless-mode.md). |
| State file | `/tmp/claude-tmux-state` | Done/working-unfocused pane IDs, persisted across popup sessions |
| Linear cache | `/tmp/claude-tmux-linear.json` | Cached Linear issue data, written by daemon |

## Related

- [../15-daemon/headless-mode.md](../15-daemon/headless-mode.md) -- headless daemon mode that writes the status and Linear cache files
- [../20-status-detection/detection-method.md](../20-status-detection/detection-method.md) -- how the status tick interval affects detection
