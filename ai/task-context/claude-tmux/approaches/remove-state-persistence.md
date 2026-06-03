# Approach: Remove state file persistence entirely

**Verdict**: Rejected
**Why**: Without persisting `worked_unfocused` between popup sessions, the popup has no way to know which sessions were Working while it was closed. Done detection only worked within a single popup session.

## What was tried

Removed `read_state_file()` call from `App::new()`, initialized both `worked_unfocused` and `done_panes` as empty HashSets.

## What happened

Done status never appeared when opening the popup after a session finished — the popup had no history of which sessions had been working.

## Key takeaway

The state file is essential for bridging the gap between popup sessions. The daemon writes it while running, and the popup reads it on open.
