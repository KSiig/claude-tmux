# Decision: Only daemon writes status file

**Alternatives considered**: Both popup and daemon write, popup writes only when daemon isn't running, file locking
**Chosen**: Only the daemon writes `/tmp/claude-tmux-status`
**Why**: Popup and daemon were racing on the file, producing inconsistent counts. Since the daemon runs persistently and has the most up-to-date view, it's the natural owner.

## Context

The status file (`/tmp/claude-tmux-status`) contains key=value counts (working, done, idle, etc.) read by the user's Claude Code statusline script. When both popup and daemon wrote to it, the statusline would show 9 Done (from old daemon binary) while the popup showed 0 Done. The popup's writes would briefly overwrite the daemon's, then the daemon would overwrite back — producing flickering counts in the statusline.

The fix uses a `writes_status_file: bool` field on `App`, set to `true` only in headless/daemon mode. This also determines `is_focused` semantics: `let is_focused = self.writes_status_file && self.sessions[idx].attached;` — the popup treats no session as focused.
