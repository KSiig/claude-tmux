# Approach: Sidecar initial capture at startup

**Verdict**: Adopted
**Why**: `tmux pipe-pane` only forwards output produced after the pipe is attached. Sessions that are already idle when the sidecar starts never receive any data, so the stream state machine stays at Unknown indefinitely.

## What was tried

Added `initial_capture()` to the monitor that runs `tmux capture-pane -t {pane_id} -p -J -e` at startup, strips ANSI, and runs the same `detect_from_stream_content()` state machine. If the result is not Unknown, writes the status file immediately before entering the stdin read loop.

Also fixed pane ID normalization: the `%` prefix was being consumed by tmux/shell when constructing the pipe-pane command. The monitor now always prepends `%` if missing.

## What happened

Before fix: all sessions showed `?` (Unknown) in sidecar mode because no data flowed through pipe-pane for idle sessions.
After fix: sessions get correct initial status from capture, then stream updates keep it current.

## Key takeaway

Any sidecar approach using `pipe-pane` must establish baseline status via an initial pane capture — the stream only carries *new* output.
