package ui

import (
	"context"
	"errors"
	"strings"
	"testing"

	"charm.land/bubbles/v2/list"
	tea "charm.land/bubbletea/v2"
	"github.com/charmbracelet/x/ansi"

	"github.com/Saba-Burduli/peek-codex/internal/codex"
	"github.com/Saba-Burduli/peek-codex/internal/domain"
)

func TestSessionFilterNeverUsesPreview(t *testing.T) {
	item := sessionItem{session: session("one", "Project health", "/tmp/peek", 10)}
	item.session.Preview = "super-secret-agent-output"
	if strings.Contains(item.FilterValue(), item.session.Preview) {
		t.Fatal("preview leaked into fuzzy-search input")
	}
}

func TestInitialViewIsLoadingNotEmpty(t *testing.T) {
	model := readyModel(t)
	view := model.View().Content
	if !strings.Contains(view, "Loading recent Codex sessions") || strings.Contains(view, "No interactive Codex sessions are available") {
		t.Fatalf("initial view did not show loading state: %q", view)
	}
}

func TestBranchIsTruncatedAfterCoreMetadata(t *testing.T) {
	item := sessionItem{session: session("one", "Project", "/tmp/peek", 1)}
	item.session.Branch = "very-long-branch-name"
	description := item.Description()
	if strings.Index(description, "OpenAI") > strings.Index(description, "[very-long-branch-name]") {
		t.Fatalf("branch did not follow provider metadata: %q", description)
	}
}

func TestNarrowRowsOmitBranchInsteadOfShowingAPartialBranch(t *testing.T) {
	model := readyModel(t)
	_, _ = model.Update(tea.WindowSizeMsg{Width: 32, Height: 20})
	item := session("one", "Project", "/tmp/peek", 1)
	item.Branch = "very-long-branch-name"
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{item}}})
	rendered := ansi.Strip(model.list.View())
	if strings.Contains(rendered, "[very") || strings.Contains(rendered, "branch-name") {
		t.Fatalf("narrow row rendered branch metadata: %q", rendered)
	}
	if !strings.Contains(rendered, "OpenAI") {
		t.Fatalf("narrow row lost provider before branch: %q", rendered)
	}
}

func TestPageAppendPreservesSelectionIdentity(t *testing.T) {
	model := readyModel(t)
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("newer", "Newer", "/tmp/peek", 20),
		session("selected", "Selected", "/tmp/peek", 10),
	}}})
	model.list.Select(1)
	model.syncSelection()
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("latest", "Latest", "/tmp/other", 30),
	}}})
	item, ok := model.list.SelectedItem().(sessionItem)
	if !ok || item.session.ID != "selected" {
		t.Fatalf("selected item = %#v, want selected session", model.list.SelectedItem())
	}
}

func TestSearchAcceptsJAndQAsTextAndEscReturnsToSessions(t *testing.T) {
	model := readyModel(t)
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("one", "Project", "/tmp/peek", 1),
	}}})
	_, _ = model.handleKey(press("/"))
	if model.list.FilterState() != list.Filtering {
		t.Fatal("search did not start")
	}
	_, _ = model.handleKey(press("j"))
	_, _ = model.handleKey(press("q"))
	if got := model.list.FilterValue(); got != "jq" {
		t.Fatalf("filter text = %q, want jq", got)
	}
	model.openDetails()
	if model.screen != detailsScreen {
		t.Fatal("Enter-equivalent did not open details")
	}
	_, _ = model.handleKey(special(tea.KeyEscape))
	if model.screen != sessionsScreen {
		t.Fatal("Esc did not return from details")
	}
}

func TestDetailsAreProjectMetadataOnly(t *testing.T) {
	model := readyModel(t)
	first := session("one", "Project health", "/tmp/peek", 10)
	first.Provider, first.Status, first.Branch = "openai/cli", "notLoaded", "main"
	first.Preview = "assistant output that must stay private"
	second := session("two", "Fix compile", "/tmp/peek", 20)
	second.Provider, second.Status, second.Branch = "openai/cli", "idle", "main"
	model.sessions = []domain.Session{first, second}
	content := model.detailsContent(first)
	for _, expected := range []string{"2 loaded local Codex sessions", "Providers: OpenAI", "Statuses:  Idle, Not loaded", "Branches:  main"} {
		if !strings.Contains(content, expected) {
			t.Errorf("details did not contain %q", expected)
		}
	}
	if strings.Contains(content, first.Preview) {
		t.Fatal("preview leaked into details")
	}
}

func TestRootQExitsButFilteredQDoesNot(t *testing.T) {
	model := readyModel(t)
	_, command := model.handleKey(press("q"))
	if command == nil {
		t.Fatal("root q did not return quit command")
	}
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{session("one", "Project", "/tmp/peek", 1)}}})
	_, _ = model.handleKey(press("/"))
	_, command = model.handleKey(press("q"))
	if command == nil || model.list.FilterValue() != "q" {
		t.Fatal("q did not become active search text")
	}
}

func TestFuzzyFilterAndRepeatedCursorState(t *testing.T) {
	model := readyModel(t)
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("one", "Project browser", "/tmp/peek", 1),
		session("two", "Other work", "/tmp/other", 2),
	}, NextCursor: "next"}})
	model.list.SetFilterText("pbr")
	if visible := model.list.VisibleItems(); len(visible) != 1 {
		t.Fatalf("fuzzy filter visible items = %d, want 1", len(visible))
	}
	model.handlePage(pageMsg{page: domain.SessionPage{NextCursor: "next"}})
	if model.loading || !strings.Contains(model.warning, "repeated cursor") {
		t.Fatalf("repeated cursor state = loading:%v warning:%q", model.loading, model.warning)
	}
}

func TestPartialFailureKeepsLoadedRowsUsable(t *testing.T) {
	model := readyModel(t)
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{session("one", "Project", "/tmp/peek", 1)}}})
	model.loading = true
	model.handlePage(pageMsg{err: errors.New("later page\nfailed")})
	if model.failure != "" || model.warning != "later page failed" || len(model.list.Items()) != 1 {
		t.Fatalf("partial failure state = failure:%q warning:%q items:%d", model.failure, model.warning, len(model.list.Items()))
	}
}

func TestFilteredSelectionSurvivesBackgroundAppend(t *testing.T) {
	model := readyModel(t)
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("selected", "Project selected", "/tmp/peek", 10),
		session("other", "Project other", "/tmp/peek", 5),
	}}})
	model.list.SetFilterText("project")
	for index, item := range model.list.VisibleItems() {
		if session, ok := item.(sessionItem); ok && session.session.ID == "selected" {
			model.list.Select(index)
		}
	}
	model.syncSelection()
	_, command := model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("latest", "Project latest", "/tmp/peek", 20),
	}}})
	if command == nil {
		t.Fatal("filtered append did not request recomputation")
	}
	updated, _ := model.Update(command())
	model = updated.(*Model)
	item, ok := model.list.SelectedItem().(sessionItem)
	if !ok || item.session.ID != "selected" {
		t.Fatalf("filtered selected item = %#v, want selected", model.list.SelectedItem())
	}
}

func TestFilteredSelectionSurvivesKeyBeforeFilterResult(t *testing.T) {
	model := readyModel(t)
	model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("selected", "Project selected", "/tmp/peek", 10),
		session("other", "Project other", "/tmp/peek", 5),
	}}})
	model.list.SetFilterText("project")
	for index, item := range model.list.VisibleItems() {
		if session, ok := item.(sessionItem); ok && session.session.ID == "selected" {
			model.list.Select(index)
		}
	}
	model.syncSelection()
	_, command := model.handlePage(pageMsg{page: domain.SessionPage{Sessions: []domain.Session{
		session("latest", "Project latest", "/tmp/peek", 20),
	}}})
	_, _ = model.handleKey(special(tea.KeyDown))
	updated, _ := model.Update(command())
	model = updated.(*Model)
	item, ok := model.list.SelectedItem().(sessionItem)
	if !ok || item.session.ID != "selected" {
		t.Fatalf("filtered selected item = %#v, want selected", model.list.SelectedItem())
	}
}

func readyModel(t *testing.T) *Model {
	t.Helper()
	model := New(context.Background(), func(context.Context) (codex.SessionSource, error) {
		return nil, nil
	})
	_, _ = model.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	return model
}

func press(text string) tea.KeyPressMsg {
	return tea.KeyPressMsg(tea.Key{Code: []rune(text)[0], Text: text})
}

func special(code rune) tea.KeyPressMsg { return tea.KeyPressMsg(tea.Key{Code: code}) }

func session(id, name, cwd string, recency int64) domain.Session {
	return domain.Session{
		ID: domain.SessionID(id), Name: name, CWD: cwd, CreatedAt: recency, UpdatedAt: recency, RecencyAt: recency,
		Provider: "openai/cli", Status: "idle",
	}
}
