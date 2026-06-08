# Approach: Parsed-status drives Done lifecycle, hooks drive display

**Verdict**: Adopted
**Why**: Hooks provide fast display updates but can be stale (Stop hook slow or missing). Using hooks for lifecycle tracking caused false Working → Done transitions.

## What was tried

Changed `worked_unfocused` and `done_panes` tracking to use `parsed_status` instead of `raw_status`. The `raw_status` (which incorporates hook data with override logic) is still used for the DISPLAYED status. This splits the concern:
- Display: hooks provide responsive Working/Idle/WaitingInput feedback
- Lifecycle: only what's actually visible in the pane (prompt? interrupt hint?) drives the Working→Idle→Done state machine

## What happened

With this change, a stale `UserPromptSubmit` hook stuck on "working" no longer accumulates `worked_unfocused` entries. The session's display may briefly show Working (from the hook, until the 5s override), but it won't trigger a Done transition unless `parsed_status` independently confirms Working via "ctrl+c to interrupt" or "Effecting…".

## Key takeaway

Hooks are advisory for display, not authoritative for lifecycle. The pane's visible content is the ground truth for Working→Done tracking.
