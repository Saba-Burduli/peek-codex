use crate::domain::{Session, SessionId, SessionPage, sanitize_terminal_text};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fmt;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PAGE_SIZE: u32 = 50;
pub(crate) const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const RESPONSE_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub trait CodexSessionSource {
    fn list_sessions(&mut self, cursor: Option<&str>) -> Result<SessionPage, SourceError>;
    fn read_thread(&mut self, id: &SessionId) -> Result<Session, SourceError>;
}

#[derive(Debug)]
pub struct SourceError {
    message: String,
}

impl SourceError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SourceError {}

pub struct AppServerSource {
    child: Child,
    writer: BufWriter<ChildStdin>,
    responses: Receiver<Result<String, String>>,
    reader_thread: Option<JoinHandle<()>>,
    next_id: u64,
    request_timeout: Duration,
    cancelled: Arc<AtomicBool>,
}

impl AppServerSource {
    pub fn spawn() -> Result<Self, SourceError> {
        Self::spawn_program_with_control(
            Path::new("codex"),
            DEFAULT_REQUEST_TIMEOUT,
            Arc::new(AtomicBool::new(false)),
        )
    }

    pub fn spawn_program(program: &Path) -> Result<Self, SourceError> {
        Self::spawn_program_with_timeout(program, DEFAULT_REQUEST_TIMEOUT)
    }

    fn spawn_program_with_timeout(
        program: &Path,
        request_timeout: Duration,
    ) -> Result<Self, SourceError> {
        Self::spawn_program_with_control(program, request_timeout, Arc::new(AtomicBool::new(false)))
    }

    pub(crate) fn spawn_program_with_control(
        program: &Path,
        request_timeout: Duration,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, SourceError> {
        let mut command = Command::new(program);
        command
            .arg("app-server")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = command.spawn().map_err(|error| {
            SourceError::new(format!(
                "could not start `codex app-server`: {error}; install Codex CLI 0.146.0 or newer and ensure `codex` is on PATH"
            ))
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| SourceError::new("app-server stdin was unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SourceError::new("app-server stdout was unavailable"))?;
        let (response_sender, responses) = mpsc::channel();
        let reader_thread = thread::spawn(move || read_responses(stdout, response_sender));

        let mut source = Self {
            child,
            writer: BufWriter::new(stdin),
            responses,
            reader_thread: Some(reader_thread),
            next_id: 1,
            request_timeout,
            cancelled,
        };
        source.initialize()?;
        Ok(source)
    }

    fn initialize(&mut self) -> Result<(), SourceError> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "peek-codex",
                    "title": "Peek Codex",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )?;
        self.notify("initialized", None)
    }

    fn notify(&mut self, method: &str, params: Option<Value>) -> Result<(), SourceError> {
        let mut notification = json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if let Some(params) = params {
            notification["params"] = params;
        }
        self.write_message(&notification)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, SourceError> {
        let id = self.next_id;
        self.next_id += 1;
        self.write_message(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        let deadline = Instant::now() + self.request_timeout;

        loop {
            if self.cancelled.load(Ordering::Relaxed) {
                return Err(SourceError::new("app-server request cancelled"));
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(SourceError::new(format!(
                    "app-server `{method}` request timed out after {} seconds",
                    self.request_timeout.as_secs_f32()
                )));
            };
            match self
                .responses
                .recv_timeout(remaining.min(RESPONSE_POLL_INTERVAL))
            {
                Ok(Ok(line)) => {
                    let response: Value = serde_json::from_str(&line).map_err(|error| {
                        SourceError::new(format!("app-server returned invalid JSON: {error}"))
                    })?;
                    if let Some(result) = decode_response(response, id)? {
                        return Ok(result);
                    }
                }
                Ok(Err(message)) => return Err(SourceError::new(message)),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(SourceError::new(
                        "app-server response channel closed unexpectedly",
                    ));
                }
            }
        }
    }

    fn write_message(&mut self, message: &Value) -> Result<(), SourceError> {
        serde_json::to_writer(&mut self.writer, message).map_err(|error| {
            SourceError::new(format!("could not encode app-server request: {error}"))
        })?;
        self.writer
            .write_all(b"\n")
            .and_then(|()| self.writer.flush())
            .map_err(|error| {
                SourceError::new(format!("could not write app-server request: {error}"))
            })
    }
}

fn decode_response(response: Value, expected_id: u64) -> Result<Option<Value>, SourceError> {
    if response.get("id").and_then(Value::as_u64) != Some(expected_id) {
        return Ok(None);
    }
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| SourceError::new("app-server returned a malformed JSON-RPC error"))?;
        return Err(SourceError::new(format!(
            "app-server request failed: {message}"
        )));
    }
    response
        .get("result")
        .cloned()
        .map(Some)
        .ok_or_else(|| SourceError::new("app-server response did not contain a result"))
}

impl CodexSessionSource for AppServerSource {
    fn list_sessions(&mut self, cursor: Option<&str>) -> Result<SessionPage, SourceError> {
        let result = self.request(
            "thread/list",
            json!({
                "cursor": cursor,
                "limit": PAGE_SIZE,
                "sortKey": "recency_at",
                "sortDirection": "desc",
                "useStateDbOnly": true,
            }),
        )?;
        decode_thread_list(result)
    }

    fn read_thread(&mut self, id: &SessionId) -> Result<Session, SourceError> {
        let result = self.request(
            "thread/read",
            json!({
                "threadId": id.as_str(),
                "includeTurns": false,
            }),
        )?;
        let response: ThreadReadResponse = serde_json::from_value(result).map_err(|error| {
            SourceError::new(format!("could not decode thread/read response: {error}"))
        })?;
        response.thread.try_into()
    }
}

impl Drop for AppServerSource {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }
    }
}

fn read_responses(stdout: ChildStdout, sender: mpsc::Sender<Result<String, String>>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                let _ = sender.send(Err(
                    "app-server exited before replying; Codex CLI 0.146.0 or newer is required"
                        .to_owned(),
                ));
                return;
            }
            Ok(_) => {
                if sender.send(Ok(line)).is_err() {
                    return;
                }
            }
            Err(error) => {
                let _ = sender.send(Err(format!("could not read app-server response: {error}")));
                return;
            }
        }
    }
}

fn decode_thread_list(value: Value) -> Result<SessionPage, SourceError> {
    let response: ThreadListResponse = serde_json::from_value(value).map_err(|error| {
        SourceError::new(format!("could not decode thread/list response: {error}"))
    })?;
    let sessions = response
        .data
        .into_iter()
        .map(Session::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let mut page = SessionPage {
        sessions,
        next_cursor: response.next_cursor,
    };
    page.sort_by_recency();
    Ok(page)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadListResponse {
    data: Vec<ProtocolThread>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct ThreadReadResponse {
    thread: ProtocolThread,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolThread {
    id: String,
    name: Option<String>,
    preview: String,
    cwd: String,
    created_at: i64,
    updated_at: i64,
    recency_at: Option<i64>,
    model_provider: String,
    source: Value,
    status: ProtocolStatus,
    git_info: Option<ProtocolGitInfo>,
}

#[derive(Deserialize)]
struct ProtocolStatus {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct ProtocolGitInfo {
    branch: Option<String>,
}

impl TryFrom<ProtocolThread> for Session {
    type Error = SourceError;

    fn try_from(thread: ProtocolThread) -> Result<Self, Self::Error> {
        let provider = sanitize_terminal_text(&thread.model_provider);
        let source = display_source(&thread.source);
        Ok(Self {
            id: SessionId::new(thread.id)
                .map_err(|error| SourceError::new(format!("invalid thread id: {error}")))?,
            name: thread
                .name
                .map(|value| sanitize_terminal_text(&value))
                .filter(|value| !value.is_empty()),
            preview: sanitize_terminal_text(&thread.preview),
            cwd: sanitize_terminal_text(&thread.cwd),
            created_at: thread.created_at,
            updated_at: thread.updated_at,
            recency_at: thread.recency_at.unwrap_or(thread.updated_at),
            provider: if source.is_empty() {
                provider
            } else {
                format!("{provider}/{source}")
            },
            status: sanitize_terminal_text(&thread.status.kind),
            branch: thread
                .git_info
                .and_then(|info| info.branch)
                .map(|value| sanitize_terminal_text(&value))
                .filter(|value| !value.is_empty()),
        })
    }
}

fn display_source(source: &Value) -> String {
    match source {
        Value::String(value) => sanitize_terminal_text(value),
        Value::Object(value) if value.contains_key("custom") => value
            .get("custom")
            .and_then(Value::as_str)
            .map(sanitize_terminal_text)
            .unwrap_or_default(),
        Value::Object(value) if value.contains_key("subAgent") => "subAgent".to_owned(),
        _ => "unknown".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::time::Instant;

    #[test]
    fn decodes_redacted_fixture_without_leaking_nested_content() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/thread-list-redacted.json"))
                .unwrap();

        let page = decode_thread_list(fixture).unwrap();

        assert_eq!(page.sessions.len(), 2);
        assert_eq!(page.sessions[0].id.as_str(), "019-first");
        assert_eq!(page.sessions[0].preview, "Fix the release build");
        assert_eq!(page.sessions[0].branch.as_deref(), Some("feature safe"));
        assert_eq!(page.sessions[0].provider, "openai/cli");
        assert_eq!(page.sessions[1].recency_at, 100);
        let debug = format!("{:?}", page.sessions);
        assert!(!debug.contains("secret-command"));
        assert!(!debug.contains("private-file-change"));
    }

    #[test]
    fn rejects_malformed_rpc_error() {
        let error: Value =
            serde_json::from_str(include_str!("../tests/fixtures/malformed-rpc-error.json"))
                .unwrap();
        let error = decode_response(error, 3).unwrap_err();
        assert_eq!(
            error.to_string(),
            "app-server returned a malformed JSON-RPC error"
        );
    }

    #[test]
    fn displays_unknown_source_without_failing() {
        assert_eq!(display_source(&json!({"future": true})), "unknown");
    }

    #[test]
    fn production_request_timeout_is_ten_seconds() {
        assert_eq!(DEFAULT_REQUEST_TIMEOUT, Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn configured_request_timeout_is_enforced() {
        let directory =
            std::env::temp_dir().join(format!("peek-codex-timeout-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("codex");
        fs::write(
            &executable,
            "#!/bin/sh\nIFS= read -r line || exit 1\nwhile IFS= read -r ignored; do :; done\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        let started = Instant::now();

        let error =
            AppServerSource::spawn_program_with_timeout(&executable, Duration::from_millis(100))
                .err()
                .expect("silent server should time out");
        let elapsed = started.elapsed();

        assert!(error.to_string().contains("initialize` request timed out"));
        assert!(elapsed >= Duration::from_millis(80));
        assert!(elapsed < Duration::from_secs(1));
        fs::remove_dir_all(directory).unwrap();
    }
}
