//! Process and path helpers shared by the advanced tools.
//!
//! Every external command is built as an argument vector. No shell is used,
//! so file names, annotation text and user parameters cannot become commands.

use std::path::{Path, PathBuf};
use std::process::Command;

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

pub fn run(command: &mut Command, label: &str) -> ToolResult<()> {
    let output = command
        .output()
        .map_err(|error| format!("run {label}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Err(format!(
        "{label} failed: {}",
        if tail.is_empty() {
            format!("exit status {}", output.status)
        } else {
            tail.join(" ")
        }
    ))
}

pub fn capture(command: &mut Command, label: &str) -> ToolResult<String> {
    let output = command
        .output()
        .map_err(|error| format!("run {label}: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{label} failed: {}",
            stderr.lines().next().unwrap_or("unknown error")
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn source_parent(source: &Path) -> PathBuf {
    source
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
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
    let cleaned: String = stem
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '\0' => '_',
            _ => ch,
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "output".into()
    } else {
        trimmed.to_owned()
    }
}

/// Collision-free file path. Existing files are never overwritten.
pub fn unique_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let stem = sanitize(stem);
    let extension = extension.trim_start_matches('.');
    if extension.is_empty() {
        let mut candidate = dir.join(&stem);
        let mut counter = 1;
        while candidate.exists() {
            candidate = dir.join(format!("{stem}-{counter}"));
            counter += 1;
        }
        return candidate;
    }
    let mut candidate = dir.join(format!("{stem}.{extension}"));
    let mut counter = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem}-{counter}.{extension}"));
        counter += 1;
    }
    candidate
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
        Command::new(require("ffprobe")?)
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=width,height")
            .arg("-of")
            .arg("csv=s=x:p=0")
            .arg(source),
        "ffprobe",
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
    let mut command = if let Some(magick) = find_tool("magick") {
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
    let Ok(ffprobe) = require("ffprobe") else {
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
    require_any(&["magick", "convert"])
}

pub fn magick_command() -> ToolResult<Command> {
    let binary = magick()?;
    Ok(Command::new(binary))
}
