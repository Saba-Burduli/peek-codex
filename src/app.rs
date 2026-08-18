use crate::domain::{Session, SessionId, SessionPage, sanitize_terminal_text};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadState {
    Loading,
    Ready,
    Empty,
    Failed(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Navigation {
    Up,
    Down,
    Home,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Sessions,
    Details,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOverview {
    pub label: String,
    pub session_count: usize,
    pub latest_activity: i64,
    pub providers: Vec<String>,
    pub statuses: Vec<String>,
    pub branches: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerMessage {
    Page(SessionPage),
    Failed(String),
    Complete,
}

#[derive(Debug)]
pub struct App {
    sessions: Vec<Session>,
    selected: Option<usize>,
    state: LoadState,
    loading_more: bool,
    warning: Option<String>,
    search: String,
    searching: bool,
    search_restore_id: Option<SessionId>,
    view: View,
}

impl Default for App {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            selected: None,
            state: LoadState::Loading,
            loading_more: true,
            warning: None,
            search: String::new(),
            searching: false,
            search_restore_id: None,
            view: View::Sessions,
        }
    }
}

impl App {
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn state(&self) -> &LoadState {
        &self.state
    }

    pub fn loading_more(&self) -> bool {
        self.loading_more
    }

    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    pub fn is_searching(&self) -> bool {
        self.searching
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        let query = self.search.to_lowercase();
        self.sessions
            .iter()
            .enumerate()
            .filter_map(|(index, session)| session_matches(session, &query).then_some(index))
            .collect()
    }

    pub fn selected_filtered_index(&self) -> Option<usize> {
        let selected = self.selected?;
        self.filtered_indices()
            .iter()
            .position(|index| *index == selected)
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.selected.and_then(|index| self.sessions.get(index))
    }

    pub fn project_count(&self) -> usize {
        self.sessions
            .iter()
            .map(|session| session.cwd.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    }

    pub fn selected_project_overview(&self) -> Option<ProjectOverview> {
        let cwd = self.selected_session()?.cwd.clone();
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|session| session.cwd == cwd)
            .collect();
        let latest_activity = sessions.iter().map(|session| session.recency_at).max()?;
        Some(ProjectOverview {
            label: sessions[0].project_label(),
            session_count: sessions.len(),
            latest_activity,
            providers: sessions
                .iter()
                .map(|session| session.provider.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            statuses: sessions
                .iter()
                .map(|session| session.status.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            branches: sessions
                .iter()
                .filter_map(|session| session.branch.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
    }

    pub fn begin_search(&mut self) {
        if !self.searching {
            self.search_restore_id = self.selected_session().map(|session| session.id.clone());
        }
        self.searching = true;
    }

    pub fn push_search(&mut self, character: char) {
        if self.searching {
            self.search.push(character);
            self.select_first_visible_if_needed();
        }
    }

    pub fn pop_search(&mut self) {
        if self.searching {
            self.search.pop();
            self.select_first_visible_if_needed();
        }
    }

    pub fn cancel_search(&mut self) {
        self.search.clear();
        self.searching = false;
        if let Some(id) = self.search_restore_id.take()
            && let Some(index) = self.sessions.iter().position(|session| session.id == id)
        {
            self.selected = Some(index);
        }
    }

    pub fn open_selected(&mut self) {
        if self.selected_filtered_index().is_some() {
            self.view = View::Details;
        }
    }

    pub fn close_details(&mut self) {
        self.view = View::Sessions;
    }

    pub fn apply(&mut self, message: WorkerMessage) {
        match message {
            WorkerMessage::Page(page) => self.append_page(page),
            WorkerMessage::Failed(message) => {
                let message = sanitize_failure_message(&message);
                self.loading_more = false;
                if self.sessions.is_empty() {
                    self.state = LoadState::Failed(message);
                } else {
                    self.state = LoadState::Ready;
                    self.warning = Some(message);
                }
            }
            WorkerMessage::Complete => {
                self.loading_more = false;
                self.warning = None;
                self.state = if self.sessions.is_empty() {
                    LoadState::Empty
                } else {
                    LoadState::Ready
                };
            }
        }
    }

    pub fn navigate(&mut self, navigation: Navigation) {
        let visible = self.filtered_indices();
        if visible.is_empty() {
            return;
        }
        let last = visible.len() - 1;
        let current = self.selected_filtered_index().unwrap_or(match navigation {
            Navigation::End => last,
            _ => 0,
        });
        let next = match (current, navigation) {
            (_, Navigation::Home) => 0,
            (_, Navigation::End) => last,
            (current, Navigation::Up) => current.saturating_sub(1),
            (current, Navigation::Down) => current.saturating_add(1).min(last),
        };
        self.selected = Some(visible[next]);
    }

    fn append_page(&mut self, page: SessionPage) {
        let selected_id = self
            .selected
            .and_then(|index| self.sessions.get(index))
            .map(|session| session.id.as_str().to_owned());
        let mut ids: HashSet<_> = self
            .sessions
            .iter()
            .map(|session| session.id.as_str().to_owned())
            .collect();
        self.sessions.extend(
            page.sessions
                .into_iter()
                .filter(|session| ids.insert(session.id.as_str().to_owned())),
        );
        self.sessions.sort_by(|left, right| {
            right
                .recency_at
                .cmp(&left.recency_at)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
        self.selected = selected_id
            .and_then(|selected_id| {
                self.sessions
                    .iter()
                    .position(|session| session.id.as_str() == selected_id)
            })
            .or_else(|| (!self.sessions.is_empty()).then_some(0));
        self.state = if self.sessions.is_empty() {
            LoadState::Loading
        } else {
            LoadState::Ready
        };
        self.loading_more = page.next_cursor.is_some();
        self.warning = None;
    }

    fn select_first_visible_if_needed(&mut self) {
        if self.selected_filtered_index().is_none()
            && let Some(index) = self.filtered_indices().first()
        {
            self.selected = Some(*index);
        }
    }
}

fn session_matches(session: &Session, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    [
        session.name.as_deref().unwrap_or_default(),
        &session.cwd,
        &session.provider,
        &session.status,
        session.branch.as_deref().unwrap_or_default(),
    ]
    .iter()
    .any(|field| field.to_lowercase().contains(query))
}

fn sanitize_failure_message(message: &str) -> String {
    let message = sanitize_terminal_text(message);
    if message.is_empty() {
        "unknown app-server error".to_owned()
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SessionId;

    fn session(id: &str, recency_at: i64) -> Session {
        Session {
            id: SessionId::new(id).unwrap(),
            name: None,
            preview: id.to_owned(),
            cwd: "/tmp/project".to_owned(),
            created_at: recency_at,
            updated_at: recency_at,
            recency_at,
            provider: "openai/cli".to_owned(),
            status: "notLoaded".to_owned(),
            branch: None,
        }
    }

    #[test]
    fn models_empty_and_failure_states() {
        let mut empty = App::default();
        empty.apply(WorkerMessage::Complete);
        assert_eq!(empty.state(), &LoadState::Empty);

        let mut failed = App::default();
        failed.apply(WorkerMessage::Failed("unavailable".to_owned()));
        assert_eq!(failed.state(), &LoadState::Failed("unavailable".to_owned()));
    }

    #[test]
    fn appends_pages_deduplicates_and_preserves_recency_order() {
        let mut app = App::default();
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session("older", 1), session("newer", 3)],
            next_cursor: Some("next".to_owned()),
        }));
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session("older", 1), session("middle", 2)],
            next_cursor: None,
        }));

        let ids: Vec<_> = app
            .sessions()
            .iter()
            .map(|session| session.id.as_str())
            .collect();
        assert_eq!(ids, ["newer", "middle", "older"]);
        assert_eq!(app.selected(), Some(0));
        assert!(!app.loading_more());
    }

    #[test]
    fn keeps_navigation_inside_boundaries() {
        let mut app = App::default();
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session("a", 2), session("b", 1)],
            next_cursor: None,
        }));

        app.navigate(Navigation::Up);
        assert_eq!(app.selected(), Some(0));
        app.navigate(Navigation::End);
        assert_eq!(app.selected(), Some(1));
        app.navigate(Navigation::Down);
        assert_eq!(app.selected(), Some(1));
        app.navigate(Navigation::Home);
        assert_eq!(app.selected(), Some(0));
    }

    #[test]
    fn filters_sessions_and_navigates_only_visible_rows() {
        let mut app = App::default();
        let mut alpha = session("alpha", 3);
        alpha.name = Some("Alpha release".to_owned());
        alpha.branch = Some("feature/search".to_owned());
        let mut beta = session("beta", 2);
        beta.cwd = "/tmp/beta-project".to_owned();
        let gamma = session("gamma", 1);
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![alpha, beta, gamma],
            next_cursor: None,
        }));

        app.begin_search();
        for character in "beta".chars() {
            app.push_search(character);
        }

        assert_eq!(app.filtered_indices(), vec![1]);
        assert_eq!(app.selected_session().unwrap().id.as_str(), "beta");

        app.open_selected();
        assert_eq!(app.view(), View::Details);
        app.close_details();

        app.cancel_search();
        assert_eq!(app.filtered_indices(), vec![0, 1, 2]);
        assert_eq!(app.selected_session().unwrap().id.as_str(), "alpha");
    }

    #[test]
    fn opens_details_only_for_a_visible_selected_session() {
        let mut app = App::default();
        let mut first = session("first", 1);
        first.name = Some("First project check".to_owned());
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![first],
            next_cursor: None,
        }));

        app.begin_search();
        app.push_search('f');
        app.open_selected();
        assert_eq!(app.view(), View::Details);
        assert!(app.is_searching());
        assert_eq!(app.search(), "f");
        app.close_details();
        assert_eq!(app.view(), View::Sessions);

        app.cancel_search();
        app.begin_search();
        app.push_search('z');
        app.open_selected();
        assert_eq!(app.view(), View::Sessions);
    }

    #[test]
    fn builds_project_overview_from_loaded_project_metadata() {
        let mut app = App::default();
        let mut newest = session("newest", 3);
        newest.cwd = "/tmp/petty".to_owned();
        newest.provider = "openai/cli".to_owned();
        newest.status = "notLoaded".to_owned();
        newest.branch = Some("main".to_owned());
        let mut older = session("older", 2);
        older.cwd = "/tmp/petty".to_owned();
        older.provider = "openai/cli".to_owned();
        older.status = "completed".to_owned();
        older.branch = Some("feature/cards".to_owned());
        let mut other = session("other", 1);
        other.cwd = "/tmp/peek-codex".to_owned();
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![newest, older, other],
            next_cursor: None,
        }));

        let overview = app.selected_project_overview().unwrap();
        assert_eq!(app.project_count(), 2);
        assert_eq!(overview.label, "petty");
        assert_eq!(overview.session_count, 2);
        assert_eq!(overview.latest_activity, 3);
        assert_eq!(overview.providers, ["openai/cli"]);
        assert_eq!(overview.statuses, ["completed", "notLoaded"]);
        assert_eq!(overview.branches, ["feature/cards", "main"]);
    }

    #[test]
    fn search_does_not_use_agent_output_preview() {
        let mut app = App::default();
        let mut session = session("title", 1);
        session.name = Some("Project check-in".to_owned());
        session.preview = "agent output that must not be searchable".to_owned();
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session],
            next_cursor: None,
        }));

        app.begin_search();
        for character in "agent output".chars() {
            app.push_search(character);
        }
        assert!(app.filtered_indices().is_empty());
    }

    #[test]
    fn later_failure_keeps_loaded_sessions_usable() {
        let mut app = App::default();
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session("loaded", 1)],
            next_cursor: Some("next".to_owned()),
        }));

        app.apply(WorkerMessage::Failed("second page failed".to_owned()));

        assert_eq!(app.state(), &LoadState::Ready);
        assert_eq!(app.sessions()[0].id.as_str(), "loaded");
        assert_eq!(app.selected(), Some(0));
        assert_eq!(app.warning(), Some("second page failed"));
        assert!(!app.loading_more());
    }

    #[test]
    fn appended_pages_preserve_selected_session_identity() {
        let mut app = App::default();
        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session("selected", 2), session("older", 1)],
            next_cursor: Some("next".to_owned()),
        }));
        app.navigate(Navigation::End);
        app.navigate(Navigation::Up);
        assert_eq!(
            app.sessions()[app.selected().unwrap()].id.as_str(),
            "selected"
        );

        app.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session("newer", 3)],
            next_cursor: None,
        }));

        assert_eq!(
            app.sessions()[app.selected().unwrap()].id.as_str(),
            "selected"
        );
    }

    #[test]
    fn sanitizes_and_bounds_fatal_and_partial_failure_messages() {
        let hostile = format!("first line\n\u{1b}[31m{}", "x".repeat(300));
        let expected = sanitize_terminal_text(&hostile);
        let mut fatal = App::default();

        fatal.apply(WorkerMessage::Failed(hostile.clone()));

        assert_eq!(fatal.state(), &LoadState::Failed(expected.clone()));
        assert!(!expected.contains('\n'));
        assert!(!expected.contains('\u{1b}'));
        assert!(expected.chars().count() <= 240);

        let mut partial = App::default();
        partial.apply(WorkerMessage::Page(SessionPage {
            sessions: vec![session("loaded", 1)],
            next_cursor: Some("next".to_owned()),
        }));
        partial.apply(WorkerMessage::Failed(hostile));
        assert_eq!(partial.warning(), Some(expected.as_str()));
    }

    #[test]
    fn replaces_empty_failure_messages() {
        let mut app = App::default();
        app.apply(WorkerMessage::Failed("\n\t".to_owned()));
        assert_eq!(
            app.state(),
            &LoadState::Failed("unknown app-server error".to_owned())
        );
    }
}
