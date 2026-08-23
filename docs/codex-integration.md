# Codex integration

Peek Codex starts `codex app-server --stdio`, sends `initialize`, then sends the `initialized` notification. Listing uses stable `thread/list` requests with:

- `useStateDbOnly: true`
- `sortKey: "recency_at"`
- `sortDirection: "desc"`
- the protocol default interactive source set
- `limit: 50`
- the opaque `nextCursor` for later pages

`thread/read` is exposed through the typed source boundary with `includeTurns: false`, but is not invoked by the first UI slice. Storage paths are never consumed and there is no SQLite or JSONL fallback.

Codex CLI 0.146.0 is the minimum locally verified version. Each request has a 10-second deadline and checks cancellation while waiting for app-server output. Protocol or startup failures become actionable UI errors. The app-server command is the intended rich-client surface but remains a compatibility risk, so the protocol is isolated in one module.

Future resume handoff must first restore the terminal and stop app-server, then execute `codex resume <uuid>` with inherited stdio. Normal exit cancels Bubble Tea loading and closes, kills, and waits for app-server.

## Discovery baseline

Recorded on 2026-08-03: Codex CLI 0.146.0 on macOS arm64; 106 local session logs totaling 1,083,105,709 bytes, with the largest 669,885,990 bytes. These counts document the environment only; application code does not enumerate these files.
