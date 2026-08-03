# Architecture

The binary has four boundaries:

1. `codex` owns JSON-RPC transport and protocol decoding.
2. `domain` contains stable session and pagination types.
3. `app` owns UI-independent state transitions and navigation.
4. `tui` owns terminal setup, input events, and Ratatui rendering.

The main thread renders and handles keys. A standard worker thread owns the app-server child and sends pages or a failure through a channel. The first 50-item page is sent immediately; subsequent pages are appended as they arrive. No async runtime is required.

Unknown JSON fields are ignored. Only stable, required values cross from the adapter into domain types. The adapter ignores the protocol's unstable `path` field.
