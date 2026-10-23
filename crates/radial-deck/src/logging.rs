//! Stderr logging plus a rolling per-user log file for bug reports.

use std::io::{self, IsTerminal};
use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Keeps the non-blocking writer alive for the lifetime of the process.
pub type LogGuard = Option<WorkerGuard>;

pub fn init() -> LogGuard {
    let stderr = || {
        tracing_subscriber::fmt::layer()
            .with_writer(io::stderr)
            .with_target(false)
            .with_ansi(io::stderr().is_terminal())
    };
    let filter = || {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("radial_deck=info,warn"))
    };

    let Some((writer, guard)) = file_writer() else {
        let _ = tracing_subscriber::registry()
            .with(filter())
            .with(stderr())
            .try_init();
        return None;
    };

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_target(true)
        .with_ansi(false);
    let result = tracing_subscriber::registry()
        .with(filter())
        .with(stderr())
        .with(file_layer)
        .try_init();
    if result.is_err() {
        // The guard must not outlive the subscriber.
        drop(guard);
        return None;
    }
    Some(guard)
}

/// `$XDG_STATE_HOME/radial-deck/logs` with the XDG default fallback.
pub fn log_dir() -> Option<PathBuf> {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(state).join("radial-deck").join("logs"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state/radial-deck/logs"))
}

fn file_writer() -> Option<(tracing_appender::non_blocking::NonBlocking, WorkerGuard)> {
    let dir = log_dir()?;
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "radial-deck: cannot create log directory {}: {error}",
            dir.display()
        );
        return None;
    }
    let appender = match RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("radial-deck")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir)
    {
        Ok(appender) => appender,
        Err(error) => {
            eprintln!(
                "radial-deck: cannot open log file in {}: {error}",
                dir.display()
            );
            return None;
        }
    };
    Some(tracing_appender::non_blocking(appender))
}
