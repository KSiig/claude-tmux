# Approach: Tab character as tmux format separator

**Verdict**: Rejected
**Why**: tmux's `vis()` function converts tab characters (0x09) to underscores (0x5f) when no locale is set. launchd agents don't inherit LANG/LC_ALL, so the daemon always hit this.

## What was tried

Used `\t` (Rust string literal tab) as the field separator in `tmux list-sessions -F` and `tmux list-panes -F` format strings. Code split output on `'\t'`.

## What happened

Worked perfectly when run from a terminal (which has `LANG=en_US.UTF-8`). Failed silently under launchd — `split('\t')` found no tabs, so `parts.len() < 4`, every line was skipped, and `list_sessions()` returned an empty vec. The daemon wrote `total=0` to the status file.

Diagnosed by:
1. Running the daemon with `env -i PATH=... HOME=...` (no LANG) — reproduced the issue
2. Comparing `tmux list-sessions -F '...'` output through `xxd` — saw `0x5f` (underscore) where `0x09` (tab) was expected
3. Adding `LANG=en_US.UTF-8` to the clean env — tabs appeared correctly

## Key takeaway

Never use non-printing characters as separators in tmux format strings — use printable ASCII like `|||` instead. tmux's output encoding depends on locale, and launchd/systemd services typically lack locale settings.
