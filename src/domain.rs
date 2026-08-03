use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError("session id is empty"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub name: Option<String>,
    pub preview: String,
    pub cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub recency_at: i64,
    pub provider: String,
    pub status: String,
    pub branch: Option<String>,
}

impl Session {
    pub fn project_label(&self) -> String {
        project_label(&self.cwd)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPage {
    pub sessions: Vec<Session>,
    pub next_cursor: Option<String>,
}

impl SessionPage {
    pub fn sort_by_recency(&mut self) {
        self.sessions.sort_by(|left, right| {
            right
                .recency_at
                .cmp(&left.recency_at)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
                .then_with(|| left.id.as_str().cmp(right.id.as_str()))
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainError(&'static str);

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for DomainError {}

pub fn sanitize_terminal_text(value: &str) -> String {
    let mut result = String::with_capacity(value.len().min(240));
    let mut pending_space = false;

    for character in value.chars() {
        if character.is_control() || character.is_whitespace() {
            pending_space = !result.is_empty();
            continue;
        }
        if pending_space {
            result.push(' ');
            pending_space = false;
        }
        if result.chars().count() == 240 {
            break;
        }
        result.push(character);
    }

    result
}

pub fn project_label(cwd: &str) -> String {
    let path = Path::new(cwd);
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(cwd)
        .to_owned()
}

pub fn format_age(timestamp: i64, now: SystemTime) -> String {
    let now = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64);
    let seconds = now.saturating_sub(timestamp).max(0);

    match seconds {
        0..=59 => "now".to_owned(),
        60..=3_599 => format!("{}m", seconds / 60),
        3_600..=86_399 => format!("{}h", seconds / 3_600),
        86_400..=2_591_999 => format!("{}d", seconds / 86_400),
        _ => format!("{}mo", seconds / 2_592_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn session(id: &str, recency_at: i64, updated_at: i64) -> Session {
        Session {
            id: SessionId::new(id).unwrap(),
            name: None,
            preview: String::new(),
            cwd: String::new(),
            created_at: 0,
            updated_at,
            recency_at,
            provider: String::new(),
            status: String::new(),
            branch: None,
        }
    }

    #[test]
    fn sanitizes_control_characters_and_collapses_lines() {
        assert_eq!(
            sanitize_terminal_text(" hello\n\u{1b}[31m\tworld\r "),
            "hello [31m world"
        );
    }

    #[test]
    fn truncates_sanitized_text_by_characters() {
        let result = sanitize_terminal_text(&"é".repeat(300));
        assert_eq!(result.chars().count(), 240);
    }

    #[test]
    fn derives_project_from_working_directory() {
        assert_eq!(project_label("/Users/example/project"), "project");
        assert_eq!(project_label("/"), "/");
        assert_eq!(project_label(""), "");
    }

    #[test]
    fn formats_relative_ages() {
        let now = UNIX_EPOCH + Duration::from_secs(10_000_000);
        assert_eq!(format_age(9_999_990, now), "now");
        assert_eq!(format_age(9_999_880, now), "2m");
        assert_eq!(format_age(9_992_800, now), "2h");
        assert_eq!(format_age(9_827_200, now), "2d");
        assert_eq!(format_age(4_816_000, now), "2mo");
        assert_eq!(format_age(10_000_100, now), "now");
    }

    #[test]
    fn orders_sessions_by_recency_with_stable_tie_breakers() {
        let mut page = SessionPage {
            sessions: vec![
                session("b", 10, 20),
                session("c", 20, 5),
                session("a", 10, 20),
            ],
            next_cursor: None,
        };

        page.sort_by_recency();

        let ids: Vec<_> = page.sessions.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["c", "a", "b"]);
    }
}
