# Decision: Daemon polls Linear, popup reads cache file

**Alternatives considered**: popup polls directly on open/refresh, shared in-memory cache, SQLite
**Chosen**: Daemon writes `/tmp/claude-tmux-linear.json`, popup reads it
**Why**: The daemon already runs a tick loop (status detection every 50-500ms). Adding a 10s Linear poll fits naturally. The popup is short-lived (opens, user picks session, closes) — making it do HTTP requests would add latency. A JSON file is simple, atomic enough for this use case, and lets the popup start instantly with pre-fetched data.

## Context

- The daemon extracts Linear issue identifiers from tmux session names using a TEAM-NUMBER pattern (e.g., "VEL-420" from session "VEL-420-556-ci-migration")
- Sub-issue IDs are also extracted: third numeric segment becomes TEAM-SUBNUMBER (e.g., "VEL-556")
- The GraphQL query batches all identifiers into a single request using `issues(filter: { id: { in: $ids } })`
- `LINEAR_API_KEY` env var must be set in the daemon's shell environment (exported in `~/.zcustom`)
- `ureq` (blocking HTTP client) was chosen over `reqwest` to avoid async runtime — the daemon is single-threaded with `thread::sleep`
