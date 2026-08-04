# Requirements

## R1 — Codex integration boundary

Peek Codex reads sessions only through `codex app-server --stdio`. It performs the initialize handshake, uses stable `thread/list` and `thread/read` methods, tolerates unknown response fields, and never reads Codex SQLite or JSONL storage directly.

## R2 — Progressive session loading

The first 50-session page is rendered as soon as it arrives. Later pages append in the background in descending recency order. Every production app-server request has a 10-second deadline, with an internal configurable seam for faster tests. Pagination must stop on completion, cancellation, timeout, protocol failure, or a repeated cursor.

## R3 — Lifecycle and failure states

Loading, ready, empty, partial-results, and fatal-failure behavior is explicit. A later-page failure keeps loaded rows usable. Startup, protocol, and partial-result messages are control-character sanitized, collapsed to one line, and limited to 240 characters. Failures become concise actionable errors rather than indefinite loading.

## R4 — Safe responsive rendering

Each row shows age, project, optional captured branch, and preview. Text is sanitized, single-line, bounded, and truncated by terminal display-cell width. Branch metadata is removed first when space is constrained.

## R5 — Navigation and shutdown

`Up`/`k`, `Down`/`j`, `Home`, and `End` preserve a valid selected session. Background appends and re-sorting preserve the selected session ID, falling back to the first row only if that session disappears. `q`, `Esc`, and `Ctrl-C` restore the terminal, cancel loading, stop app-server, and join the worker without leaving a child process.

## R6 — CLI and diagnostics

The public CLI supports normal launch, `--help`, `--version`, and optional `--log-file <PATH>`. Non-TTY launch fails before terminal mutation. Diagnostics are silent by default and never contain conversation text.

## R7 — Compatibility and safety

macOS and Linux are supported with Codex CLI 0.146.0 as the minimum verified version. Unknown fields are forward-compatible, the unstable protocol `path` is ignored, and failures never fall back to private storage.

## R8 — Independent QA gate

Every tracked change receives a read-only testing-agent review before commit and push. The main agent owns all edits and regression tests, resolves every reproducible bug or requirement mismatch, obtains tester clearance, then commits, pushes, verifies remote SHA, and confirms CI.
