# Git delivery workflow

Ship one coherent change at a time, regardless of size:

1. Inspect `git status --short` and stage only the intended paths.
2. Run the narrowest validation that proves the change.
3. Review `git diff --cached --check` and the staged summary.
4. Commit immediately with a focused message describing only that change.
5. Push the commit immediately and verify the remote branch points to the local commit.

Do not hold small completed changes to combine them with later work. Do not mix refactors, fixes, docs, or CI changes unless they are inseparable. The GitHub Actions CI workflow validates every push and pull request; local targeted validation remains required before pushing.
