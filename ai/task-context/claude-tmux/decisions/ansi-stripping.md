# Decision: Strip ANSI before content detection

**Alternatives considered**: Capture without `-e` flag for detection; use regex matching that ignores ANSI
**Chosen**: `strip_ansi()` at the top of `detect_from_content()`
**Why**: The `-e` flag is needed because the same capture function serves both preview (needs ANSI for rendering) and detection. Stripping is simpler and more robust than regex alternatives. The function already existed in `monitor.rs` — moved to `content.rs` and shared.

## Context

Discovered when `cloudsim-seo-questions` showed Idle while clearly in Sketching phase. The raw capture showed `\x1b[174mSketchi\x1b[216mng…` — the word "Sketching" split across two ANSI color boundaries, so `content.contains("Sketching")` returned false.
