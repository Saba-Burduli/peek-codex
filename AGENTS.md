# Peek Codex contributor guidance

- Keep Codex protocol types inside `src/codex.rs`; the rest of the application uses domain types.
- Never read Codex SQLite or JSONL storage directly, and never log conversation text.
- Preserve terminal restoration on every exit path.
- Prefer synchronous Rust, worker threads, and channels unless async behavior is demonstrably required.
- Keep changes narrowly scoped and run the validation commands in `README.md` before committing.
- Treat every completed change as its own delivery unit, including small fixes and documentation edits.
- After targeted validation, commit that unit and push it immediately; do not bundle unrelated completed work for a later push.
- Stage explicit paths and never rewrite published history unless the user explicitly requests it.
