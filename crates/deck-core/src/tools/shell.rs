//! Process and path helpers shared by the advanced tools.
//!
//! Every external command is built as an argument vector. No shell is used,
//! so file names, annotation text and user parameters cannot become commands.

use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::binaries;
use crate::convert::registry::find_tool;

pub type ToolResult<T> = Result<T, String>;

pub fn require(tool: &str) -> ToolResult<PathBuf> {
    find_tool(tool).ok_or_else(|| format!("{tool} is not installed"))
}

pub fn require_any(tools: &[&str]) -> ToolResult<PathBuf> {
    tools
        .iter()
        .find_map(|tool| find_tool(tool))
        .ok_or_else(|| format!("{} is not installed", tools.join(" or ")))
}

/// Runs a command while forwarding its stderr and honouring cancellation.
///
/// The child is polled instead of blocking on `output()`, so the Cancel button
/// stops long FFmpeg/Ghostscript runs within about 50 ms. Stderr is read on a
/// separate thread (FFmpeg's statistics are chatty and can fill a pipe); the
/// last lines are kept for the error message.
pub fn run(command: &mut Command, label: &str) -> ToolResult<()> {
    command.stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("run {label}: {error}"))?;

    let collected = Arc::new(Mutex::new(Vec::<String>::new()));
    // The reader thread runs outside the job scope (thread-local), so it
    // cannot call the reporter itself; it hands lines back to this loop.
    let (line_tx, line_rx) = std::sync::mpsc::channel::<String>();
    let reader = child.stderr.take().map(|stderr| {
        let collected = Arc::clone(&collected);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buffer = [0u8; 4096];
            let mut pending = String::new();
            loop {
                let read = match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(_) => break,
                };
                pending.push_str(&String::from_utf8_lossy(&buffer[..read]));
                // FFmpeg separates statistics with carriage returns, so both
                // delimiters count as a finished line.
                while let Some(index) = pending.find(['\n', '\r']) {
                    let line: String = pending.drain(..=index).collect();
                    record_line(&collected, &line, &line_tx);
                }
            }
            record_line(&collected, &pending, &line_tx);
        })
    });

    let mut last_forward = Instant::now();
    loop {
        if crate::job::cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(reader) = reader {
                let _ = reader.join();
            }
            return Err(format!("{label} cancelled"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(reader) = reader {
                    let _ = reader.join();
                }
                drain_lines(&line_rx);
                if status.success() {
                    return Ok(());
                }
                let lines = collected.lock().unwrap();
                let tail: Vec<&str> = lines
                    .iter()
                    .rev()
                    .take(4)
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                return Err(format!(
                    "{label} failed: {}",
                    if tail.is_empty() {
                        format!("exit status {status}")
                    } else {
                        tail.join(" ")
                    }
                ));
            }
            Ok(None) => {
                forward_lines(&line_rx, &mut last_forward);
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("run {label}: {error}"));
            }
        }
    }
}

fn record_line(collected: &Mutex<Vec<String>>, line: &str, forward: &Sender<String>) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    let _ = forward.send(line.to_owned());
    let mut lines = collected.lock().unwrap();
    lines.push(line.to_owned());
    if lines.len() > 40 {
        lines.remove(0);
    }
}

/// Forwards the newest buffered line to the progress reporter at most every
/// 150 ms, so chatty FFmpeg statistics cannot flood the UI channel.
fn forward_lines(receiver: &Receiver<String>, last_forward: &mut Instant) {
    let Some(newest) = newest_line(receiver) else {
        return;
    };
    if last_forward.elapsed() >= Duration::from_millis(150) {
        *last_forward = Instant::now();
        crate::job::report_detail(&newest);
    }
}

/// Forwards the last remaining line unconditionally, after the child exited.
fn drain_lines(receiver: &Receiver<String>) {
    if let Some(newest) = newest_line(receiver) {
        crate::job::report_detail(&newest);
    }
}

fn newest_line(receiver: &Receiver<String>) -> Option<String> {
    let mut newest = None;
    while let Ok(line) = receiver.try_recv() {
        newest = Some(line);
    }
    newest
}

/// Runs a probing command (ffprobe, identify) and captures stdout.
///
/// Both pipes are drained on their own threads so a chatty child cannot fill
/// the pipe buffer and deadlock, and the wait is polled like `run` so Cancel
/// stops it and a wedged network mount cannot park the worker forever.
pub fn capture(command: &mut Command, label: &str) -> ToolResult<String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("run {label}: {error}"))?;
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_reader = read_all_pipe(stdout_pipe);
    let stderr_reader = read_all_pipe(stderr_pipe);
    loop {
        if crate::job::cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{label} cancelled"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_reader.join().unwrap_or_default();
                let stderr = stderr_reader.join().unwrap_or_default();
                if !status.success() {
                    let text = String::from_utf8_lossy(&stderr);
                    return Err(format!(
                        "{label} failed: {}",
                        text.lines().next().unwrap_or("unknown error")
                    ));
                }
                return Ok(String::from_utf8_lossy(&stdout).trim().to_owned());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("run {label}: {error}"));
            }
        }
    }
}

/// Drains one pipe on its own thread; both stdout and stderr must be drained
/// or the child blocks once a pipe buffer fills.
fn read_all_pipe<R: std::io::Read + Send + 'static>(
    pipe: Option<R>,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let Some(pipe) = pipe else {
            return Vec::new();
        };
        let mut reader = BufReader::new(pipe);
        let mut buffer = [0u8; 4096];
        let mut collected = Vec::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => collected.extend_from_slice(&buffer[..read]),
            }
        }
        collected
    })
}

/// Output directory for a source under the active operation's policy.
pub fn source_parent(source: &Path) -> PathBuf {
    crate::job::destination_dir(source)
}

pub fn source_stem(source: &Path) -> String {
    source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into())
}

pub fn extension(source: &Path) -> String {
    source
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn sanitize(stem: &str) -> String {
    crate::convert::sanitize(stem)
}

/// Collision-free file path. Existing files are never overwritten. Delegates
/// to the convert engine's implementation so both call sites share one
/// naming/extension policy.
pub fn unique_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    crate::convert::unique_path(dir, stem, extension)
}

/// Collision-free directory path, created by the caller.
pub fn unique_dir(parent: &Path, stem: &str) -> PathBuf {
    let stem = sanitize(stem);
    let mut candidate = parent.join(&stem);
    let mut counter = 1;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}-{counter}"));
        counter += 1;
    }
    crate::job::register(&candidate);
    candidate
}

pub fn create_dir(path: &Path) -> ToolResult<()> {
    std::fs::create_dir_all(path).map_err(|error| format!("create {}: {error}", path.display()))
}

pub fn parse_seconds(raw: &str) -> ToolResult<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("time cannot be empty".into());
    }
    let parts: Vec<&str> = raw.split(':').collect();
    let value = match parts.as_slice() {
        [seconds] => seconds.parse::<f64>().map_err(|_| ()),
        [minutes, seconds] => minutes
            .parse::<f64>()
            .and_then(|minutes| {
                seconds
                    .parse::<f64>()
                    .map(|seconds| minutes * 60.0 + seconds)
            })
            .map_err(|_| ()),
        [hours, minutes, seconds] => hours
            .parse::<f64>()
            .and_then(|hours| {
                minutes.parse::<f64>().and_then(|minutes| {
                    seconds
                        .parse::<f64>()
                        .map(|seconds| hours * 3600.0 + minutes * 60.0 + seconds)
                })
            })
            .map_err(|_| ()),
        _ => Err(()),
    }
    .map_err(|_| format!("invalid time: {raw}"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!("invalid time: {raw}"));
    }
    Ok(value)
}

pub fn format_seconds(seconds: f64) -> String {
    format!("{seconds:.3}")
}

pub fn parse_u32(raw: &str, label: &str) -> ToolResult<u32> {
    raw.trim()
        .parse::<u32>()
        .map_err(|_| format!("{label} must be a whole number"))
}

pub fn ffprobe_dimensions(source: &Path) -> ToolResult<(u32, u32)> {
    let output = capture(
        Command::new(require(binaries::FFPROBE)?)
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=width,height")
            .arg("-of")
            .arg("csv=s=x:p=0")
            .arg(source),
        binaries::FFPROBE,
    )?;
    let (width, height) = output
        .split_once('x')
        .ok_or_else(|| "could not read video dimensions".to_owned())?;
    Ok((
        width
            .trim()
            .parse()
            .map_err(|_| "invalid video width".to_owned())?,
        height
            .trim()
            .parse()
            .map_err(|_| "invalid video height".to_owned())?,
    ))
}

pub fn image_dimensions(source: &Path) -> ToolResult<(u32, u32)> {
    let mut command = if let Some(magick) = find_tool(binaries::IMAGEMAGICK) {
        let mut command = Command::new(magick);
        command.arg("identify");
        command
    } else {
        Command::new(require("identify")?)
    };
    let output = capture(
        command.arg("-format").arg("%w %h").arg(source),
        "ImageMagick identify",
    )?;
    let mut parts = output.split_whitespace();
    let width = parts
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "could not read image width".to_owned())?;
    let height = parts
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "could not read image height".to_owned())?;
    Ok((width, height))
}

pub fn has_audio(source: &Path) -> bool {
    let Ok(ffprobe) = require(binaries::FFPROBE) else {
        return true;
    };
    Command::new(ffprobe)
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("a:0")
        .arg("-show_entries")
        .arg("stream=index")
        .arg("-of")
        .arg("csv=p=0")
        .arg(source)
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

/// Escapes one path for ffmpeg's concat demuxer list format.
pub fn concat_line(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\'', "'\\''");
    format!("file '{text}'\n")
}

pub fn magick() -> ToolResult<PathBuf> {
    require_any(&[binaries::IMAGEMAGICK, binaries::CONVERT])
}

pub fn magick_command() -> ToolResult<Command> {
    let binary = magick()?;
    Ok(Command::new(binary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{self, OutputLocation, Progress};
    use std::sync::Arc;

    /// The stderr reader runs outside the job scope, so `run` must hand its
    /// lines back to the polling thread before they reach the reporter.
    #[test]
    fn run_forwards_stderr_to_the_reporter() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::<Progress>::new()));
        let sink = seen.clone();
        job::scope(OutputLocation::Alongside, None, || {
            job::set_reporter(Arc::new(move |progress| {
                sink.lock().unwrap().push(progress);
            }));
            let mut command = Command::new("dd");
            command.args(["if=/dev/null", "of=/dev/null", "count=1"]);
            run(&mut command, "stderr test").unwrap();
        });
        let seen = seen.lock().unwrap();
        assert!(
            seen.iter().any(|progress| progress.detail.is_some()),
            "expected stderr detail to reach the reporter, got {seen:?}"
        );
    }
}
