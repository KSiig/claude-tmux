# Approach: Force Idle on first tick for all statuses

**Verdict**: Rejected
**Why**: Forcing `None => ClaudeCodeStatus::Idle` in the match arm meant Working was never detected on the first tick. Combined with `detect_static_status()` lacking Working detection at the time, sessions running static operations (like `sleep`) stayed Idle permanently.

## What was tried

Changed the `None` (no previous content) match arm from `detect_status(&content)` to `ClaudeCodeStatus::Idle`, so every pane starts as Idle on its first observation.

## What happened

Ran `sleep 5` in a session. Status stayed Idle the entire time and never transitioned to Working. Two compounding issues:

1. First tick forced Idle, so the pane was never added to `worked_unfocused`.
2. On subsequent ticks, content hadn't changed (sleep produces no output), so `detect_static_status()` was called — but it had no Working detection (no "ctrl+c to interrupt" check).

## Key takeaway

The first-observation guard must be selective: suppress only the `worked_unfocused → Done` transition, not the raw status detection. Working and WaitingInput must still be detected accurately from first-tick content.
