# TUI behavior

Sessions is the root view. Its short introduction identifies the loaded and currently matching session count, and each row provides an age, project, optional branch, and safe preview for quick comparison of recent and older history. `Up`/`k` and `Down`/`j` move one row; `Home` and `End` jump to the first and last visible rows. `/` starts a live case-insensitive search across safe metadata; typing filters immediately, `Backspace` edits, and `Esc` clears the search. While search is active, every printable key is query text and arrows/Home/End navigate its filtered rows. `Enter` opens a read-only session detail view. In details, `Esc` returns to Sessions; `q` exits when not typing search and `Ctrl-C` exits from every view.

The detail view shows captured title, project, path, branch, provider, status, relative timestamps, session ID, and preview. It does not load conversation turns, mutate sessions, or invoke `codex resume`.

Rows collapse gracefully in narrow terminals: optional branch metadata disappears before project and preview. Long content is truncated to terminal display-cell width, including wide and combining Unicode characters. Session text, fatal errors, and partial-result warnings have control characters and line breaks replaced before rendering and are capped at 240 characters.

Terminal raw mode and the alternate screen are guarded so cleanup runs on normal exit and errors. Non-TTY execution fails before terminal mutation with an actionable message.
