# Peek Codex

Peek Codex is a fast, read-only terminal browser for local Codex sessions. The first vertical slice lists real sessions through the supported Codex app-server boundary, keeps loading later pages in the background, and provides predictable keyboard navigation.

## Requirements

- macOS or Linux
- Rust 1.97.1 (pinned by `rust-toolchain.toml`)
- Codex CLI 0.146.0 or newer on `PATH`
- An interactive terminal

## Run

```sh
cargo run --locked
```

Use `--help`, `--version`, or `--log-file <PATH>`. Diagnostics are silent unless a log file is explicitly requested.

Keys: `Up`/`k`, `Down`/`j`, `Home`, `End`, and `q`/`Esc`/`Ctrl-C` to exit.

## Validate

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --locked
```

## Scope and safety

This release is read-only. It talks to `codex app-server` over stdio and does not inspect Codex SQLite databases or JSONL rollout files. Conversation previews are sanitized before display and are never written to diagnostics.

See [`TASKS.md`](TASKS.md) for the roadmap and [`docs/`](docs/) for product, architecture, integration, TUI, testing, and the small-commit delivery workflow.

## License

MIT
