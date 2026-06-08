# Decision: Sidecar as subcommand, not separate binary

**Alternatives considered**: separate `claude-tmux-monitor` binary (two `[[bin]]` targets), external dependency
**Chosen**: `claude-tmux monitor --pane {pane_id}` subcommand within the main binary
**Why**: Single `cargo install` installs everything. No coordination between two binaries. `pipe-pane` invokes the same binary the user already has.

## Context

`cargo install` with two `[[bin]]` targets works but adds complexity (both must be in PATH, version sync). A subcommand keeps everything in one binary. The `init` command sets up tmux hooks that invoke `claude-tmux monitor --pane #{pane_id}` via `pipe-pane`.
