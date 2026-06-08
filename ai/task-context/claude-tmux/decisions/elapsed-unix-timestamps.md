# Decision: Elapsed time uses unix timestamps in state file

**Alternatives considered**: `Instant` (Rust monotonic clock); no persistence (reset on popup open)
**Chosen**: Unix timestamps (`u64`) persisted as `s:<pane_id> <epoch>` lines in `/tmp/claude-tmux-state`
**Why**: `Instant` resets when the popup process exits — every popup open showed "0s" for all sessions. Unix timestamps survive across processes. The daemon maintains them continuously; the popup reads on startup.

## Context

First implementation used `HashMap<String, Instant>`. The user reported the counter restarting every time the popup opened. Switched to `HashMap<String, u64>` with `SystemTime::now().duration_since(UNIX_EPOCH)`. The state file already existed for Done/worked_unfocused persistence — adding `s:` lines was trivial.
