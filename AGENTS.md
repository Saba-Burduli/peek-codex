# Codex Slice contributor guidance

- Keep Codex protocol types inside `src/codex.rs`; the rest of the application uses domain types.
- Never read Codex SQLite or JSONL storage directly, and never log conversation text.
- Preserve terminal restoration on every exit path.
- Prefer synchronous Rust, worker threads, and channels unless async behavior is demonstrably required.
- Keep changes narrowly scoped and run the validation commands in `README.md` before committing.
