# Approach: Token-line active-phase detection

**Verdict**: Adopted
**Why**: Claude Code uses many different working-phase verbs (Sketching, Deploying, Effecting, Gesticulating, etc.). Matching individual verbs is fragile. All active phases share a common pattern: `… (Xm Xs · ↑ Xk tokens)`.

## What was tried

Updated `is_active_tool_phase()` to check each line for the combination of `…` (U+2026 ellipsis) and `tokens)`. This catches any active operation regardless of the verb. Also kept the existing `✢` (U+2722) check.

## What happened

The Sketching phase (which was previously misclassified as Idle) is now correctly detected as Working. All future working-phase names are automatically covered.

## Key takeaway

Match on the structural pattern (`… + tokens)`) not the content verb. Claude Code randomizes the verb.
