package domain

import (
	"strings"
	"testing"
	"time"
)

func TestSanitizeTerminalText(t *testing.T) {
	if got := SanitizeTerminalText(" hello\n\x1b[31m\tworld\r "); got != "hello [31m world" {
		t.Fatalf("SanitizeTerminalText() = %q", got)
	}
	if got := []rune(SanitizeTerminalText(strings.Repeat("é", 300))); len(got) != 240 {
		t.Fatalf("sanitized length = %d, want 240", len(got))
	}
}

func TestProjectAndDisplayMetadata(t *testing.T) {
	if got := ProjectLabel("/Users/example/project"); got != "project" {
		t.Fatalf("project label = %q", got)
	}
	if got := ProjectLabel("/Users/example"); got != "Workspace" {
		t.Fatalf("home project label = %q", got)
	}
	if got := DisplayProvider("openai/cli"); got != "OpenAI" {
		t.Fatalf("provider = %q", got)
	}
	if got := DisplayStatus("notLoaded"); got != "Not loaded" {
		t.Fatalf("status = %q", got)
	}
}

func TestSortAndAge(t *testing.T) {
	items := []Session{{ID: "b", RecencyAt: 10, UpdatedAt: 20}, {ID: "c", RecencyAt: 20, UpdatedAt: 5}, {ID: "a", RecencyAt: 10, UpdatedAt: 20}}
	SortSessions(items)
	if got := []SessionID{items[0].ID, items[1].ID, items[2].ID}; got[0] != "c" || got[1] != "a" || got[2] != "b" {
		t.Fatalf("unexpected ordering: %v", got)
	}
	now := time.Unix(10_000_000, 0)
	if got := FormatAge(9_992_800, now); got != "2h" {
		t.Fatalf("age = %q", got)
	}
}
