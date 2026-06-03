# Approach: Persist worked_unfocused only, not done_panes

**Verdict**: Rejected
**Why**: Without persisting `done_panes`, Done status disappeared on the second popup open. The popup re-derived Idle from the pane content, and without the `d:` entry in the state file, had no way to know the pane had already transitioned to Done.

## What was tried

Kept `w:` (worked_unfocused) entries in `/tmp/claude-tmux-state` but removed `d:` (done_panes) entries. The idea was that the popup could re-derive Done from `worked_unfocused + Idle` on startup.

## What happened

On second popup open, previously-Done sessions showed as Idle. The first-observation guard suppressed the `worked_unfocused → Done` transition on tick 1 (by design, to prevent false Done flash), so the re-derivation never happened.

## Key takeaway

Both `w:` and `d:` entries are needed in the state file. `worked_unfocused` tracks candidates; `done_panes` tracks confirmed transitions. The first-observation guard makes it impossible to re-derive Done from worked_unfocused alone.
