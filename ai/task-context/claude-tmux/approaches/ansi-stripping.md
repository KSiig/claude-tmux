# Approach: ANSI stripping for content detection

**Verdict**: Adopted
**Why**: tmux `capture-pane -e` inserts ANSI color codes that split text across boundaries, making `contains()` pattern matching unreliable. Example: "Sketching…" becomes `\x1b[174mSketchi\x1b[216mng…`.

## What was tried

Added `strip_ansi()` to `src/detection/content.rs` (moved from `monitor.rs` where it was already used for sidecar stream processing). Called at the top of `detect_from_content()` before any pattern matching.

## What happened

All content patterns now match reliably regardless of how tmux colorizes the output. The monitor module reuses the same function via `use crate::detection::content::strip_ansi`.

## Key takeaway

Any text pattern matching on tmux capture output must strip ANSI first. The `-e` flag is needed for preview rendering but poisons `contains()` checks.
