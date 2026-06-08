# Approach: Tag-based pane exclusion

**Verdict**: Adopted
**Why**: `/flush` creates a second Claude pane in the same tmux session via `split-window`. Session-name exclusion can't distinguish between the fresh and stale panes since they share the same session name. A tmux user option on the pane itself is the only way to target individual panes.

## What was tried

1. First attempted `exclude_sessions` glob patterns in settings.json — works for hiding entire sessions by name but doesn't solve the flush case since both panes are in the same session.
2. Added `@claude-tmux-exclude` tmux pane option. `list_panes()` format string reads `#{@claude-tmux-exclude}`. Non-empty, non-zero values cause the pane to be filtered out when building Session rows.
3. Updated `/flush` skill to run `tmux set-option -p @claude-tmux-exclude 1` on the old pane before `split-window`.

## What happened

Works correctly. `tmux list-panes -F '#{@claude-tmux-exclude}'` returns empty string for unset panes and "1" for tagged panes. The tag persists for the lifetime of the pane.

## Key takeaway

tmux user options (`@name`) are per-pane when set with `-p` and readable in format strings — a clean way to attach metadata to panes without external state files.
