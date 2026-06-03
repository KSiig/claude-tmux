# Decision: Popup treats no session as focused

**Alternatives considered**: Use tmux `#{session_attached}` in both modes, track popup's "parent" session separately
**Chosen**: In popup mode, `is_focused` is always false for all sessions
**Why**: tmux's `#{session_attached}` reports "1" for the session the popup was invoked from, even while the popup overlay is showing. Using it in popup mode prevented the invoking session from ever being eligible for Done.

## Context

When the user presses `prefix-C`, tmux opens the popup as an overlay on the current session. That session's `#{session_attached}` stays "1" because the session is technically still attached — the popup is just drawn on top. If the popup used this flag for `is_focused`, the invoking session would have `worked_unfocused` and `done_panes` cleared every tick, making Done impossible for the one session the user is most likely working in.

The daemon, running persistently without a popup overlay, correctly uses `#{session_attached}` — the actually-viewed session should not accumulate Done status since the user can already see it.

This asymmetry is encoded as: `let is_focused = self.writes_status_file && self.sessions[idx].attached;` where `writes_status_file` is true only in daemon mode.
