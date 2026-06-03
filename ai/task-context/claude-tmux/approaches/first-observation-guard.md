# Approach: First-observation guard on Idle + worked_unfocused only

**Verdict**: Adopted
**Why**: Prevents false Done flash on popup open while preserving correct Working/WaitingInput detection and Done persistence.

## What was tried

On the first observation of a pane (no entry in `pane_content_cache`):
- Raw status is detected normally via `detect_status(&content)` — Working and WaitingInput are classified correctly.
- If raw status is Idle AND the pane is not already in `done_panes`, the `worked_unfocused → Done` transition is suppressed. The pane stays Idle.
- If the pane IS in `done_panes` (restored from state file), the guard is skipped and Done is preserved.
- A fast second tick fires ~20ms later (by rewinding `last_status_tick`), providing real content-diff classification.

## What happened

All test cases passed:
- Opening popup while sessions are Working: shows Idle briefly, then Working on second tick (~20ms). No false Done flash.
- Opening popup after sessions finished: shows Done immediately (restored from state file, exempt from guard).
- Sessions finishing while popup is open: Working → Idle → Done transition works normally.
- Sessions finishing while popup is closed: daemon tracks the transition, state file preserves it, next popup open shows Done.

## Key takeaway

The guard is minimal and targeted: it only suppresses one specific transition (first-tick Idle triggering Done via stale worked_unfocused), while letting everything else through. The `done_panes` exemption is critical — without it, `apply_persisted_done()` sets Done on startup but tick 1 immediately overwrites it back to Idle.
