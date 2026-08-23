# Testing strategy

Go unit tests cover ordering, age formatting, project labels, text sanitization, fuzzy-search preview exclusion, navigation, selection identity through filtered pagination, loading/empty/failure states, partial failures, repeated cursors, and Bubble Tea key routing. Adapter tests cover unknown fields, unstable `path` exclusion, nested source values, malformed RPC errors, timeout, and cancellation without requiring personal history.

An integration test places a fake `codex` executable first on a controlled path and verifies the initialize/initialized handshake plus cursor pagination. Tests never require personal Codex history.

Release validation uses `gofmt` verification, `go vet`, race-enabled tests, a Go build, and a PTY smoke test against the installed Codex app-server.

## Independent testing-agent gate

Every tracked change follows this sequence before commit and push:

1. The main agent implements one coherent change and its regression tests.
2. The main agent runs the narrowest useful validation and pauses edits.
3. A fresh read-only testing agent reviews the working diff against named IDs from `REQUIREMENTS.md`.
4. The tester reports findings using the contract below.
5. The main agent fixes every reproducible bug or requirement mismatch and adds missing regression coverage.
6. The same tester retests the corrected diff. Unresolved product ambiguity goes to the user.
7. After clearance, the main agent commits, pushes, verifies the remote SHA, and confirms CI.

Testing agents never edit files, stage changes, commit, push, or include session/conversation text in output. Typo-only changes may use a lightweight review, but may not skip the gate.

### Finding contract

Each finding includes:

- severity: critical, high, medium, or low;
- affected requirement ID;
- exact file and line;
- minimal reproduction or concrete evidence;
- expected and actual behavior;
- missing or recommended regression coverage.

All reproducible correctness findings and requirement mismatches block delivery. Suggestions that do not describe a bug are added to `TASKS.md` and handled separately.
