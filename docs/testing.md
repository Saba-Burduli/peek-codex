# Testing strategy

Unit tests cover ordering, age formatting, project labels, text sanitization, navigation bounds, UI states, and page appending. Redacted JSON fixtures cover unknown fields, missing optional metadata, nested source values, commands/file-change content that must not leak into domain values, and malformed RPC errors.

An integration test places a fake `codex` executable first on a controlled path and verifies the initialize/initialized handshake plus cursor pagination. Tests never require personal Codex history.

Release validation uses formatting, Clippy with warnings denied, tests, a locked build, and a PTY smoke test against the installed Codex app-server.
