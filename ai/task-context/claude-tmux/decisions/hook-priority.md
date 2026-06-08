# Decision: Hooks as primary signal, parsing as fallback

**Alternatives considered**: hooks-only (no parsing), parsing-only (current), hooks with immediate override
**Chosen**: Hooks win unless parsing disagrees for >5 seconds
**Why**: Hooks are authoritative but require user setup (`claude-tmux init`). Parsing works out of the box. The hybrid gives reliability with hooks and graceful degradation without.

## Context

Pane-capture parsing is inherently fragile — transient content causes flickering. Hooks provide clean lifecycle boundaries but require configuring Claude Code's hooks system. Making hooks optional and layering them on top means:

1. Zero setup still works (parsing-only, same as before)
2. `claude-tmux init` adds hooks for stability
3. If Claude crashes and the hook file goes stale, parsing corrects after 5s
4. The 5s threshold is configurable via `hook_override_delay_ms` in settings
