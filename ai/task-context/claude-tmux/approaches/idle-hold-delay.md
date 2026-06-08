# Approach: Idle-hold delay before Done transition

**Verdict**: Adopted
**Why**: A single Idle tick during active work (e.g., two content captures landing in the same second of "Effecting… (34s)") was enough to trigger the Working→Done transition. Requiring sustained Idle prevents false positives from brief content pauses.

## What was tried

Added `idle_since: HashMap<String, Instant>` and `done_delay: Duration` (default 2s, configurable via `done_delay_ms`) to App. The worked_unfocused→Done transition now requires the pane to remain Idle for `done_delay` before completing. The timer resets if the pane goes back to Working.

## What happened

Works as a safety net for detection gaps. Even when text-based detection misses a working phase (e.g., "Gesticulating…" not in the check list), the 2-second hold prevents immediate false Done.

## Key takeaway

Text-based detection is fragile (Claude Code changes UI strings). The idle-hold delay is the robust backstop — it doesn't depend on recognizing specific strings.
