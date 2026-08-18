# Product definition

Peek Codex reduces the cost of finding a previous Codex session. Its first release is intentionally narrow: start quickly, show recent interactive sessions, remain usable while additional pages arrive, and exit without leaving the terminal altered.

The first screen is Sessions: a concise introduction followed by the loaded session list, with both total and live search-match counts. Each row shows age, a project label derived from the captured working directory, an optional captured Git branch, and a single-line preview. Users can press `/` to search safe metadata and `Enter` to inspect a selected session's read-only metadata and summary. Explicit loading, ready, empty, partial-results, and fatal-failure behavior keeps the application understandable. If a later page fails, already loaded rows remain usable and the footer explains that results are partial.

The browser is read-only. Resume handoff, project-only filtering, live Git enrichment, caching, and packaging remain deferred. The detail view never loads conversation turns.
