# Tasks

## Slice 1: repository foundation and basic browser

- [x] Establish repository guidance, documentation, ADRs, and MIT licensing.
- [x] Add the typed session domain and Codex app-server adapter.
- [x] Test parsing, pagination, malformed errors, and a fake app-server handshake.
- [x] Add the responsive Bubble Tea/Bubbles session-list TUI and Go CLI.
- [x] Model loading, ready, empty, and failure states.
- [x] Validate formatting, linting, tests, locked build, and a real PTY smoke test.

## Deferred

- [x] Enrich a selected session with a read-only metadata peek.
- [ ] Restore the terminal and hand off with `codex resume <id>` on Enter.
- [x] Add keyboard search across safe session metadata.
- [x] Make Sessions and details project-centric without displaying agent-output previews.
- [ ] Add project-only filtering controls.
- [ ] Inspect current Git state separately from captured session metadata.
- [ ] Add a persistent cache.
- [ ] Package release artifacts.
- [ ] Add Windows-specific process handoff.

Out of scope: AI summaries, telemetry, session mutation, direct storage parsing, persistent cache, and packaging.

## Quality hardening

- [x] Define numbered acceptance requirements and an independent testing-agent gate.
- [x] Bound app-server requests and make loader shutdown deterministic.
- [x] Reject repeated pagination cursors and preserve partial results and selection.
- [x] Sanitize failure text and enforce the terminal-text length bound.
- [x] Render rows with Bubbles display-cell-aware truncation.
- [ ] Add explicit `A → B → A` cursor-cycle regression coverage.
- [x] Add a fake-server page-success-then-RPC-error integration test.
