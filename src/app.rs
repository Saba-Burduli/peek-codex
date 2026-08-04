use crate::domain::{Session, SessionPage, sanitize_terminal_text};
use std::collections::HashSet;

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
}

impl Default for App {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            selected: None,
            state: LoadState::Loading,
            loading_more: true,
            warning: None,
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
        if self.sessions.is_empty() {
            self.selected = None;
            return;
        }
        let last = self.sessions.len() - 1;
        self.selected = Some(match (self.selected.unwrap_or(0), navigation) {
            (_, Navigation::Home) => 0,
            (_, Navigation::End) => last,
            (current, Navigation::Up) => current.saturating_sub(1),
            (current, Navigation::Down) => current.saturating_add(1).min(last),
        });
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
