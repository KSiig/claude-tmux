# Approach: Hooks with content fallback

**Verdict**: Adopted
**Why**: After switching to hooks via `claude-tmux init`, all sessions showed Unknown (`?`) because no hook files existed yet. Sessions started before hooks were configured never fire hooks.

## What was tried

Changed `HooksBackend::detect()` to fall back to `content::detect_from_content()` when `read_hook_status()` returns Unknown (no file or stale file). Added `needs_content() -> true` so the backend receives pane content.

## What happened

Fresh starts now show proper Idle/Working/WaitingInput from content analysis. Once a hook event fires, the hook file takes priority.

## Key takeaway

Event-driven backends need a fallback for initial state — they can't know what happened before they started listening.
