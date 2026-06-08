# Decision: Parsed drives lifecycle, hooks drive display

**Alternatives considered**: shorter hook override delay, don't update worked_unfocused during disagree period, require hook+parsed agreement for lifecycle
**Chosen**: Use `parsed_status` for `worked_unfocused`/`done_panes` tracking, `raw_status` for displayed session status
**Why**: Clean separation of concerns. Hooks are fast but unreliable for lifecycle (Stop hook may not fire, UserPromptSubmit fires before UI updates). Parsed detection is slower but ground-truth.

## Context

The `UserPromptSubmit` hook writes "working" immediately when the user submits. Claude processes the request. The `Stop` hook should write "idle" when done. If Stop is slow or doesn't fire, the hook stays at "working" and the `hook_override_delay` (5s) must elapse before the daemon switches to parsed_status.

During that 5s window, `raw_status=Working` (from hook) while `parsed_status=Idle` (prompt visible). Previously, `raw_status` drove `worked_unfocused`, so the pane would enter worked_unfocused from the stale hook, then transition to Done when the override kicked in. With the split, `parsed_status=Idle` means `worked_unfocused` is never set from the stale hook.
