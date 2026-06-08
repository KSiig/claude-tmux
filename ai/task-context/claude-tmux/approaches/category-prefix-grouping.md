# Approach: Category-prefix grouping with display stripping

**Verdict**: Adopted
**Why**: Enables grouping by naming convention (e.g., `skill-flush`, `skill-linear` under `skill`) without changing session names or adding explicit config.

## What was tried

Relaxed the `compute_group_key()` common-prefix check to allow single-segment prefixes (no dash required in prefix) when neither session is a task ID (`is_task_id()` returns false). Added `strip_prefix: bool` to `SessionGroup` — set when:
1. Group label is NOT a task ID
2. No session has the exact group label as its name (no parent session)
3. All sessions start with `{label}-`

In the UI, `display_names` are built per-group, stripping the prefix when `strip_prefix` is true.

## What happened

Works correctly. `skill-flush` + `skill-linear` group under `skill` and display as `flush`, `linear`. Task IDs (`VEL-419`, `VEL-420`) remain separate. Parent sessions (`skill` + `skill-flush`) group but don't strip (parent session would become nameless). 4 new tests cover these scenarios.

## Key takeaway

The `is_task_id()` check is the critical guard — without it, `VEL-419` and `VEL-420` would wrongly merge under `VEL`.
