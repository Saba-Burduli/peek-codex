use serde_json::json;
use std::env;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const HELP: &str = "Peek Codex — browse local Codex sessions\n\nUsage: peek-codex [OPTIONS]\n\nOptions:\n      --log-file <PATH>  Append structured diagnostics to PATH\n  -h, --help             Print help\n  -V, --version          Print version";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("peek-codex: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let options = parse_args(env::args_os().skip(1))?;
    match options.action {
        Action::Help => {
            println!("{HELP}");
            return Ok(());
        }
        Action::Version => {
            println!("peek-codex {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Action::Run => {}
    }

    let mut diagnostics = options
        .log_file
        .map(Diagnostics::open)
        .transpose()
        .map_err(|error| format!("could not open log file: {error}"))?;
    if let Some(log) = diagnostics.as_mut() {
        log.event("start")?;
    }
    let result = peek_codex::tui::run();
    if let Some(log) = diagnostics.as_mut() {
        let event = if result.is_ok() { "stop" } else { "failure" };
        log.event(event)?;
    }
    result
}

#[derive(Debug, Eq, PartialEq)]
enum Action {
    Run,
    Help,
    Version,
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    action: Action,
    log_file: Option<PathBuf>,
}

fn parse_args(arguments: impl Iterator<Item = OsString>) -> Result<Options, String> {
    let mut action = Action::Run;
    let mut log_file = None;
    let mut arguments = arguments.peekable();

    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("-h" | "--help") => action = Action::Help,
            Some("-V" | "--version") => action = Action::Version,
            Some("--log-file") => {
                let path = arguments.next().ok_or_else(|| {
                    "--log-file requires a path; run `peek-codex --help`".to_owned()
                })?;
                if log_file.replace(PathBuf::from(path)).is_some() {
                    return Err("--log-file may only be provided once".to_owned());
                }
            }
            Some(value) => {
                return Err(format!("unknown option `{value}`; run `peek-codex --help`"));
            }
            None => return Err("arguments must be valid UTF-8".to_owned()),
        }
    }

    Ok(Options { action, log_file })
}

struct Diagnostics {
    file: File,
}

impl Diagnostics {
    fn open(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file })
    }

    fn event(&mut self, event: &str) -> Result<(), String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        writeln!(
            self.file,
            "{}",
            json!({"timestamp": timestamp, "event": event})
        )
        .map_err(|error| format!("could not write diagnostics: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_public_cli() {
        let options = parse_args(
            ["--log-file", "/tmp/slice.log"]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert_eq!(options.action, Action::Run);
        assert_eq!(options.log_file, Some(PathBuf::from("/tmp/slice.log")));
    }

    #[test]
    fn rejects_unknown_options() {
        let error = parse_args(["--resume"].into_iter().map(OsString::from)).unwrap_err();
        assert!(error.contains("unknown option"));
    }
}
