# ADR 0002: Codex app-server boundary

Status: accepted

## Decision

Read sessions through Codex app-server over stdio using `thread/list` and `thread/read`. Reserve `codex resume <id>` for a future explicit handoff after restoring the terminal and stopping app-server.

## Rationale

App-server is Codex's rich-client integration surface and avoids coupling this tool to private storage schemas. Keeping its JSON-RPC types inside one adapter limits compatibility risk.

## Consequences

Peek Codex never opens Codex SQLite or JSONL storage. Unknown response fields are tolerated, the unstable `path` field is ignored, and incompatible Codex versions fail with a focused error rather than falling back to private data.
