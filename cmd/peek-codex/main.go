package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	tea "charm.land/bubbletea/v2"

	"github.com/Saba-Burduli/peek-codex/internal/codex"
	"github.com/Saba-Burduli/peek-codex/internal/ui"
)

const version = "0.1.0"

const helpText = "Peek Codex — browse local Codex sessions\n\nUsage: peek-codex [OPTIONS]\n\nOptions:\n      --log-file <PATH>  Append structured diagnostics to PATH\n  -h, --help             Print help\n  -V, --version          Print version"

type options struct {
	help    bool
	version bool
	logFile string
}

func main() {
	if err := run(os.Args[1:], os.Stdout); err != nil {
		fmt.Fprintf(os.Stderr, "peek-codex: %v\n", err)
		os.Exit(1)
	}
}

func run(args []string, stdout io.Writer) error {
	options, err := parseArgs(args)
	if err != nil {
		return err
	}
	if options.help {
		_, err := fmt.Fprintln(stdout, helpText)
		return err
	}
	if options.version {
		_, err := fmt.Fprintf(stdout, "peek-codex %s\n", version)
		return err
	}
	if !isTerminal(os.Stdin) || !isTerminal(os.Stdout) {
		return errors.New("an interactive terminal is required; run Peek Codex directly in a terminal")
	}
	log, err := openDiagnostics(options.logFile)
	if err != nil {
		return fmt.Errorf("could not open log file: %w", err)
	}
	defer log.Close()
	if err := log.event("start"); err != nil {
		return err
	}
	model := ui.New(context.Background(), func(ctx context.Context) (codex.SessionSource, error) {
		return codex.Start(ctx, "codex", version)
	})
	program := tea.NewProgram(model)
	_, err = program.Run()
	model.Close()
	if err != nil && !errors.Is(err, tea.ErrInterrupted) {
		_ = log.event("failure")
		return fmt.Errorf("terminal session failed: %w", err)
	}
	return log.event("stop")
}

func parseArgs(args []string) (options, error) {
	var parsed options
	for index := 0; index < len(args); index++ {
		switch args[index] {
		case "-h", "--help":
			parsed.help = true
		case "-V", "--version":
			parsed.version = true
		case "--log-file":
			index++
			if index == len(args) {
				return options{}, errors.New("--log-file requires a path; run peek-codex --help")
			}
			if parsed.logFile != "" {
				return options{}, errors.New("--log-file may only be provided once")
			}
			parsed.logFile = args[index]
		default:
			return options{}, fmt.Errorf("unknown option %q; run peek-codex --help", args[index])
		}
	}
	return parsed, nil
}

func isTerminal(file *os.File) bool {
	info, err := file.Stat()
	return err == nil && info.Mode()&os.ModeCharDevice != 0
}

type diagnostics struct{ file *os.File }

func openDiagnostics(path string) (*diagnostics, error) {
	if path == "" {
		return &diagnostics{}, nil
	}
	directory := filepath.Dir(path)
	if directory != "." {
		if err := os.MkdirAll(directory, 0o755); err != nil {
			return nil, err
		}
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		return nil, err
	}
	return &diagnostics{file: file}, nil
}

func (diagnostics *diagnostics) event(event string) error {
	if diagnostics.file == nil {
		return nil
	}
	return json.NewEncoder(diagnostics.file).Encode(map[string]any{
		"timestamp": time.Now().Unix(),
		"event":     event,
	})
}

func (diagnostics *diagnostics) Close() {
	if diagnostics.file != nil {
		_ = diagnostics.file.Close()
	}
}
