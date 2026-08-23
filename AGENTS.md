# Peek Codex contributor guidance

- Keep Codex protocol types inside `internal/codex`; the rest of the application uses domain types.
- Never read Codex SQLite or JSONL storage directly, and never log conversation text.
- Preserve terminal restoration on every exit path.
- Keep Bubble Tea commands cancellable and keep app-server lifecycle ownership in the Go model.
- Keep changes narrowly scoped and run the validation commands in `README.md` before committing.
- Treat every completed change as its own delivery unit, including small fixes and documentation edits.
- After targeted validation, commit that unit and push it immediately; do not bundle unrelated completed work for a later push.
- Stage explicit paths and never rewrite published history unless the user explicitly requests it.
- Treat `REQUIREMENTS.md` as the acceptance contract and name the affected requirement IDs in testing-agent handoffs.
- The main agent is the only editor. Testing agents are read-only and must not stage, commit, push, or mutate external state.
- Before every commit, spawn a fresh testing agent to review the working diff against the named requirements. Even typo-only changes require a lightweight independent pass.
- Testing reports must include severity, requirement ID, exact location, reproduction or evidence, expected versus actual behavior, and missing coverage.
- Fix every reproducible bug and requirement mismatch, add a regression test when behavior changed, and ask the same tester to retest. Commit and push only after tester clearance.
- Record non-bug suggestions in `TASKS.md`; do not silently broaden the current delivery unit.
