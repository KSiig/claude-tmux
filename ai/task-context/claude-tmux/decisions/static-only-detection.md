# Decision: Drop content-diff, use static-only detection

**Alternatives considered**: content-diff with static cross-check, consecutive-change requirement, ANSI stripping before comparison
**Chosen**: Remove content-diff entirely, use `detect_static_status()` as sole authority
**Why**: Content-diff was inherently fragile — any one-off change (resize, escape drift, tmux re-render) triggered false Working. All alternatives added complexity without eliminating the fundamental fragility. Static detection reliably identifies Working via "ctrl+c to interrupt" and "Effecting…" with only ~150ms latency cost.

## Context

Verified by capturing the same idle pane 10 times at 200ms intervals — all hashes were identical. The daemon's 150ms tick interval should never see content changes for idle panes under normal conditions. But a single terminal resize changes ALL panes simultaneously (escape sequences differ, line wrapping changes with `-J`). This caused 16+ panes to enter `done_panes` within 2 seconds via the content-diff → Working → done_delay → Done path.

The `content_above_status_bar()` function and `pane_content_cache` are retained but no longer used in `tick_status()`. The cache still stores captures for potential future use.
