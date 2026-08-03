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
