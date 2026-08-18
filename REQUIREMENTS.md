# Requirements

## R1 — Codex integration boundary

Peek Codex reads sessions only through `codex app-server --stdio`. It performs the initialize handshake, uses stable `thread/list` and `thread/read` methods, tolerates unknown response fields, and never reads Codex SQLite or JSONL storage directly.

## R2 — Progressive session loading

The first 50-session page is rendered as soon as it arrives. Later pages append in the background in descending recency order. Every production app-server request has a 10-second deadline, with an internal configurable seam for faster tests. Pagination must stop on completion, cancellation, timeout, protocol failure, or a repeated cursor.

## R3 — Lifecycle and failure states

Loading, ready, empty, partial-results, and fatal-failure behavior is explicit. A later-page failure keeps loaded rows usable. Startup, protocol, and partial-result messages are control-character sanitized, collapsed to one line, and limited to 240 characters. Failures become concise actionable errors rather than indefinite loading.

## R4 — Safe responsive rendering

Each row shows age, the captured project-folder label, optional captured branch, and the saved session title. Agent-output previews are never rendered or searchable. Text is sanitized, single-line, bounded, and truncated by terminal display-cell width. Branch metadata is removed first when space is constrained. A session launched from a home directory is labelled `Workspace`, never with the account-name folder.

## R5 — Navigation and shutdown

`Up`/`k`, `Down`/`j`, `Home`, and `End` preserve a valid selected session. Background appends and re-sorting preserve the selected session ID, falling back to the first row only if that session disappears. `Ctrl-C` always restores the terminal, cancels loading, stops app-server, and joins the worker without leaving a child process. Outside active text search, `q` exits; during search it is query text and `Esc` clears search or returns from session details before exiting the root Sessions view.

## R6 — CLI and diagnostics

The public CLI supports normal launch, `--help`, `--version`, and optional `--log-file <PATH>`. Non-TTY launch fails before terminal mutation. Diagnostics are silent by default and never contain conversation text.

## R7 — Compatibility and safety

macOS and Linux are supported with Codex CLI 0.146.0 as the minimum verified version. Unknown fields are forward-compatible, the unstable protocol `path` is ignored, and failures never fall back to private storage.

## R8 — Independent QA gate

Every tracked change receives a read-only testing-agent review before commit and push. The main agent owns all edits and regression tests, resolves every reproducible bug or requirement mismatch, obtains tester clearance, then commits, pushes, verifies remote SHA, and confirms CI.

## R9 — Sessions-first discovery and inspection

The root view is a Sessions overview that identifies the loaded and matching session count and project count, and explains keyboard discovery. `/` starts a live, case-insensitive search across safe session metadata excluding agent-output previews; navigation stays within matching rows and keeps selection stable after search is cleared. `Enter` opens a read-only project details view for the visible selection, showing a derived project summary (loaded session count, latest activity, providers, statuses, and branches) plus the selected session's metadata. It never displays the captured preview or loads conversation turns. `Esc` returns to Sessions from details.
