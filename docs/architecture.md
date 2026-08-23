# Architecture

The Go binary has four boundaries:

1. `internal/codex` owns JSON-RPC transport and protocol decoding.
2. `internal/domain` contains stable session and pagination types.
3. `internal/ui` owns Bubble Tea state transitions, navigation, and project summaries.
4. `cmd/peek-codex` owns public CLI parsing, diagnostics, TTY preflight, and program lifecycle.

Bubble Tea owns rendering and input. Cancellable Bubble Tea commands start the app-server and load one page at a time; each received page schedules the next. The first 50-item page renders as soon as it arrives, and later pages append without losing the selected session ID. The model closes app-server when Bubble Tea exits, restoring the terminal's alternate screen.

Unknown JSON fields are ignored. Only stable, required values cross from the adapter into domain types. The adapter ignores the protocol's unstable `path` field.
