package main

import (
	"bytes"
	"os"
	"strings"
	"testing"
)

func TestPublicCLIArguments(t *testing.T) {
	options, err := parseArgs([]string{"--log-file", "/tmp/peek.log"})
	if err != nil || options.logFile != "/tmp/peek.log" {
		t.Fatalf("parseArgs() = %#v, %v", options, err)
	}
	if _, err := parseArgs([]string{"--resume"}); err == nil {
		t.Fatal("unknown argument was accepted")
	}
}

func TestHelpAndVersionDoNotNeedATerminal(t *testing.T) {
	var stdout bytes.Buffer
	if err := run([]string{"--help"}, &stdout); err != nil || !strings.Contains(stdout.String(), "Usage:") {
		t.Fatalf("help = %q, %v", stdout.String(), err)
	}
	stdout.Reset()
	if err := run([]string{"--version"}, &stdout); err != nil || !strings.Contains(stdout.String(), version) {
		t.Fatalf("version = %q, %v", stdout.String(), err)
	}
}

func TestDevNullIsNotAnInteractiveTerminal(t *testing.T) {
	file, err := os.Open(os.DevNull)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = file.Close() })
	if isTerminal(file) {
		t.Fatal("character device /dev/null was accepted as a terminal")
	}
}
