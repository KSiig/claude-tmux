# Decision: Hooks as sole authority (no parsing fallback)

**Verdict**: Superseded — hooks now fall back to content detection when no hook file exists

**Alternatives considered**: hooks + parsing hybrid (current), parsing-only, hooks with parsing tiebreaker
**Chosen**: Originally: hooks-only, no fallback. Now: hooks take priority, content detection as fallback for missing/stale files.
**Why**: The original "no fallback" design was correct for the reconciliation bugs, but too strict in practice. Sessions started before hooks were configured have no hook files and show as Unknown. Content fallback is a clean, non-interfering baseline.

## Context

The original motivation (avoiding reconciliation bugs) remains valid — hook files still take absolute priority when present and fresh. The fallback only activates when there is genuinely no hook data. This avoids the arbitration/timing issues that caused the original bugs while still providing useful status for pre-hook sessions.

See [hooks-content-fallback](hooks-content-fallback.md) for the specific change.
