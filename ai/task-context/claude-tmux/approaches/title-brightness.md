# Approach: Color::Gray for titled sessions

**Verdict**: Rejected
**Why**: Color::Gray (ANSI 8) is nearly identical to Color::DarkGray (ANSI 7) in the user's terminal theme. Also tried Color::Reset (terminal default fg) which also wasn't visibly different enough. Color::Cyan was used as a debug test and confirmed the rendering path works — the issue was purely color choice.

## What was tried

Set `detail_color` to `Color::Gray` for sessions with an inline title from titles.json, while keeping `Color::DarkGray` for plain paths. Also tried `Color::Reset`.

## What happened

Both Gray and Reset looked identical to DarkGray in the user's dark terminal theme. Cyan was unmistakably different, confirming the code path was correct. Final choice: `Color::White`.

## Key takeaway

In this terminal theme, only White and named colors (Cyan, Green, etc.) are reliably distinct from DarkGray. Gray and Reset blend in.
