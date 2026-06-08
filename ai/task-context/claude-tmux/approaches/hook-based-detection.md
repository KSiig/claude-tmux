# Approach: Hook-based status detection

**Verdict**: Adopted
**Why**: Claude Code hooks provide authoritative lifecycle signals, eliminating flickering caused by transient pane content misreads. Parsing stays as fallback for unconfigured sessions.

## What was tried

Layered Claude Code hook signals on top of existing pane-capture parsing:
- Hooks write `<status> <timestamp>` to `/tmp/claude-tmux-hooks/<pane_id>`
- `UserPromptSubmit` → working, `Stop` → idle, `StopFailure` → error, `PermissionRequest`/`Elicitation` → waiting_input
- `tick_status()` reads hook file first, falls back to parsing if absent
- If hook and parsing disagree, hook wins for 5 seconds; after that, parsing takes over (handles crashed sessions)
- `claude-tmux init` subcommand creates the hook script and wires it into `~/.claude/settings.json`

New `Error` status variant added for `StopFailure` (red ✕ icon).

## What happened

Clean implementation, all tests pass. Hook resolution integrates between the existing parsed status computation and the Done-transition logic, so `worked_unfocused`/`done_panes` continue to work unchanged.

## Key takeaway

The hook approach is strictly additive — no setup required for basic functionality, `init` is an opt-in upgrade. The 5-second disagree threshold is the key design parameter for balancing hook authority with crash recovery.
