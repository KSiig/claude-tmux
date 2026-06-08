# Decision: Process-tree as default backend

**Alternatives considered**: hooks-only default, sidecar default, current hybrid as default
**Chosen**: Process-tree inspection as default, hooks and sidecar opt-in via `claude-tmux init`
**Why**: Zero config requirement. A user who does `cargo install` + runs the binary should see status detection working immediately. Hooks require Claude Code settings changes. Sidecar requires pipe-pane setup. Process-tree uses only OS tools (ps, lsof) and tmux's pane_pid — both always available.

## Context

The user installs via `cargo install --git ...` and runs `claude-tmux`. Process-tree detection works immediately because it only needs the pane PID (from tmux) and process inspection (from the OS). The tradeoff is that "thinking" phases (model generating server-side, no local children/connections) may briefly show as Idle, but this is a mild visual glitch, not a cascading bug.
