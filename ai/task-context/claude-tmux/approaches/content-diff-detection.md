# Approach: Content-diff for Working detection

**Verdict**: Rejected
**Why**: Any one-off content change (terminal resize, escape sequence drift, tmux re-render) triggered Working for ALL sessions simultaneously, causing a mass false-Done cascade within seconds.

## What was tried

Content-diff was the original Working detection mechanism: compare `content_above_status_bar()` of the current capture against the cached previous capture. If different → Working. If same → fall through to `detect_static_status()`.

An intermediate fix attempted to cross-check with `detect_static_status()` — if static said Idle/WaitingInput, treat the content change as cosmetic. But this still fell through to Working when static returned Unknown (e.g., after a resize scrambled the captured 15 lines).

## What happened

The daemon (150ms tick interval, 2s done_delay) would accumulate ALL panes in `done_panes` within 10 seconds of any terminal resize or similar event. The state file persisted this across daemon restarts and popup opens. Proved by:
1. Capturing the same idle pane 10 times at 200ms intervals — all hashes identical (content IS stable)
2. Clearing the state file — daemon repopulated 16 done entries within 2 seconds
3. After removing content-diff entirely — state file stayed empty for 30+ seconds

## Key takeaway

Static detection via "ctrl+c to interrupt" and "Effecting…" is sufficient and reliable. The ~150ms latency cost of not using content-diff is negligible compared to the false-positive risk.
