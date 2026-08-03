# ADR 0001: Rust with Ratatui and Crossterm

Status: accepted

## Decision

Use Rust 1.97.1, edition 2024, Ratatui 0.30.2, and Crossterm 0.29 for the initial application.

## Rationale

Rust produces a fast distributable binary and makes process and terminal cleanup explicit. Ratatui supplies composable terminal widgets; Crossterm supplies portable terminal events and is the normal Ratatui backend for application use.

## Consequences

The repository pins the compiler and commits `Cargo.lock`. Initial platform support is macOS and Linux. The application avoids an async runtime until concurrency requirements justify it.
