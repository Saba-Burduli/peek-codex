// Package ui owns Bubble Tea state and rendering. It never decodes Codex
// protocol values directly.
package ui

import (
	"context"
	"fmt"
	"io"
	"sort"
	"strings"
	"time"

	"charm.land/bubbles/v2/key"
	"charm.land/bubbles/v2/list"
	"charm.land/bubbles/v2/spinner"
	"charm.land/bubbles/v2/viewport"
	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/charmbracelet/x/ansi"

	"github.com/Saba-Burduli/peek-codex/internal/codex"
	"github.com/Saba-Burduli/peek-codex/internal/domain"
)

type SourceFactory func(context.Context) (codex.SessionSource, error)

type screen uint8

const (
	sessionsScreen screen = iota
	detailsScreen
)

type Model struct {
	ctx     context.Context
	cancel  context.CancelFunc
	factory SourceFactory
	source  codex.SessionSource

	list     list.Model
	spinner  spinner.Model
	viewport viewport.Model
	screen   screen
	width    int
	height   int

	sessions    []domain.Session
	seenIDs     map[domain.SessionID]struct{}
	seenCursors map[string]struct{}
	nextCursor  string
	selectedID  domain.SessionID
	restoreID   domain.SessionID
	detail      domain.Session
	hasDetail   bool
	loading     bool
	failure     string
	warning     string
}

type sourceReadyMsg struct {
	source codex.SessionSource
	err    error
}

type pageMsg struct {
	page domain.SessionPage
	err  error
}

func New(ctx context.Context, factory SourceFactory) *Model {
	ctx, cancel := context.WithCancel(ctx)
	delegate := sessionDelegate{DefaultDelegate: list.NewDefaultDelegate()}
	delegate.Styles.NormalTitle = lipgloss.NewStyle().Foreground(lipgloss.Color("#e6edf3")).PaddingLeft(2)
	delegate.Styles.NormalDesc = lipgloss.NewStyle().Foreground(lipgloss.Color("#8b949e")).PaddingLeft(2)
	delegate.Styles.SelectedTitle = lipgloss.NewStyle().Border(lipgloss.NormalBorder(), false, false, false, true).BorderForeground(lipgloss.Color("#58a6ff")).Foreground(lipgloss.Color("#58a6ff")).Bold(true).PaddingLeft(1)
	delegate.Styles.SelectedDesc = delegate.Styles.SelectedTitle.Foreground(lipgloss.Color("#c9d1d9"))
	sessions := list.New(nil, delegate, 0, 0)
	sessions.Title = "Sessions"
	sessions.SetStatusBarItemName("session", "sessions")
	sessions.FilterInput.Prompt = "Search: "
	sessions.FilterInput.CharLimit = 240
	sessions.KeyMap.Quit.SetEnabled(false)
	sessions.KeyMap.ForceQuit.SetEnabled(false)
	sessions.KeyMap.AcceptWhileFiltering = key.NewBinding(key.WithKeys("enter"), key.WithHelp("enter", "apply filter"))
	sessions.AdditionalShortHelpKeys = func() []key.Binding {
		return []key.Binding{
			key.NewBinding(key.WithKeys("enter"), key.WithHelp("enter", "details")),
			key.NewBinding(key.WithKeys("q"), key.WithHelp("q", "quit")),
		}
	}
	return &Model{
		ctx: ctx, cancel: cancel, factory: factory, list: sessions,
		spinner: spinner.New(spinner.WithSpinner(spinner.Dot)), viewport: viewport.New(),
		seenIDs: make(map[domain.SessionID]struct{}), seenCursors: make(map[string]struct{}), loading: true,
	}
}

func (model *Model) Init() tea.Cmd {
	return tea.Batch(func() tea.Msg { return model.spinner.Tick() }, model.list.StartSpinner(), model.startSourceCmd())
}

func (model *Model) Update(message tea.Msg) (tea.Model, tea.Cmd) {
	switch message := message.(type) {
	case spinner.TickMsg:
		var spinnerCommand, listCommand tea.Cmd
		model.spinner, spinnerCommand = model.spinner.Update(message)
		model.list, listCommand = model.list.Update(message)
		if model.loading {
			return model, tea.Batch(spinnerCommand, listCommand)
		}
		return model, listCommand
	case tea.WindowSizeMsg:
		model.width, model.height = message.Width, message.Height
		model.resize()
		return model, nil
	case sourceReadyMsg:
		if message.err != nil {
			model.loading = false
			model.list.StopSpinner()
			model.failure = domain.SanitizeTerminalText(message.err.Error())
			return model, nil
		}
		model.source = message.source
		return model, model.loadPageCmd()
	case pageMsg:
		return model.handlePage(message)
	case list.FilterMatchesMsg:
		var command tea.Cmd
		model.list, command = model.list.Update(message)
		model.restoreAfterFilter()
		return model, command
	case tea.KeyPressMsg:
		return model.handleKey(message)
	}
	if model.screen == detailsScreen {
		var command tea.Cmd
		model.viewport, command = model.viewport.Update(message)
		return model, command
	}
	var command tea.Cmd
	model.list, command = model.list.Update(message)
	model.syncSelection()
	return model, command
}

func (model *Model) View() tea.View {
	view := tea.NewView(lipgloss.JoinVertical(lipgloss.Left, model.header(), model.content()))
	view.AltScreen = true
	view.WindowTitle = "Peek Codex"
	return view
}

func (model *Model) Close() {
	model.cancel()
	if model.source != nil {
		_ = model.source.Close()
	}
}

func (model *Model) handleKey(message tea.KeyPressMsg) (tea.Model, tea.Cmd) {
	keyMessage := message.Key()
	if message.String() == "ctrl+c" {
		return model, tea.Quit
	}
	if model.screen == detailsScreen {
		if keyMessage.Code == tea.KeyEscape {
			model.screen = sessionsScreen
			return model, nil
		}
		if keyMessage.Text == "q" {
			return model, tea.Quit
		}
		var command tea.Cmd
		model.viewport, command = model.viewport.Update(message)
		return model, command
	}
	if model.list.FilterState() == list.Filtering {
		switch keyMessage.Code {
		case tea.KeyUp:
			model.list.CursorUp()
			model.syncSelection()
			return model, nil
		case tea.KeyDown:
			model.list.CursorDown()
			model.syncSelection()
			return model, nil
		case tea.KeyHome:
			model.list.GoToStart()
			model.syncSelection()
			return model, nil
		case tea.KeyEnd:
			model.list.GoToEnd()
			model.syncSelection()
			return model, nil
		}
	} else if keyMessage.Text == "q" {
		return model, tea.Quit
	} else if model.list.FilterState() == list.Unfiltered {
		if keyMessage.Code == tea.KeyEscape {
			return model, tea.Quit
		}
		if keyMessage.Code == tea.KeyEnter {
			model.openDetails()
			return model, nil
		}
	} else if keyMessage.Code == tea.KeyEnter {
		model.openDetails()
		return model, nil
	}
	var command tea.Cmd
	model.list, command = model.list.Update(message)
	model.syncSelection()
	return model, command
}

func (model *Model) handlePage(message pageMsg) (tea.Model, tea.Cmd) {
	if message.err != nil {
		model.loading = false
		model.list.StopSpinner()
		if len(model.sessions) == 0 {
			model.failure = domain.SanitizeTerminalText(message.err.Error())
		} else {
			model.warning = domain.SanitizeTerminalText(message.err.Error())
		}
		return model, nil
	}
	previous := model.selectedID
	for _, session := range message.page.Sessions {
		if _, exists := model.seenIDs[session.ID]; !exists {
			model.seenIDs[session.ID] = struct{}{}
			model.sessions = append(model.sessions, session)
		}
	}
	domain.SortSessions(model.sessions)
	model.restoreID = previous
	command := model.list.SetItems(sessionItems(model.sessions))
	if model.list.FilterState() == list.Unfiltered {
		model.syncSelection()
	}
	if message.page.NextCursor == "" {
		model.loading = false
		model.list.StopSpinner()
		return model, command
	}
	if _, repeated := model.seenCursors[message.page.NextCursor]; repeated {
		model.loading = false
		model.list.StopSpinner()
		model.warning = "Pagination stopped after a repeated cursor."
		return model, command
	}
	model.seenCursors[message.page.NextCursor] = struct{}{}
	model.nextCursor = message.page.NextCursor
	return model, tea.Batch(command, model.loadPageCmd())
}

func (model *Model) startSourceCmd() tea.Cmd {
	return func() tea.Msg {
		source, err := model.factory(model.ctx)
		return sourceReadyMsg{source: source, err: err}
	}
}

func (model *Model) loadPageCmd() tea.Cmd {
	return func() tea.Msg {
		page, err := model.source.ListSessions(model.ctx, model.nextCursor)
		return pageMsg{page: page, err: err}
	}
}

func (model *Model) resize() {
	available := max(model.height-lipgloss.Height(model.header())-1, 1)
	model.list.SetSize(model.width, available)
	model.viewport.SetWidth(model.width)
	model.viewport.SetHeight(available)
	if model.screen == detailsScreen && model.hasDetail {
		model.viewport.SetContent(model.detailsContent(model.detail))
	}
}

func (model *Model) syncSelection() {
	model.restoreSelection()
	if item, ok := model.list.SelectedItem().(sessionItem); ok {
		model.selectedID = item.session.ID
	}
}

func (model *Model) restoreSelection() {
	if model.restoreID == "" {
		return
	}
	for index, item := range model.list.VisibleItems() {
		if session, ok := item.(sessionItem); ok && session.session.ID == model.restoreID {
			model.list.Select(index)
			model.selectedID, model.restoreID = model.restoreID, ""
			return
		}
	}
	if model.list.FilterState() == list.Unfiltered {
		model.restoreID = ""
	}
}

func (model *Model) restoreAfterFilter() {
	model.restoreSelection()
	if model.restoreID != "" {
		model.restoreID = ""
		if item, ok := model.list.SelectedItem().(sessionItem); ok {
			model.selectedID = item.session.ID
		}
	}
}

func (model *Model) openDetails() {
	item, ok := model.list.SelectedItem().(sessionItem)
	if !ok {
		return
	}
	model.showDetails(item.session)
}

func (model *Model) showDetails(session domain.Session) {
	model.selectedID = session.ID
	model.detail = session
	model.hasDetail = true
	model.screen = detailsScreen
	model.viewport.SetContent(model.detailsContent(model.detail))
	model.viewport.GotoTop()
}

func (model *Model) detailsContent(session domain.Session) string {
	project := model.project(session.CWD)
	branches := "—"
	if len(project.branches) > 0 {
		branches = strings.Join(project.branches, ", ")
	}
	cardWidth := max(model.width-6, 1)
	if model.width >= 88 {
		cardWidth = max((model.width-6)/2, 1)
	}
	panels := []string{
		detailPanel("Project overview", cardWidth, []detailField{
			{"Project", project.label},
			{"Sessions", fmt.Sprintf("%d loaded", project.count)},
			{"Latest activity", domain.FormatAge(project.latest, time.Now()) + " ago"},
		}),
		detailPanel("Project signals", cardWidth, []detailField{
			{"Providers", strings.Join(project.providers, ", ")},
			{"Statuses", strings.Join(project.statuses, ", ")},
			{"Branches", branches},
		}),
		detailPanel("Selected session", cardWidth, []detailField{
			{"Name", session.Title()},
			{"Project", session.ProjectLabel()},
			{"Path", session.CWD},
			{"Thread ID", session.ID.Display()},
		}),
		detailPanel("Session activity", cardWidth, []detailField{
			{"Provider", domain.DisplayProvider(session.Provider)},
			{"Status", domain.DisplayStatus(session.Status)},
			{"Branch", emptyAsDash(session.Branch)},
			{"Created", domain.FormatAge(session.CreatedAt, time.Now()) + " ago"},
			{"Updated", domain.FormatAge(session.UpdatedAt, time.Now()) + " ago"},
		}),
	}
	content := lipgloss.JoinVertical(lipgloss.Left, panels...)
	if model.width >= 88 {
		content = lipgloss.JoinVertical(lipgloss.Left,
			lipgloss.JoinHorizontal(lipgloss.Top, panels[0], "  ", panels[1]),
			lipgloss.JoinHorizontal(lipgloss.Top, panels[2], "  ", panels[3]),
		)
	}
	return content + "\n\nProject panels use loaded metadata only; conversation turns and agent output are not shown.\n\nEsc back  q/Ctrl-C quit"
}

type detailField struct{ label, value string }

func detailPanel(title string, width int, fields []detailField) string {
	valueWidth := max(width-18, 1)
	lines := make([]string, 0, len(fields)+1)
	lines = append(lines, lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("#58a6ff")).Render(title))
	for _, field := range fields {
		value := ansi.Truncate(domain.SanitizeTerminalText(field.value), valueWidth, "…")
		lines = append(lines, lipgloss.NewStyle().Foreground(lipgloss.Color("#8b949e")).Render(field.label+": ")+value)
	}
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("#30363d")).
		Padding(0, 1).
		Width(width).
		Render(strings.Join(lines, "\n"))
}

type projectSummary struct {
	label                         string
	count                         int
	latest                        int64
	providers, statuses, branches []string
}

func (model *Model) project(cwd string) projectSummary {
	providers, statuses, branches := map[string]struct{}{}, map[string]struct{}{}, map[string]struct{}{}
	var summary projectSummary
	summary.label = domain.ProjectLabel(cwd)
	for _, session := range model.sessions {
		if session.CWD != cwd {
			continue
		}
		summary.count++
		summary.latest = max(summary.latest, session.RecencyAt)
		providers[domain.DisplayProvider(session.Provider)] = struct{}{}
		statuses[domain.DisplayStatus(session.Status)] = struct{}{}
		if session.Branch != "" {
			branches[session.Branch] = struct{}{}
		}
	}
	summary.providers, summary.statuses, summary.branches = sortedKeys(providers), sortedKeys(statuses), sortedKeys(branches)
	return summary
}

func (model *Model) header() string {
	status := "connecting to Codex"
	switch {
	case model.failure != "":
		status = "integration error"
	case model.loading:
		status = fmt.Sprintf("%d loaded · loading more", len(model.sessions))
	case len(model.sessions) == 0:
		status = "no interactive sessions found"
	default:
		status = fmt.Sprintf("%d loaded · %d projects", len(model.sessions), model.projectCount())
	}
	screenLabel := "Sessions"
	if model.screen == detailsScreen {
		screenLabel = "Session details"
	}
	line := lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("#58a6ff")).Render("Peek Codex / "+screenLabel) + "  " + lipgloss.NewStyle().Foreground(lipgloss.Color("#8b949e")).Render(status)
	if model.screen == sessionsScreen && model.list.FilterState() == list.Unfiltered {
		line += "\nBrowse safe project metadata · / fuzzy search · Enter project details"
	}
	if model.warning != "" {
		line += "\n" + lipgloss.NewStyle().Foreground(lipgloss.Color("#d29922")).Render("Partial results: "+model.warning)
	}
	return lipgloss.NewStyle().Border(lipgloss.NormalBorder(), false, false, true, false).BorderForeground(lipgloss.Color("#30363d")).Width(max(model.width, 1)).Render(line)
}

func (model *Model) content() string {
	if model.failure != "" {
		return lipgloss.NewStyle().Foreground(lipgloss.Color("#f85149")).Padding(1, 2).Render("Could not load sessions.\n\n" + model.failure)
	}
	if model.screen == detailsScreen {
		return model.viewport.View()
	}
	if model.loading && len(model.sessions) == 0 {
		return lipgloss.NewStyle().Padding(1, 2).Render(model.spinner.View() + " Loading recent Codex sessions…")
	}
	if !model.loading && len(model.sessions) == 0 {
		return lipgloss.NewStyle().Padding(1, 2).Render("No interactive Codex sessions are available.")
	}
	return model.list.View()
}

func (model *Model) projectCount() int {
	projects := make(map[string]struct{})
	for _, session := range model.sessions {
		projects[session.CWD] = struct{}{}
	}
	return len(projects)
}

type sessionItem struct{ session domain.Session }

type sessionDelegate struct{ list.DefaultDelegate }

func (delegate sessionDelegate) Render(writer io.Writer, model list.Model, index int, item list.Item) {
	session, ok := item.(sessionItem)
	if ok && session.session.Branch != "" {
		fullWidth := lipgloss.Width(session.description(true))
		available := max(model.Width()-4, 0)
		if fullWidth > available {
			session.session.Branch = ""
			item = session
		}
	}
	delegate.DefaultDelegate.Render(writer, model, index, item)
}

func (item sessionItem) Title() string {
	return fmt.Sprintf("%-4s  %s", domain.FormatAge(item.session.RecencyAt, time.Now()), item.session.Title())
}

func (item sessionItem) Description() string {
	return item.description(true)
}

func (item sessionItem) description(includeBranch bool) string {
	metadata := item.session.ProjectLabel() + "  ·  " + domain.DisplayProvider(item.session.Provider) + "  ·  " + domain.DisplayStatus(item.session.Status)
	if includeBranch && item.session.Branch != "" {
		metadata += "  [" + item.session.Branch + "]"
	}
	return metadata
}

func (item sessionItem) FilterValue() string {
	return strings.Join([]string{item.session.Title(), item.session.ProjectLabel(), item.session.Branch, item.session.Provider, item.session.Status, item.session.CWD}, " ")
}

func sessionItems(sessions []domain.Session) []list.Item {
	items := make([]list.Item, 0, len(sessions))
	for _, session := range sessions {
		items = append(items, sessionItem{session: session})
	}
	return items
}

func emptyAsDash(value string) string {
	if value == "" {
		return "—"
	}
	return value
}

func plural(count int, singular, multiple string) string {
	if count == 1 {
		return singular
	}
	return multiple
}

func sortedKeys(values map[string]struct{}) []string {
	result := make([]string, 0, len(values))
	for value := range values {
		result = append(result, value)
	}
	sort.Strings(result)
	return result
}
