# TUI behavior

The list keeps selection in bounds when moving and when pages are appended. `Up`/`k` and `Down`/`j` move one row; `Home` and `End` jump to the first and last loaded rows. `q`, `Esc`, and `Ctrl-C` exit.

Rows collapse gracefully in narrow terminals: optional branch metadata disappears before the project and preview. Long content is truncated to the available display width. Control characters and line breaks are replaced before rendering.

Terminal raw mode and the alternate screen are guarded so cleanup runs on normal exit and errors. Non-TTY execution fails before terminal mutation with an actionable message.
