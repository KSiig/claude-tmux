# Approach: Detect "Effecting…" as working indicator

**Verdict**: Adopted
**Why**: During the "Effecting…" phase (Claude applying tool results), "ctrl+c to interrupt" scrolls off the 15-line capture window but the `❯` prompt is still visible. `detect_static_status` was returning Idle, which triggered a false Working→Done transition.

## What was tried

Added `is_active_tool_phase()` to `detection.rs` that checks for "Effecting\u{2026}" (Unicode horizontal ellipsis). Inserted before the `has_input_field` check in both `detect_status` and `detect_static_status`.

## What happened

Correctly identifies the tool-application phase as Working. User captured a screenshot showing the exact state: "✢ Effecting… (34s · ↑ 1.1k tokens)" with the prompt visible but no "ctrl+c to interrupt" in the last 15 lines.

## Key takeaway

Claude Code uses many randomized phase names ("Gesticulating…", "Crunching…", etc.) — only "Effecting…" is detected by text match. The idle-hold delay provides a safety net for unrecognized phase names.
