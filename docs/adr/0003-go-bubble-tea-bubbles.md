# ADR 0003: Go with Bubble Tea and Bubbles

Status: accepted

## Decision

Use Go 1.25 or newer with Bubble Tea v2, Bubbles v2, and Lip Gloss v2 for the Peek Codex executable.

## Rationale

Peek Codex needs a mature terminal component ecosystem, especially a fuzzy-filterable list, spinner, built-in help, responsive viewport, and styling system. Bubble Tea and Bubbles provide these widgets in one native Go binary while keeping Codex app-server transport isolated behind typed domain values.

## Consequences

The project uses Go modules and race-enabled tests. Bubble Tea owns terminal restoration and alternate-screen rendering; the application model closes the Codex child process after every terminal exit. macOS and Linux support remains unchanged.
