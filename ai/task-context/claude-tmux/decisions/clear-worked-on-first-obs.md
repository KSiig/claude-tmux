# Decision: Clear worked_unfocused on first observation if Idle

**Alternatives considered**: Don't persist worked_unfocused at all; add timestamps to worked_unfocused entries; expire entries after N minutes
**Chosen**: Remove worked_unfocused entries during first observation if the pane is currently Idle
**Why**: Simplest fix that handles the exact failure mode. If we have no content diff history for a pane (first observation), we can't confirm the Working→Idle transition just happened — so we shouldn't trigger Done.

## Context

After the daemon hung for 7 hours (blocked on Linear API), the state file had several panes in `worked_unfocused`. When the new daemon started:
1. First tick: first-observation guard suppressed Done but kept worked_unfocused entries
2. Second tick: content unchanged → Idle → worked_unfocused → Done for all stale entries
3. User saw ~6 sessions flip to Done simultaneously

The fix adds `self.worked_unfocused.remove(&pane_id)` inside the first-observation guard branch. If a pane is Idle on first observation, we missed the transition — remove it from tracking rather than converting it to Done on the next tick.
