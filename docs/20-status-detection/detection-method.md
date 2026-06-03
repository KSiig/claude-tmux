# Detection method

claude-tmux detects Claude Code status by capturing the last 15 lines of each Claude Code pane and analyzing the content. Detection runs on a configurable tick interval (default 500 ms, set via `status_interval_ms` in [settings-file.md](../10-configuration/settings-file.md)).

## Two detection mechanisms

1. **Content-diff detection**: Compare the current pane capture against the previous one. If the content above the status bar changed, the session is **Working**.
2. **Static content analysis**: When content has not changed, inspect text patterns to classify as **Idle**, **WaitingInput**, **Working** (from `ctrl+c to interrupt`), or **Unknown**.

Content-diff is the primary signal for Working. Static analysis handles everything else.

## Status bar stripping

Claude Code renders a status bar below the input prompt (showing model name, cost, elapsed time, etc.). This status bar updates every second, which would cause false Working detection if compared directly.

`content_above_status_bar()` in `src/detection.rs` finds the input prompt boundary -- a line containing `❯` with a `─` border line directly above it -- and returns only the content up to and including the prompt line. Everything below (status bar lines) is excluded from content comparison.

If no prompt boundary is found, the full content is returned unchanged.

## Pattern matching

### Input prompt detection (`has_input_field`)

A Claude Code input field is detected when a line contains `❯` and the line directly above it contains `─` (the border).

### WaitingInput patterns (`has_input_prompt`)

Any of these strings in the pane content triggers WaitingInput:

- `[y/n]` -- permission prompts
- `[Y/n]` -- permission prompts (default yes)
- `shift+tab to approve` -- plan approval and selection prompts
- `Esc to cancel` -- hook and tool confirmation prompts

WaitingInput takes priority over all other statuses.

### Working detection (static)

When content has not changed but `ctrl+c` and `to interrupt` are both present in the content, the session is classified as Working. This catches cases where Claude is processing but the visible output hasn't changed yet.

## Classification summary

| Condition | Status |
|-----------|--------|
| Content contains WaitingInput pattern | WaitingInput |
| Input field present + `ctrl+c to interrupt` | Working |
| Input field present, no interrupt message | Idle |
| No input field + `ctrl+c to interrupt` | Working |
| None of the above | Unknown |

When content-diff detection is available (not the first observation), content change overrides the above and forces Working.

## Key source files

| File | Contains |
|------|----------|
| `src/detection.rs` | `content_above_status_bar()`, `detect_status()`, `detect_static_status()`, `has_input_prompt()`, `has_input_field()` |
| `src/session.rs` | `ClaudeCodeStatus` enum (Idle, Working, Done, WaitingInput, Unknown) |

## Related

- [done-lifecycle.md](done-lifecycle.md) -- how Done transitions work (not detected from pane content)
- [../01-reference/status-indicators.md](../01-reference/status-indicators.md) -- symbols and colors for each status
- [../10-configuration/settings-file.md](../10-configuration/settings-file.md) -- `status_interval_ms` controls tick rate
- [../15-daemon/headless-mode.md](../15-daemon/headless-mode.md) -- daemon vs popup detection differences
