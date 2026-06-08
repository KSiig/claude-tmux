# Approach: Use hook file timestamp for staleness check

**Verdict**: Adopted
**Why**: The popup is ephemeral — each open creates a fresh `App` with a fresh `parse_disagree_since` map. The 5-second in-memory disagree timer could never accumulate across short popup sessions. Using the hook file's own Unix timestamp gives immediate staleness detection.

## What was tried

In `tick_status`, when hook status disagrees with parsing, compute the hook file's age from its embedded timestamp (`working 1780576193`). If the age exceeds `hook_override_delay`, trust parsing immediately without waiting for the in-memory timer.

The in-memory `parse_disagree_since` timer is kept as a secondary mechanism for recent hooks (where the timestamp is within the threshold but the status is wrong — e.g., a hook fires just before Claude finishes).

## What happened

Fixes the stuck-Working issue in popup mode. A stale hook from minutes ago is immediately overridden on the first tick. The daemon also benefits when restarting with stale hook files.

## Key takeaway

Any state that needs to survive across popup sessions must be persisted to a file. In-memory timers reset on every popup open.
