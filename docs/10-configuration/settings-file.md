# Settings file

claude-tmux uses a JSON settings file. No configuration is required -- defaults work out of the box.

## Settings file locations

Lookup order (first found wins):

1. **User override**: `~/.claude-tmux/settings.json`
2. **Repo default**: `settings.json` in the directory where the binary was compiled (`CARGO_MANIFEST_DIR`)

If neither file exists or is parseable, built-in defaults are used.

## Settings format

```json
{
  "status_interval_ms": 500
}
```

## Options

| Key | Type | Default | Min | Description |
|-----|------|---------|-----|-------------|
| `status_interval_ms` | integer | `500` | `20` | Milliseconds between status detection ticks. Controls how often pane content is captured and compared. |

### `status_interval_ms`

How frequently claude-tmux captures pane content and runs status detection. Lower values mean faster detection of Working/Done transitions but use more CPU for tmux pane captures.

- Default `500` ms is a good balance for normal use.
- Minimum enforced at `20` ms (values below are clamped to 20).
- The first-observation guard fires a second tick ~20 ms after the first regardless of this setting, so initial classification is always fast.

## File locations summary

| File | Path | Purpose |
|------|------|---------|
| User settings | `~/.claude-tmux/settings.json` | User overrides |
| Repo settings | `<repo>/settings.json` | Defaults shipped with the repo |
| Status file | `/tmp/claude-tmux-status` | Session counts, written by daemon only. See [headless-mode.md](../15-daemon/headless-mode.md). |
| State file | `/tmp/claude-tmux-state` | Done/working-unfocused pane IDs, persisted across popup sessions |

## Related

- [../15-daemon/headless-mode.md](../15-daemon/headless-mode.md) -- headless daemon mode that writes the status file
- [../20-status-detection/detection-method.md](../20-status-detection/detection-method.md) -- how the status tick interval affects detection
