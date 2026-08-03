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
use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime};

pub fn run() -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "an interactive terminal is required; run `codex-slice` directly in a terminal"
                .to_owned(),
        );
    }

    let stop = Arc::new(AtomicBool::new(false));
    let receiver = spawn_loader(Arc::clone(&stop));
    let guard =
        TerminalGuard::enter().map_err(|error| format!("terminal setup failed: {error}"))?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)
        .map_err(|error| format!("could not initialize terminal: {error}"))?;
    let mut app = App::default();
    let result = event_loop(&mut terminal, &mut app, receiver);
    stop.store(true, Ordering::Relaxed);
    drop(terminal);
    drop(guard);
    result
}

fn spawn_loader(stop: Arc<AtomicBool>) -> mpsc::Receiver<WorkerMessage> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut source = match AppServerSource::spawn() {
            Ok(source) => source,
            Err(error) => {
                let _ = sender.send(WorkerMessage::Failed(error.to_string()));
                return;
            }
        };
        let mut cursor = None;

        loop {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let page = match source.list_sessions(cursor.as_deref()) {
                Ok(page) => page,
                Err(error) => {
                    let _ = sender.send(WorkerMessage::Failed(error.to_string()));
                    return;
                }
            };
            cursor.clone_from(&page.next_cursor);
            if sender.send(WorkerMessage::Page(page)).is_err() {
                return;
            }
            if cursor.is_none() {
                let _ = sender.send(WorkerMessage::Complete);
                return;
            }
        }
    });
    receiver
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    receiver: mpsc::Receiver<WorkerMessage>,
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
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(frame.area());
    let status = match app.state() {
        LoadState::Loading => "loading sessions".to_owned(),
        LoadState::Ready if app.loading_more() => {
            format!("{} sessions · loading more", app.sessions().len())
        }
        LoadState::Ready => format!("{} sessions", app.sessions().len()),
        LoadState::Empty => "no interactive sessions found".to_owned(),
        LoadState::Failed(_) => "integration error".to_owned(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Codex Slice", Style::default().add_modifier(Modifier::BOLD)),
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

    frame.render_widget(
        Paragraph::new("↑/k ↓/j navigate  Home/End jump  q/Esc/Ctrl-C quit")
            .style(Style::default().fg(Color::DarkGray)),
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
    let age = format_age(session.recency_at, SystemTime::now());
    let project = session.project_label();
    let branch = session.branch.as_deref().unwrap_or("");
    let metadata = if width >= 70 && !branch.is_empty() {
        format!("{age:>4}  {project} [{branch}]  ")
    } else {
        format!("{age:>4}  {project}  ")
    };
    let available = width.saturating_sub(metadata.chars().count());
    let preview = truncate(&session.preview, available);
    Line::from(vec![
        Span::styled(metadata, Style::default().fg(Color::Cyan)),
        Span::raw(preview),
    ])
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_owned();
    }
    value.chars().take(width - 1).chain(['…']).collect()
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

    #[test]
    fn truncates_at_terminal_width() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("hello", 1), "…");
        assert_eq!(truncate("hello", 0), "");
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
}
