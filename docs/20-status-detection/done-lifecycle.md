# Done lifecycle

Done is not detected from pane content. It is a state transition tracked by the application. This page covers how panes enter and exit Done status, how state persists, and what "focused" means.

## How a pane becomes Done

1. A pane is detected as **Working** while the user is not focused on that session.
2. The pane ID is added to the `worked_unfocused` set (in `src/app/mod.rs`).
3. On a subsequent tick, if the pane transitions to **Idle** and is in the `worked_unfocused` set:
   - The pane ID moves from `worked_unfocused` to `done_panes`.
   - The session is displayed as **Done** (cyan `◉`).

The transition path is: Working (unfocused) -> Idle -> Done.

## How Done clears

- **Switching to the session**: When the user selects a Done session and switches to it, the pane is removed from `done_panes` and `worked_unfocused`, and the status reverts to Idle.
- **Focused in daemon mode**: If the daemon detects the session is the currently attached session, Done and worked_unfocused state is cleared automatically.

## What "focused" means

- **Daemon mode**: The attached tmux session is considered focused. Done tracking does not apply to it (the user can already see it).
- **Popup mode**: No session is considered focused, because the user is viewing the popup overlay, not any specific session. All sessions are eligible for Done tracking.

This difference means the daemon will clear Done for the attached session, while the popup will not.

## Persistence across popup sessions

Done state is persisted to `/tmp/claude-tmux-state` on every tick. When a new popup instance starts, it reads this file and restores Done status for panes that are currently Idle. This means closing and reopening the popup does not lose Done indicators.

The headless daemon also reads and writes this file, so Done state is shared between daemon and popup instances.

### State file format

`/tmp/claude-tmux-state` contains pane IDs with status prefixes, one per line:

```
w:%5
d:%3
d:%8
```

- `w:<pane_id>` -- pane was Working while unfocused (candidate for Done transition)
- `d:<pane_id>` -- pane has transitioned to Done

Stale pane IDs (from killed sessions) are pruned on each tick.

## First-observation guard

On the very first tick after startup (or after a session is first seen), there is no previous content to compare against. Without mitigation, the initial `detect_status()` call might classify a pane as Idle, which combined with a stale `worked_unfocused` entry could cause a false Done flash.

The guard works as follows:

- On the first observation of a pane, if the raw status is Idle and the pane is not already in `done_panes`, the status is kept as Idle without triggering the `worked_unfocused` to Done transition.
- Working and WaitingInput are still detected correctly from static content on the first observation.
- A second tick fires ~20 ms after the first (by rewinding `last_status_tick`), providing fast real classification with content-diff data.

This prevents the popup from briefly flashing Done on sessions that are actually Idle when it first opens.

## Key source files

| File | Contains |
|------|----------|
| `src/app/mod.rs` | `tick_status()`, `worked_unfocused`/`done_panes` sets, state file read/write |
| `src/session.rs` | `ClaudeCodeStatus::Done` variant |

## Related

- [detection-method.md](detection-method.md) -- how Working/Idle/WaitingInput are detected (upstream of Done)
- [../15-daemon/headless-mode.md](../15-daemon/headless-mode.md) -- daemon vs popup "focused" behavior
- [../01-reference/status-indicators.md](../01-reference/status-indicators.md) -- Done symbol and color
