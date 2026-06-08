# Decision: Singletons before headed groups

**Alternatives considered**: separator lines between groups, keep insertion order
**Chosen**: Sort headerless singletons before headed groups in `group_sessions()`
**Why**: Headerless singletons (e.g. `cloudsim`, `obsidian`) had no visual separator and appeared to belong to the preceding headed group (e.g. `flush`). Moving all singletons to the top cleanly separates them — the first group header acts as a natural visual boundary.

## Context

The grouping logic in `group_sessions()` originally returned groups in insertion order (based on session name sorting). With headed groups (multi-member or task-ID groups) interspersed with headerless singletons, singletons visually merged into the preceding group.

An intermediate attempt added a `separator` field to `SessionGroup` and rendered `───` lines before headerless groups that followed headed ones. This worked visually but was abandoned in favor of the simpler reorder since the first group header already provides sufficient visual separation.

The `separator` field remains in `SessionGroup` but is always `false` currently.

Critical follow-up: after reordering, `selected_session()` and all selection methods must use `display_ordered_sessions()` (which flattens `grouped_filtered_sessions()`) rather than `filtered_sessions()` (which returns the pre-grouping alphabetical order). Missing this caused selection to target the wrong session.
