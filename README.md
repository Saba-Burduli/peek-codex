# Peek Codex

Peek Codex is a fast, read-only terminal browser for local Codex sessions. The first vertical slice lists real sessions through the supported Codex app-server boundary, keeps loading later pages in the background, and provides predictable keyboard navigation.

## Requirements

- macOS or Linux
- Go 1.25 or newer
- Codex CLI 0.146.0 or newer on `PATH`
- An interactive terminal

## Run

```sh
go run ./cmd/peek-codex
```

Use `--help`, `--version`, or `--log-file <PATH>`. Diagnostics are silent unless a log file is explicitly requested.

Sessions is the first screen. Use `Up`/`k`, `Down`/`j`, `Home`, and `End` to move; press `/` for fuzzy search across safe metadata, `Enter` for a read-only details view, and `q`/`Ctrl-C` to exit. While typing a search, every printable key—including `j`, `k`, and `q`—is query text; use arrows to move filtered rows and `Esc` to clear search or return from details before exiting Sessions.

## Validate

```sh
go mod verify
test -z "$(gofmt -l .)"
go vet -mod=readonly ./...
go test -mod=readonly -race ./...
go build -mod=readonly -o /tmp/peek-codex ./cmd/peek-codex
go install golang.org/x/vuln/cmd/govulncheck@v1.7.0
"$(go env GOPATH)/bin/govulncheck" ./...
```

## Continuous integration

GitHub Actions validates pull requests and pushes to `main`. It caches Go dependencies, cancels superseded runs for the same branch or pull request, and runs the validation commands above with a 10-minute job limit. A manual run remains available through `workflow_dispatch`.

## Scope and safety

This release is read-only. It uses Bubble Tea, Bubbles, and Lip Gloss over `codex app-server` stdio; it does not inspect Codex SQLite databases or JSONL rollout files. Agent-output previews are neither displayed nor searchable. Enter opens a project-derived summary from safe list metadata; conversation turns are not loaded.

See [`TASKS.md`](TASKS.md) for the roadmap and [`docs/`](docs/) for product, architecture, integration, TUI, testing, and the small-commit delivery workflow.

## License

MIT
