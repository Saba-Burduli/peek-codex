#![cfg(unix)]

use codex_slice::codex::{AppServerSource, CodexSessionSource};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process;

fn fake_server() -> (PathBuf, PathBuf, PathBuf) {
    let directory = std::env::temp_dir().join(format!("codex-slice-test-{}", process::id()));
    fs::create_dir_all(&directory).unwrap();
    let executable = directory.join("codex");
    let requests = directory.join("requests.log");
    let script = format!(
        r#"#!/bin/sh
read_line() {{
  IFS= read -r line || exit 1
  printf '%s\n' "$line" >> '{}'
}}
read_line
printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{"userAgent":"fake"}}}}'
read_line
read_line
printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"data":[{{"id":"first","name":null,"preview":"First","cwd":"/tmp/one","createdAt":1,"updatedAt":3,"recencyAt":3,"modelProvider":"openai","source":"cli","status":{{"type":"notLoaded"}},"gitInfo":null}}],"nextCursor":"next"}}}}'
read_line
printf '%s\n' '{{"jsonrpc":"2.0","id":3,"result":{{"data":[{{"id":"second","name":null,"preview":"Second","cwd":"/tmp/two","createdAt":1,"updatedAt":2,"recencyAt":2,"modelProvider":"openai","source":"vscode","status":{{"type":"idle"}},"gitInfo":null}}],"nextCursor":null}}}}'
read_line
printf '%s\n' '{{"jsonrpc":"2.0","id":4,"result":{{"thread":{{"id":"first","name":null,"preview":"First","cwd":"/tmp/one","createdAt":1,"updatedAt":3,"recencyAt":3,"modelProvider":"openai","source":"cli","status":{{"type":"notLoaded"}},"gitInfo":null}}}}}}'
"#,
        requests.display()
    );
    fs::write(&executable, script).unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).unwrap();
    (directory, executable, requests)
}

#[test]
fn handshakes_paginates_and_reads_through_stdio() {
    let (directory, executable, requests) = fake_server();
    let mut source = AppServerSource::spawn_program(&executable).unwrap();

    let first = source.list_sessions(None).unwrap();
    assert_eq!(first.sessions[0].id.as_str(), "first");
    assert_eq!(first.next_cursor.as_deref(), Some("next"));

    let second = source.list_sessions(first.next_cursor.as_deref()).unwrap();
    assert_eq!(second.sessions[0].id.as_str(), "second");
    assert!(second.next_cursor.is_none());

    let detail = source.read_thread(&first.sessions[0].id).unwrap();
    assert_eq!(detail.preview, "First");
    drop(source);

    let captured = fs::read_to_string(&requests).unwrap();
    assert!(captured.contains("\"method\":\"initialize\""));
    assert!(captured.contains("\"method\":\"initialized\""));
    assert!(captured.contains("\"useStateDbOnly\":true"));
    assert!(captured.contains("\"sortKey\":\"recency_at\""));
    assert!(captured.contains("\"cursor\":\"next\""));
    assert!(captured.contains("\"includeTurns\":false"));

    fs::remove_dir_all(directory).unwrap();
}
