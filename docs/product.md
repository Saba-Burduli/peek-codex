# Product definition

Peek Codex reduces the cost of finding a previous Codex session. Its first release is intentionally narrow: start quickly, show recent interactive sessions, remain usable while additional pages arrive, and exit without leaving the terminal altered.

Each row shows age, a project label derived from the captured working directory, an optional captured Git branch, and a single-line preview. Explicit loading, ready, empty, partial-results, and fatal-failure behavior keeps the application understandable. If a later page fails, already loaded rows remain usable and the footer explains that results are partial.

The browser is read-only. Resuming, search, filters, enrichment, caching, and packaging are deferred until the listing boundary is stable.
