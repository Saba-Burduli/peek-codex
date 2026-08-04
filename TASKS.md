# Tasks

## Slice 1: repository foundation and basic browser

- [x] Establish repository guidance, documentation, ADRs, and MIT licensing.
- [x] Add the typed session domain and Codex app-server adapter.
- [x] Test parsing, pagination, malformed errors, and a fake app-server handshake.
- [x] Add the responsive session-list TUI and CLI.
- [x] Model loading, ready, empty, and failure states.
- [x] Validate formatting, linting, tests, locked build, and a real PTY smoke test.

## Deferred

- [ ] Enrich a selected session with a read-only peek.
- [ ] Restore the terminal and hand off with `codex resume <id>` on Enter.
- [ ] Add search and project filtering.
- [ ] Inspect current Git state separately from captured session metadata.
- [ ] Add a persistent cache.
- [ ] Package release artifacts.
- [ ] Add Windows-specific process handoff.

Out of scope: AI summaries, telemetry, session mutation, direct storage parsing, an async runtime, and fuzzy-search dependencies.

## Quality hardening

- [x] Define numbered acceptance requirements and an independent testing-agent gate.
- [x] Bound app-server requests and make loader shutdown deterministic.
- [x] Reject repeated pagination cursors and preserve partial results and selection.
- [ ] Sanitize failure text and enforce the terminal-text length bound.
- [ ] Render rows using terminal display-cell width.
- [ ] Add explicit `A → B → A` cursor-cycle regression coverage.
- [ ] Add a fake-server page-success-then-RPC-error integration test.
