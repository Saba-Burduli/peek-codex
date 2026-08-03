use crate::domain::{Session, SessionPage};
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            selected: None,
            state: LoadState::Loading,
            loading_more: true,
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

    pub fn apply(&mut self, message: WorkerMessage) {
        match message {
            WorkerMessage::Page(page) => self.append_page(page),
            WorkerMessage::Failed(message) => {
                self.loading_more = false;
                self.state = LoadState::Failed(message);
            }
            WorkerMessage::Complete => {
                self.loading_more = false;
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
        if self.selected.is_none() && !self.sessions.is_empty() {
            self.selected = Some(0);
        }
        self.state = if self.sessions.is_empty() {
            LoadState::Loading
        } else {
            LoadState::Ready
        };
        self.loading_more = page.next_cursor.is_some();
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
}
