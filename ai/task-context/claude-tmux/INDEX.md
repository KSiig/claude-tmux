# Task Context: claude-tmux

> Pluggable status detection with three backends, docs overhaul, hooks fallback, ANSI stripping, and elapsed-time display.

## Status

All three backends working. 56 tests pass. Release build clean. Daemon running.

Completed this session:
1. Docs overhaul — README, CLAUDE.md, all docs/ pages updated for new detection module structure and `claude-tmux init`
2. Hooks backend fallback — falls back to content detection when no hook file exists (fresh start)
3. Content detection: ANSI stripping — `detect_from_content` now strips ANSI codes before pattern matching (ANSI splits text across color boundaries, breaking `contains()`)
4. Content detection: active-phase pattern — matches `… (Xm Xs · ↑ Xk tokens)` for any working phase (Sketching, Deploying, etc.)
5. Elapsed time display — shows how long each session has been in its current state (`3s`, `5m`, `2h`), persisted in state file as unix timestamps

**Next:** commit all changes, potentially switch default to hooks backend after more testing.

## Approaches Explored

| Approach | Verdict | Detail |
|----------|---------|--------|
| Remove state file persistence entirely | Rejected — no Done detection between popup sessions | [detail](approaches/remove-state-persistence.md) |
| Persist worked_unfocused only, not done_panes | Rejected — Done disappeared on second popup open | [detail](approaches/partial-persistence.md) |
| Force Idle on first tick for all statuses | Rejected — broke Working detection for static content | [detail](approaches/force-idle-first-tick.md) |
| First-observation guard | Superseded — no longer needed, backends produce clean signals | [detail](approaches/first-observation-guard.md) |
| Hook-based detection (hybrid) | Superseded — replaced by hooks-only backend | [detail](approaches/hook-based-detection.md) |
| Content-diff for Working detection | Rejected — cosmetic changes cause mass false Done | [detail](approaches/content-diff-detection.md) |
| Three-backend detection architecture | Implemented — process default, hooks opt-in, sidecar experimental | [detail](approaches/three-backend-architecture.md) |
| Sidecar initial capture | Adopted — pipe-pane only sends new output, need baseline | [detail](approaches/sidecar-initial-capture.md) |
| Hooks with content fallback | Adopted — hooks backend falls back to content detection when no hook file | [detail](approaches/hooks-content-fallback.md) |
| ANSI stripping for content detection | Adopted — tmux `-e` flag inserts ANSI codes that break pattern matching | [detail](approaches/ansi-stripping.md) |
| Token-line active-phase detection | Adopted — match `… (Xm · tokens)` pattern for any working phase | [detail](approaches/token-line-detection.md) |

## Key Files

| File | Role |
|------|------|
| `src/detection/mod.rs` | `DetectionBackend` trait, `DetectionMethod` enum, `DetectionContext`, `create_backend()` factory |
| `src/detection/process.rs` | ProcessBackend: builds process tree via `ps`, checks for tool child processes, content fallback |
| `src/detection/hooks.rs` | HooksBackend: reads `/tmp/claude-tmux-hooks/{pane_id}`, staleness → content fallback. Also `cleanup_hook_files()` |
| `src/detection/sidecar.rs` | SidecarBackend: reads same hook files. `ensure_sidecars()` starts pipe-pane for claude panes |
| `src/detection/content.rs` | Shared: `strip_ansi()`, `detect_from_content()`, `has_input_prompt()`, `has_input_field()`, `is_active_tool_phase()` |
| `src/monitor.rs` | `claude-tmux monitor --pane {id}` subcommand. Ring buffer, stream state machine. Uses `content::strip_ansi` |
| `src/app/mod.rs` | `tick_status()` delegates to backend. Done lifecycle. `status_since: HashMap<String, u64>` for elapsed time |
| `src/settings.rs` | `detection_method`, `hook_staleness_secs`, `done_delay_ms`. Removed: `hook_override_delay` |
| `src/init.rs` | `claude-tmux init` wizard: detection method, hook installation, daemon service setup (launchd/systemd) |
| `src/ui/mod.rs` | Renders elapsed time from `status_since` as `format_elapsed()`. Uses unix timestamps from state file |

## Decisions

| Decision | Rationale | Detail |
|----------|-----------|--------|
| Process-tree as default backend | Zero config, works out of box without `claude-tmux init` | [detail](decisions/default-backend.md) |
| Hooks as sole authority (no parsing fallback) | Superseded — hooks now fall back to content when no hook file | [detail](decisions/hooks-sole-authority.md) |
| Hooks fall back to content detection | Sessions started before hooks configured have no hook files | [detail](decisions/hooks-content-fallback.md) |
| Strip ANSI before content detection | tmux `-e` inserts color codes that split words, breaking `contains()` | [detail](decisions/ansi-stripping.md) |
| Elapsed time as unix timestamps in state file | `Instant` resets on popup open; unix timestamps persist across processes | [detail](decisions/elapsed-unix-timestamps.md) |
| Sidecar as subcommand, not separate binary | Single `cargo install`, `claude-tmux monitor` runs in pipe-pane mode | [detail](decisions/sidecar-subcommand.md) |

## Gotchas

- `session.attached` is "1" for the session the popup was invoked FROM, even while the popup overlay is showing.
- The daemon must be restarted after rebuilding to pick up the new binary.
- tmux's `vis()` function converts tabs to underscores when `LANG` is not set. Use `|||` separator.
- `capture_pane -e` includes ANSI escape sequences that change on resize — caused mass false-Done with content-diff.
- **ANSI codes split text across color boundaries** — `Sketching…` becomes `Sketchi` + color change + `ng…`. Must strip ANSI before any `contains()` pattern matching.
- Claude Code uses randomized working-phase names ("Effecting…", "Sketching…", "Gesticulating…", etc.) — match on the `… (Xm · tokens)` pattern, not the verb.
- **Hooks only fire for sessions started after `claude-tmux init`** — Claude Code loads hooks from `~/.claude/settings.json` at startup. Already-running sessions don't pick up new hooks until restarted.
- **Pane ID `%` in pipe-pane**: tmux's `pipe-pane -o "cmd --pane %361"` drops the `%`. Pass raw numeric ID, let monitor normalize with `%` prefix.
- **pipe-pane only forwards new output**: already-idle sessions get no data. Sidecar must do `tmux capture-pane` at startup.
- Process-tree backend can't distinguish "thinking" (API call, no local children) from "idle" — both show as Idle. Content fallback catches `… (tokens)` pattern.
- Old `src/hooks.rs` deleted — functionality absorbed into `src/detection/hooks.rs`. Old `src/detection.rs` deleted — replaced by `src/detection/` module.
