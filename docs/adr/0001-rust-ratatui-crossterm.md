# ADR 0001: Rust with Ratatui and Crossterm

Status: superseded by ADR 0003

## Decision

The initial release used Rust 1.97.1, edition 2024, Ratatui 0.30.2, and Crossterm 0.29.

## Rationale

This was sufficient for the first vertical slice, but did not provide the mature reusable list, spinner, help, fuzzy-filtering, and viewport component ecosystem selected for Peek Codex.

## Consequences

The Rust implementation and toolchain pin are removed during the Go cutover. Git history preserves the original implementation.
