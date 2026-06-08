# Decision: Hooks backend falls back to content detection

**Alternatives considered**: Return Unknown when no hook file exists; require daemon restart of all Claude sessions after init
**Chosen**: Fall back to `detect_from_content()` when hook file is missing or stale
**Why**: Sessions started before `claude-tmux init` never fire hooks. Making the user restart every Claude Code session is impractical. Content detection is good enough as a baseline — once the first hook event fires, the hook file takes over.

## Context

After running `claude-tmux init` and switching to hooks mode, all sessions showed `?` (Unknown). Only one pane had a hook file — the one where a new prompt was submitted after hooks were configured. The rest had no hook files and no way to get one without a lifecycle event.
