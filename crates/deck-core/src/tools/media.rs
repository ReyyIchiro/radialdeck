//! Audio and video tools built on ffmpeg.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::shell::{
    concat_line, create_dir, extension, ffprobe_dimensions, format_seconds, has_audio,
    parse_seconds, parse_u32, require, run, source_parent, source_stem, unique_dir, unique_path,
    ToolResult,
};
use super::{quality, seconds, value, ToolFailure, ToolId, ToolOutcome};
use crate::Category;

pub fn execute(
    id: ToolId,
    category: Category,
    sources: &[PathBuf],
    values: &BTreeMap<String, String>,
) -> ToolOutcome {
    match id {
        ToolId::JoinVideos => join(sources),
        ToolId::Compress => each(sources, |source| match category {
            Category::Video => compress_video(source, values),
            Category::Audio => compress_audio(source, values),
            _ => Err("compress is not an image or PDF tool here".into()),
        }),
        ToolId::RemoveMetadata => each(sources, remove_metadata),
        ToolId::MuteVideo => each(sources, mute),
        ToolId::TrimVideo => each(sources, |source| trim_video(source, values)),
        ToolId::SplitVideo => each(sources, |source| split_video(source, values)),
        ToolId::CropVideo => each(sources, |source| crop_video(source, values)),
        ToolId::ChangeVideoSpeed => each(sources, |source| speed(source, values)),
        ToolId::VideoSnapshots => each(sources, |source| snapshots(source, values)),
        ToolId::RedactVideo => each(sources, |source| redact_video(source, values)),
        ToolId::NormalizeAudio => each(sources, |source| normalize(source, values)),
        ToolId::TrimAudio => each(sources, |source| trim_audio(source, values)),
        ToolId::AudioChannels => each(sources, |source| channels(source, values)),
        ToolId::AudioToVideo => each(sources, |source| visualize(source, values)),
        ToolId::RedactAudio => each(sources, |source| redact_audio(source, values)),
        _ => ToolOutcome::failure(
            sources
                .first()
                .map(PathBuf::as_path)
                .unwrap_or(Path::new("")),
            "unsupported audio/video tool".to_owned(),
        ),
    }
}

fn each<F>(sources: &[PathBuf], mut operation: F) -> ToolOutcome
where
    F: FnMut(&Path) -> ToolResult<Vec<PathBuf>>,
{
    let mut outcome = ToolOutcome::default();
    crate::job::set_total(sources.len());
    for source in sources {
        if crate::job::cancelled() {
            outcome.failures.push(ToolFailure {
                source: source.clone(),
                message: "Cancelled".into(),
            });
            break;
        }
        match operation(source) {
            Ok(created) => outcome.created.extend(created),
            Err(message) => outcome.failures.push(ToolFailure {
                source: source.clone(),
                message,
            }),
        }
        crate::job::report_file(source);
    }
    outcome
}

fn ffmpeg() -> ToolResult<Command> {
    let mut command = Command::new(require("ffmpeg")?);
    command
        .arg("-nostdin")
        .arg("-y")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error");
    Ok(command)
}

fn finish(command: &mut Command, label: &str, destination: &Path) -> ToolResult<()> {
    match run(command, label) {
        Ok(()) => Ok(()),
        Err(message) => {
            let _ = fs::remove_file(destination);
            Err(message)
        }
    }
}

fn join(sources: &[PathBuf]) -> ToolOutcome {
    let first = &sources[0];
    if sources.len() < 2 {
        return ToolOutcome::failure(first, "select at least two videos".to_owned());
    }
    let temp = match tempfile::tempdir() {
        Ok(temp) => temp,
        Err(error) => return ToolOutcome::failure(first, format!("create temp dir: {error}")),
    };
    let list = temp.path().join("inputs.txt");
    let mut text = String::new();
    for source in sources {
        // ffmpeg resolves concat entries relative to the list file, not the
        // working directory, so the demuxer needs absolute paths.
        let absolute = fs::canonicalize(source).unwrap_or_else(|_| source.clone());
        text.push_str(&concat_line(&absolute));
    }
    if let Err(error) = fs::write(&list, text) {
        return ToolOutcome::failure(first, format!("write concat list: {error}"));
    }
    let destination = unique_path(&source_parent(first), "joined", "mp4");
    let mut command = match ffmpeg() {
        Ok(command) => command,
        Err(message) => return ToolOutcome::failure(first, message),
    };
    command
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&list)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("23")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("160k")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    match finish(&mut command, "ffmpeg join", &destination) {
        Ok(()) => ToolOutcome::success(vec![destination]),
        Err(message) => ToolOutcome::failure(
            first,
            format!("{message} (join works best when the clips share a codec and size)"),
        ),
    }
}

fn compress_video(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let crf = quality(values, "crf", 28)?;
    if !(16..=40).contains(&crf) {
        return Err("CRF must be between 16 and 40".into());
    }
    let preset = value(values, "preset", "medium");
    if !matches!(preset, "fast" | "medium" | "slow") {
        return Err("preset must be fast, medium, or slow".into());
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-compressed", source_stem(source)),
        "mp4",
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg(preset)
        .arg("-crf")
        .arg(crf.to_string())
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("128k")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    finish(&mut command, "ffmpeg compress", &destination)?;
    Ok(vec![destination])
}

fn compress_audio(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let bitrate = parse_u32(value(values, "bitrate", "128"), "bitrate")?;
    if !(32..=320).contains(&bitrate) {
        return Err("bitrate must be between 32 and 320 kbps".into());
    }
    let source_format = extension(source);
    let (output_format, compressed) = if matches!(source_format.as_str(), "wav" | "aiff") {
        (source_format, false)
    } else {
        (source_format, true)
    };
    // WAV and AIFF are uncompressed; FLAC gives a real size reduction while
    // still being lossless.
    let output_format = if !compressed {
        "flac".to_owned()
    } else {
        output_format
    };
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-compressed", source_stem(source)),
        &output_format,
    );
    let mut command = ffmpeg()?;
    command.arg("-i").arg(source);
    audio_codec(&mut command, &output_format, bitrate);
    command.arg(&destination);
    finish(&mut command, "ffmpeg audio compress", &destination)?;
    Ok(vec![destination])
}

fn remove_metadata(source: &Path) -> ToolResult<Vec<PathBuf>> {
    let format = extension(source);
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-clean", source_stem(source)),
        &format,
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0")
        .arg("-map_metadata")
        .arg("-1")
        .arg("-c")
        .arg("copy")
        .arg(&destination);
    finish(&mut command, "ffmpeg metadata", &destination)?;
    Ok(vec![destination])
}

fn mute(source: &Path) -> ToolResult<Vec<PathBuf>> {
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-muted", source_stem(source)),
        "mp4",
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-an")
        .arg("-c:v")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    if finish(&mut command, "ffmpeg mute", &destination).is_ok() {
        return Ok(vec![destination]);
    }
    let mut fallback = ffmpeg()?;
    fallback
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-an")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    finish(&mut fallback, "ffmpeg mute", &destination)?;
    Ok(vec![destination])
}

fn trim_video(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let start = seconds(values, "start", 0.0)?;
    let end = optional_time(values, "end")?;
    if let Some(end) = end {
        if end <= start {
            return Err("end must be after start".into());
        }
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-trimmed", source_stem(source)),
        "mp4",
    );
    let mut command = ffmpeg()?;
    command
        .arg("-ss")
        .arg(format_seconds(start))
        .arg("-i")
        .arg(source);
    if let Some(end) = end {
        command.arg("-t").arg(format_seconds(end - start));
    }
    command
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    finish(&mut command, "ffmpeg trim", &destination)?;
    Ok(vec![destination])
}

fn split_video(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let raw = value(values, "at", "");
    if raw.is_empty() {
        return Err("enter at least one split time".into());
    }
    let mut times = Vec::new();
    for piece in raw.split(',') {
        if piece.trim().is_empty() {
            continue;
        }
        times.push(parse_seconds(piece)?);
    }
    if times.is_empty() {
        return Err("enter at least one split time".into());
    }
    times.sort_by(|a, b| a.total_cmp(b));
    times.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
    let times: Vec<String> = times.iter().map(|time| format_seconds(*time)).collect();
    let directory = unique_dir(
        &source_parent(source),
        &format!("{}-parts", source_stem(source)),
    );
    create_dir(&directory)?;
    let pattern = directory.join("part-%03d.mp4");
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-c:a")
        .arg("aac")
        .arg("-force_key_frames")
        .arg(times.join(","))
        .arg("-f")
        .arg("segment")
        .arg("-segment_times")
        .arg(times.join(","))
        .arg("-reset_timestamps")
        .arg("1")
        .arg(&pattern);
    finish(&mut command, "ffmpeg split", &pattern)?;
    list_files(&directory)
}

fn crop_video(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let (source_width, source_height) = ffprobe_dimensions(source)?;
    let width = dimension(value(values, "width", "80%"), source_width, "width")?;
    let height = dimension(value(values, "height", "80%"), source_height, "height")?;
    let x = offset(value(values, "x", "10%"), source_width, width, "x")?;
    let y = offset(value(values, "y", "10%"), source_height, height, "y")?;
    let (width, height, x, y) = (even(width), even(height), even(x), even(y));
    if width == 0 || height == 0 || x + width > source_width || y + height > source_height {
        return Err(format!(
            "crop rectangle is outside the video ({source_width}×{source_height})"
        ));
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-cropped", source_stem(source)),
        "mp4",
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-vf")
        .arg(format!("crop={width}:{height}:{x}:{y}"))
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    finish(&mut command, "ffmpeg crop", &destination)?;
    Ok(vec![destination])
}

fn speed(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let factor = value(values, "factor", "2")
        .parse::<f64>()
        .map_err(|_| "speed must be a number".to_owned())?;
    if !factor.is_finite() || !(0.25..=4.0).contains(&factor) {
        return Err("speed must be between 0.25 and 4".into());
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-{}x", source_stem(source), trim_number(factor)),
        "mp4",
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-vf")
        .arg(format!("setpts=PTS/{factor}"));
    if has_audio(source) {
        command.arg("-af").arg(atempo_chain(factor));
    } else {
        command.arg("-an");
    }
    command
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    finish(&mut command, "ffmpeg speed", &destination)?;
    Ok(vec![destination])
}

fn snapshots(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let interval = value(values, "interval", "5")
        .parse::<f64>()
        .map_err(|_| "interval must be a number".to_owned())?;
    if !interval.is_finite() || !(0.1..=3600.0).contains(&interval) {
        return Err("interval must be between 0.1 and 3600 seconds".into());
    }
    let format = value(values, "format", "jpg");
    if !matches!(format, "jpg" | "png") {
        return Err("snapshot format must be jpg or png".into());
    }
    let directory = unique_dir(
        &source_parent(source),
        &format!("{}-frames", source_stem(source)),
    );
    create_dir(&directory)?;
    let pattern = directory.join(format!("frame-%03d.{format}"));
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-vf")
        .arg(format!("fps=1/{interval}"))
        .arg("-q:v")
        .arg("2")
        .arg(&pattern);
    finish(&mut command, "ffmpeg snapshots", &pattern)?;
    let files = list_files(&directory)?;
    if files.is_empty() {
        return Err("no frames were produced".into());
    }
    Ok(files)
}

fn redact_video(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let (source_width, source_height) = ffprobe_dimensions(source)?;
    let width = dimension(value(values, "width", "40%"), source_width, "width")?;
    let height = dimension(value(values, "height", "30%"), source_height, "height")?;
    let x = offset(value(values, "x", "30%"), source_width, width, "x")?;
    let y = offset(value(values, "y", "35%"), source_height, height, "y")?;
    let (width, height, x, y) = (even(width), even(height), even(x), even(y));
    if width == 0 || height == 0 || x + width > source_width || y + height > source_height {
        return Err(format!(
            "redaction rectangle is outside the video ({source_width}×{source_height})"
        ));
    }
    let from = seconds(values, "from", 0.0)?;
    let to = optional_time(values, "to")?;
    if let Some(to) = to {
        if to <= from {
            return Err("end must be after start".into());
        }
    }
    let mut filter = format!("drawbox=x={x}:y={y}:w={width}:h={height}:color=black@1:t=fill");
    if let Some(to) = to {
        filter.push_str(&format!(":enable='between(t,{from},{to})'"));
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-redacted", source_stem(source)),
        "mp4",
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-vf")
        .arg(filter)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-c:a")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&destination);
    finish(&mut command, "ffmpeg redact", &destination)?;
    Ok(vec![destination])
}

fn normalize(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let loudness = value(values, "loudness", "-16")
        .parse::<i32>()
        .map_err(|_| "target loudness must be a whole number".to_owned())?;
    if !(-30..=-5).contains(&loudness) {
        return Err("target loudness must be between -30 and -5 LUFS".into());
    }
    let format = extension(source);
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-normalized", source_stem(source)),
        &format,
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-af")
        .arg(format!("loudnorm=I={loudness}:TP=-1.5:LRA=11"));
    audio_codec(&mut command, &format, 192);
    command.arg(&destination);
    finish(&mut command, "ffmpeg normalize", &destination)?;
    Ok(vec![destination])
}

fn trim_audio(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let start = seconds(values, "start", 0.0)?;
    let end = optional_time(values, "end")?;
    if let Some(end) = end {
        if end <= start {
            return Err("end must be after start".into());
        }
    }
    let format = extension(source);
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-trimmed", source_stem(source)),
        &format,
    );
    let mut command = ffmpeg()?;
    command
        .arg("-ss")
        .arg(format_seconds(start))
        .arg("-i")
        .arg(source);
    if let Some(end) = end {
        command.arg("-t").arg(format_seconds(end - start));
    }
    audio_codec(&mut command, &format, 192);
    command.arg(&destination);
    finish(&mut command, "ffmpeg audio trim", &destination)?;
    Ok(vec![destination])
}

fn channels(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let choice = value(values, "channels", "mono");
    let (channels, suffix) = match choice {
        "mono" => ("1", "mono"),
        "stereo" => ("2", "stereo"),
        _ => return Err("channels must be mono or stereo".into()),
    };
    let format = extension(source);
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-{suffix}", source_stem(source)),
        &format,
    );
    let mut command = ffmpeg()?;
    command.arg("-i").arg(source).arg("-ac").arg(channels);
    audio_codec(&mut command, &format, 192);
    command.arg(&destination);
    finish(&mut command, "ffmpeg channels", &destination)?;
    Ok(vec![destination])
}

fn visualize(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let mode = value(values, "mode", "waveform");
    let size = value(values, "size", "1280x720");
    if !matches!(size, "1280x720" | "1920x1080") {
        return Err("resolution must be 1280x720 or 1920x1080".into());
    }
    let filter = match mode {
        "waveform" => format!("[0:a]showwaves=s={size}:mode=line:colors=white[v]"),
        "spectrum" => format!("[0:a]showspectrum=s={size}:mode=combined:color=intensity[v]"),
        _ => return Err("visualizer must be waveform or spectrum".into()),
    };
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-visualizer", source_stem(source)),
        "mp4",
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-filter_complex")
        .arg(filter)
        .arg("-map")
        .arg("[v]")
        .arg("-map")
        .arg("0:a")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-shortest")
        .arg(&destination);
    finish(&mut command, "ffmpeg visualizer", &destination)?;
    Ok(vec![destination])
}

fn redact_audio(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let from = seconds(values, "from", 0.0)?;
    let to = seconds(values, "to", 5.0)?;
    if to <= from {
        return Err("end must be after start".into());
    }
    let format = extension(source);
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-bleeped", source_stem(source)),
        &format,
    );
    let mut command = ffmpeg()?;
    command
        .arg("-i")
        .arg(source)
        .arg("-af")
        .arg(format!("volume=enable='between(t,{from},{to})':volume=0"));
    audio_codec(&mut command, &format, 192);
    command.arg(&destination);
    finish(&mut command, "ffmpeg bleep", &destination)?;
    Ok(vec![destination])
}

fn optional_time(values: &BTreeMap<String, String>, key: &str) -> ToolResult<Option<f64>> {
    let raw = value(values, key, "");
    if raw.is_empty() {
        Ok(None)
    } else {
        parse_seconds(raw).map(Some)
    }
}

fn audio_codec(command: &mut Command, format: &str, bitrate: u32) {
    match format {
        "mp3" => {
            command
                .arg("-c:a")
                .arg("libmp3lame")
                .arg("-b:a")
                .arg(format!("{bitrate}k"));
        }
        "m4a" => {
            command
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg(format!("{bitrate}k"));
        }
        "wav" => {
            command.arg("-c:a").arg("pcm_s16le");
        }
        "flac" => {
            command
                .arg("-c:a")
                .arg("flac")
                .arg("-compression_level")
                .arg("8");
        }
        "ogg" => {
            command
                .arg("-c:a")
                .arg("libvorbis")
                .arg("-b:a")
                .arg(format!("{bitrate}k"));
        }
        "opus" => {
            command
                .arg("-c:a")
                .arg("libopus")
                .arg("-b:a")
                .arg(format!("{bitrate}k"));
        }
        "aiff" => {
            command.arg("-c:a").arg("pcm_s16be");
        }
        "wma" => {
            command
                .arg("-c:a")
                .arg("wmav2")
                .arg("-b:a")
                .arg(format!("{bitrate}k"));
        }
        _ => {
            command
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg(format!("{bitrate}k"));
        }
    }
}

fn atempo_chain(mut factor: f64) -> String {
    let mut filters = Vec::new();
    while factor > 2.0 {
        filters.push("atempo=2.0".to_owned());
        factor /= 2.0;
    }
    while factor < 0.5 {
        filters.push("atempo=0.5".to_owned());
        factor /= 0.5;
    }
    if (factor - 1.0).abs() > f64::EPSILON {
        filters.push(format!("atempo={factor}"));
    }
    if filters.is_empty() {
        "atempo=1.0".into()
    } else {
        filters.join(",")
    }
}

fn trim_number(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn even(value: u32) -> u32 {
    value - (value % 2)
}

fn dimension(raw: &str, total: u32, label: &str) -> ToolResult<u32> {
    let raw = raw.trim();
    if let Some(percent) = raw.strip_suffix('%') {
        let percent = percent
            .parse::<f64>()
            .map_err(|_| format!("{label} must be a percentage"))?;
        if !percent.is_finite() || percent <= 0.0 || percent > 100.0 {
            return Err(format!("{label} percentage must be between 0 and 100"));
        }
        return Ok(((total as f64 * percent / 100.0).round() as u32).max(2));
    }
    let value = raw
        .parse::<u32>()
        .map_err(|_| format!("{label} must be pixels or a percentage"))?;
    if value < 2 {
        return Err(format!("{label} must be at least 2 pixels"));
    }
    Ok(value)
}

fn offset(raw: &str, total: u32, size: u32, label: &str) -> ToolResult<u32> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("center") {
        return Ok((total.saturating_sub(size) / 2) & !1);
    }
    if let Some(percent) = raw.strip_suffix('%') {
        let percent = percent
            .parse::<f64>()
            .map_err(|_| format!("{label} must be a percentage"))?;
        if !percent.is_finite() || percent < 0.0 || percent > 100.0 {
            return Err(format!("{label} percentage must be between 0 and 100"));
        }
        return Ok(((total as f64 * percent / 100.0).round() as u32) & !1);
    }
    let value = raw
        .parse::<u32>()
        .map_err(|_| format!("{label} must be pixels, a percentage, or center"))?;
    Ok(value & !1)
}

fn list_files(directory: &Path) -> ToolResult<Vec<PathBuf>> {
    let mut files = fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}
