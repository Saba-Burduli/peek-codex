use crate::app::{App, LoadState, Navigation, WorkerMessage};
use crate::codex::{AppServerSource, CodexSessionSource};
use crate::domain::{Session, format_age};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::collections::HashSet;
use std::io::{self, IsTerminal};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn run() -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "an interactive terminal is required; run `peek-codex` directly in a terminal"
                .to_owned(),
        );
    }

    let guard =
        TerminalGuard::enter().map_err(|error| format!("terminal setup failed: {error}"))?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)
        .map_err(|error| format!("could not initialize terminal: {error}"))?;
    let mut loader = Loader::start();
    let mut app = App::default();
    let result = event_loop(&mut terminal, &mut app, &loader.receiver);
    loader.cancel();
    drop(terminal);
    drop(guard);
    result.and(loader.join())
}

struct Loader {
    receiver: mpsc::Receiver<WorkerMessage>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Loader {
    fn start() -> Self {
        Self::start_with_program(Path::new("codex"), crate::codex::DEFAULT_REQUEST_TIMEOUT)
    }

    fn start_with_program(program: &Path, request_timeout: Duration) -> Self {
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let program = program.to_owned();
        let thread = thread::spawn(move || {
            let mut source = match AppServerSource::spawn_program_with_control(
                &program,
                request_timeout,
                Arc::clone(&worker_stop),
            ) {
                Ok(source) => source,
                Err(error) => {
                    let _ = sender.send(WorkerMessage::Failed(error.to_string()));
                    return;
                }
            };
            let mut cursor = None;
            let mut seen_cursors = HashSet::new();

            loop {
                if worker_stop.load(Ordering::Relaxed) {
                    return;
                }
                let page = match source.list_sessions(cursor.as_deref()) {
                    Ok(page) => page,
                    Err(error) => {
                        let _ = sender.send(WorkerMessage::Failed(error.to_string()));
                        return;
                    }
                };
                let next_cursor = page.next_cursor.clone();
                if sender.send(WorkerMessage::Page(page)).is_err() {
                    return;
                }
                match next_cursor {
                    None => {
                        let _ = sender.send(WorkerMessage::Complete);
                        return;
                    }
                    Some(next_cursor) if !seen_cursors.insert(next_cursor.clone()) => {
                        let _ = sender.send(WorkerMessage::Failed(
                            "app-server returned a repeated pagination cursor".to_owned(),
                        ));
                        return;
                    }
                    Some(next_cursor) => cursor = Some(next_cursor),
                }
            }
        });
        Self {
            receiver,
            stop,
            thread: Some(thread),
        }
    }

    fn cancel(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    fn join(&mut self) -> Result<(), String> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        thread
            .join()
            .map_err(|_| "session loader thread panicked".to_owned())
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel();
        let _ = self.join();
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    receiver: &mpsc::Receiver<WorkerMessage>,
) -> Result<(), String> {
    loop {
        for message in receiver.try_iter() {
            app.apply(message);
        }
        terminal
            .draw(|frame| render(frame, app))
            .map_err(|error| format!("terminal draw failed: {error}"))?;

        if event::poll(Duration::from_millis(100))
            .map_err(|error| format!("terminal event poll failed: {error}"))?
            && let Event::Key(key) =
                event::read().map_err(|error| format!("terminal input failed: {error}"))?
        {
            if should_quit(key) {
                return Ok(());
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => app.navigate(Navigation::Up),
                KeyCode::Down | KeyCode::Char('j') => app.navigate(Navigation::Down),
                KeyCode::Home => app.navigate(Navigation::Home),
                KeyCode::End => app.navigate(Navigation::End),
                _ => {}
            }
        }
    }
}

fn should_quit(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn render(frame: &mut ratatui::Frame<'_>, app: &App) {
    let footer_height = if app.warning().is_some() { 2 } else { 1 };
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(footer_height),
    ])
    .split(frame.area());
    let status = match app.state() {
        LoadState::Loading => "loading sessions".to_owned(),
        LoadState::Ready if app.warning().is_some() => {
            format!("{} sessions · partial", app.sessions().len())
        }
        LoadState::Ready if app.loading_more() => {
            format!("{} sessions · loading more", app.sessions().len())
        }
        LoadState::Ready => format!("{} sessions", app.sessions().len()),
        LoadState::Empty => "no interactive sessions found".to_owned(),
        LoadState::Failed(_) => "integration error".to_owned(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Peek Codex", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(status, Style::default().fg(Color::DarkGray)),
        ]))
        .block(Block::default().borders(Borders::BOTTOM)),
        areas[0],
    );

    match app.state() {
        LoadState::Failed(message) => frame.render_widget(
            Paragraph::new(format!("Could not load sessions.\n\n{message}"))
                .block(Block::bordered().title("Error"))
                .style(Style::default().fg(Color::Red)),
            areas[1],
        ),
        LoadState::Empty => frame.render_widget(
            Paragraph::new("No interactive Codex sessions are available.")
                .block(Block::bordered().title("Sessions")),
            areas[1],
        ),
        LoadState::Loading if app.sessions().is_empty() => frame.render_widget(
            Paragraph::new("Loading recent Codex sessions…")
                .block(Block::bordered().title("Sessions")),
            areas[1],
        ),
        _ => render_list(frame, areas[1], app),
    }

    let keys = "↑/k ↓/j navigate  Home/End jump  q/Esc/Ctrl-C quit";
    let footer = app
        .warning()
        .map(|warning| format!("Partial results: {warning}\n{keys}"))
        .unwrap_or_else(|| keys.to_owned());
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(if app.warning().is_some() {
            Color::Yellow
        } else {
            Color::DarkGray
        })),
        areas[2],
    );
}

fn render_list(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, app: &App) {
    let width = area.width.saturating_sub(4) as usize;
    let items: Vec<_> = app
        .sessions()
        .iter()
        .map(|session| ListItem::new(session_line(session, width)))
        .collect();
    let list = List::new(items)
        .block(Block::bordered().title("Sessions"))
        .highlight_symbol("› ")
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default().with_selected(app.selected());
    frame.render_stateful_widget(list, area, &mut state);
}

fn session_line(session: &Session, width: usize) -> Line<'static> {
    let (metadata, preview) = session_parts(session, width);
    Line::from(vec![
        Span::styled(metadata, Style::default().fg(Color::Cyan)),
        Span::raw(preview),
    ])
}

fn session_parts(session: &Session, width: usize) -> (String, String) {
    let age = format_age(session.recency_at, SystemTime::now());
    let age = left_pad(&truncate(&age, 4), 4);
    let age_segment = format!("{age}  ");
    if width < display_width(&age_segment) {
        return (truncate(&age_segment, width), String::new());
    }
    let available_after_age = width.saturating_sub(display_width(&age_segment));
    let branch_segment = session
        .branch
        .as_deref()
        .filter(|branch| width >= 70 && !branch.is_empty())
        .map(|branch| format!(" [{}]  ", truncate(branch, 20)))
        .unwrap_or_default();
    let preview_reserve = 8;
    let project_budget = available_after_age
        .saturating_sub(display_width(&branch_segment))
        .saturating_sub(preview_reserve)
        .min(24);
    let project = session.project_label();
    let project_segment = if project_budget == 0 || project.is_empty() {
        String::new()
    } else {
        format!("{}  ", truncate(&project, project_budget))
    };
    let metadata = format!("{age_segment}{project_segment}{branch_segment}");
    let available = width.saturating_sub(display_width(&metadata));
    let preview = truncate(&session.preview, available);
    (metadata, preview)
}

fn truncate(value: &str, width: usize) -> String {
    if display_width(value) <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_owned();
    }
    let content_width = width - 1;
    let mut result = String::new();
    let mut used_width = 0;
    for character in value.chars() {
        let character_width = character.width().unwrap_or(0);
        if character_width == 0 && result.is_empty() {
            continue;
        }
        if used_width + character_width > content_width {
            break;
        }
        result.push(character);
        used_width += character_width;
    }
    result.push('…');
    result
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

fn left_pad(value: &str, width: usize) -> String {
    format!(
        "{}{}",
        " ".repeat(width.saturating_sub(display_width(value))),
        value
    )
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SessionId;

    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::process::{Command, Stdio};
    #[cfg(unix)]
    use std::time::Instant;

    #[test]
    fn truncates_at_terminal_width() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("hello", 1), "…");
        assert_eq!(truncate("hello", 0), "");

        let wide = truncate("界界", 3);
        assert_eq!(wide, "界…");
        assert_eq!(display_width(&wide), 3);

        let combining = truncate("e\u{301}xy", 2);
        assert_eq!(combining, "e\u{301}…");
        assert_eq!(display_width(&combining), 2);
        assert_eq!(truncate("\u{301}界x", 2), "…");
    }

    #[test]
    fn session_parts_fit_narrow_rows_and_prioritize_preview() {
        let session = session(
            "/tmp/界界界-project",
            "preview with wide 文字",
            Some("feature/界界界"),
        );

        for width in 0..=16 {
            let (metadata, preview) = session_parts(&session, width);
            assert!(display_width(&format!("{metadata}{preview}")) <= width);
        }

        let (metadata, preview) = session_parts(&session, 16);

        assert!(!preview.is_empty());
        assert!(!metadata.contains('['));
    }

    #[test]
    fn session_parts_cap_metadata_at_display_cell_width() {
        let session = session(
            "/tmp/界界界界界界界界界界界界",
            "preview with wide 文字",
            Some("feature/界界界界界界界界界界界界"),
        );

        let (metadata, preview) = session_parts(&session, 69);
        assert!(!metadata.contains('['));
        assert!(metadata.contains('界'));
        assert!(display_width(&format!("{metadata}{preview}")) <= 69);

        let (metadata, preview) = session_parts(&session, 70);

        assert!(metadata.contains('['));
        assert!(display_width(&format!("{metadata}{preview}")) <= 70);
        assert!(!preview.is_empty());
    }

    fn session(cwd: &str, preview: &str, branch: Option<&str>) -> Session {
        Session {
            id: SessionId::new("row").unwrap(),
            name: None,
            preview: preview.to_owned(),
            cwd: cwd.to_owned(),
            created_at: 0,
            updated_at: 0,
            recency_at: 0,
            provider: String::new(),
            status: String::new(),
            branch: branch.map(str::to_owned),
        }
    }

    #[test]
    fn recognizes_every_exit_key() {
        assert!(should_quit(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        )));
        assert!(should_quit(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(should_quit(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_joins_loader_and_stops_child() {
        let directory =
            std::env::temp_dir().join(format!("peek-codex-loader-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("codex");
        let pid_file = directory.join("pid");
        let script = format!(
            r#"#!/bin/sh
printf '%s' "$$" > '{}'
IFS= read -r initialize || exit 1
printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{"userAgent":"fake"}}}}'
IFS= read -r initialized || exit 1
IFS= read -r list || exit 1
while IFS= read -r ignored; do :; done
"#,
            pid_file.display()
        );
        fs::write(&executable, script).unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let mut loader = Loader::start_with_program(&executable, Duration::from_secs(5));
        for _ in 0..100 {
            if pid_file.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let pid = fs::read_to_string(&pid_file).unwrap();
        let started = Instant::now();
        loader.cancel();
        loader.join().unwrap();

        assert!(started.elapsed() < Duration::from_secs(1));
        let child_is_running = Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        assert!(!child_is_running, "fake app-server child remained alive");
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn repeated_cursor_stops_pagination_without_third_request() {
        let directory =
            std::env::temp_dir().join(format!("peek-codex-cursor-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("codex");
        let requests = directory.join("requests");
        let script = format!(
            r#"#!/bin/sh
read_request() {{
  IFS= read -r line || exit 1
  printf '%s\n' "$line" >> '{}'
}}
read_request
printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{"userAgent":"fake"}}}}'
read_request
read_request
printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"data":[],"nextCursor":"repeat"}}}}'
read_request
printf '%s\n' '{{"jsonrpc":"2.0","id":3,"result":{{"data":[],"nextCursor":"repeat"}}}}'
while IFS= read -r ignored; do :; done
"#,
            requests.display()
        );
        fs::write(&executable, script).unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let mut loader = Loader::start_with_program(&executable, Duration::from_secs(2));
        let messages: Vec<_> = (0..3)
            .map(|_| {
                loader
                    .receiver
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap()
            })
            .collect();
        loader.join().unwrap();

        assert!(matches!(messages[0], WorkerMessage::Page(_)));
        assert!(matches!(messages[1], WorkerMessage::Page(_)));
        assert!(matches!(
            &messages[2],
            WorkerMessage::Failed(message) if message.contains("repeated pagination cursor")
        ));
        let captured = fs::read_to_string(&requests).unwrap();
        assert_eq!(captured.matches("\"method\":\"thread/list\"").count(), 2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn valid_second_page_rpc_error_becomes_sanitized_partial_warning() {
        let directory =
            std::env::temp_dir().join(format!("peek-codex-rpc-error-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("codex");
        let hostile = format!("second page\n\u{1b}[31m{}", "x".repeat(300));
        let error_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "error": {"code": -32603, "message": hostile}
        })
        .to_string();
        let script = format!(
            r#"#!/bin/sh
IFS= read -r initialize || exit 1
printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{"userAgent":"fake"}}}}'
IFS= read -r initialized || exit 1
IFS= read -r first_page || exit 1
printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"data":[{{"id":"loaded","name":null,"preview":"Loaded","cwd":"/tmp/project","createdAt":1,"updatedAt":2,"recencyAt":2,"modelProvider":"openai","source":"cli","status":{{"type":"notLoaded"}},"gitInfo":null}}],"nextCursor":"next"}}}}'
IFS= read -r second_page || exit 1
printf '%s\n' '{}'
"#,
            error_response
        );
        fs::write(&executable, script).unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let mut loader = Loader::start_with_program(&executable, Duration::from_secs(2));
        let mut app = App::default();
        for _ in 0..2 {
            app.apply(
                loader
                    .receiver
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap(),
            );
        }
        loader.join().unwrap();

        assert_eq!(app.state(), &LoadState::Ready);
        assert_eq!(app.sessions()[0].id.as_str(), "loaded");
        let warning = app.warning().expect("partial warning");
        assert!(!warning.contains('\n'));
        assert!(!warning.contains('\u{1b}'));
        assert!(warning.chars().count() <= 240);
        fs::remove_dir_all(directory).unwrap();
    }
}
