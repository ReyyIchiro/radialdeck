//! Image tools built on ImageMagick.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::shell::{
    extension, image_dimensions, magick_command, parse_u32, require, run, source_parent,
    source_stem, unique_path, ToolResult,
};
use super::{quality, value, ToolFailure, ToolId, ToolOutcome};

pub fn execute(id: ToolId, sources: &[PathBuf], values: &BTreeMap<String, String>) -> ToolOutcome {
    match id {
        ToolId::CreatePdf => create_pdf(sources),
        ToolId::CreateCollage => create_collage(sources, values),
        ToolId::CropImage => each(sources, |source| crop(source, values)),
        ToolId::EditImage => each(sources, |source| edit(source, values)),
        ToolId::AnnotateImage => each(sources, |source| annotate(source, values)),
        ToolId::FrameImage => each(sources, |source| frame(source, values)),
        ToolId::RedactImage => each(sources, |source| redact(source, values)),
        ToolId::Compress => each(sources, |source| compress(source, values)),
        ToolId::RemoveMetadata => each(sources, strip_metadata),
        _ => ToolOutcome::failure(
            sources
                .first()
                .map(PathBuf::as_path)
                .unwrap_or(Path::new("")),
            "unsupported image tool".to_owned(),
        ),
    }
}

fn each<F>(sources: &[PathBuf], mut operation: F) -> ToolOutcome
where
    F: FnMut(&Path) -> ToolResult<Vec<PathBuf>>,
{
    let mut outcome = ToolOutcome::default();
    for source in sources {
        match operation(source) {
            Ok(created) => outcome.created.extend(created),
            Err(message) => outcome.failures.push(ToolFailure {
                source: source.clone(),
                message,
            }),
        }
    }
    outcome
}

fn create_pdf(sources: &[PathBuf]) -> ToolOutcome {
    let first = &sources[0];
    let destination = unique_path(
        &source_parent(first),
        &format!("{}-images", source_stem(first)),
        "pdf",
    );
    let mut command = match magick_command() {
        Ok(command) => command,
        Err(message) => return ToolOutcome::failure(first, message),
    };
    for source in sources {
        command.arg(source);
    }
    command
        .arg("-auto-orient")
        .arg("-compress")
        .arg("jpeg")
        .arg("-quality")
        .arg("90")
        .arg(&destination);
    match run(&mut command, "ImageMagick") {
        Ok(()) => ToolOutcome::success(vec![destination]),
        Err(message) => ToolOutcome::failure(first, message),
    }
}

fn create_collage(sources: &[PathBuf], values: &BTreeMap<String, String>) -> ToolOutcome {
    let first = &sources[0];
    if sources.len() < 2 {
        return ToolOutcome::failure(first, "select at least two images".to_owned());
    }
    let columns = match parse_u32(value(values, "columns", "3"), "columns") {
        Ok(value) if (1..=8).contains(&value) => value,
        Ok(_) => return ToolOutcome::failure(first, "columns must be between 1 and 8"),
        Err(message) => return ToolOutcome::failure(first, message),
    };
    let cell = match parse_u32(value(values, "cell", "480"), "cell size") {
        Ok(value) if (32..=4096).contains(&value) => value,
        Ok(_) => return ToolOutcome::failure(first, "cell size must be between 32 and 4096"),
        Err(message) => return ToolOutcome::failure(first, message),
    };
    let destination = unique_path(&source_parent(first), "collage", "jpg");
    let montage = match require("montage") {
        Ok(binary) => binary,
        Err(message) => return ToolOutcome::failure(first, message),
    };
    let mut command = Command::new(montage);
    for source in sources {
        command.arg(source);
    }
    command
        .arg("-auto-orient")
        .arg("-tile")
        .arg(format!("{columns}x"))
        .arg("-geometry")
        .arg(format!("{cell}x{cell}+8+8"))
        .arg("-background")
        .arg("#ffffff")
        .arg(&destination);
    match run(&mut command, "ImageMagick montage") {
        Ok(()) => ToolOutcome::success(vec![destination]),
        Err(message) => ToolOutcome::failure(first, message),
    }
}

fn crop(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let (width, height) = image_dimensions(source)?;
    let crop_width = dimension(value(values, "width", "80%"), width, "width")?;
    let crop_height = dimension(value(values, "height", "80%"), height, "height")?;
    let x = offset(value(values, "x", "center"), width, crop_width, "x")?;
    let y = offset(value(values, "y", "center"), height, crop_height, "y")?;
    if crop_width == 0 || crop_height == 0 {
        return Err("crop dimensions must be greater than zero".into());
    }
    if x + crop_width > width || y + crop_height > height {
        return Err(format!(
            "crop rectangle is outside the image ({width}×{height})"
        ));
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-cropped", source_stem(source)),
        &extension(source),
    );
    let mut command = magick_command()?;
    command
        .arg(source)
        .arg("-auto-orient")
        .arg("-crop")
        .arg(format!("{crop_width}x{crop_height}+{x}+{y}"))
        .arg("+repage")
        .arg(&destination);
    run(&mut command, "ImageMagick crop")?;
    Ok(vec![destination])
}

fn edit(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let brightness = signed(values, "brightness", 0, -100, 100)?;
    let contrast = signed(values, "contrast", 0, -100, 100)?;
    let saturation = signed(values, "saturation", 0, -100, 100)?;
    let rotate = value(values, "rotate", "0");
    if !matches!(rotate, "0" | "90" | "180" | "270") {
        return Err("rotation must be 0, 90, 180, or 270".into());
    }
    let flip = value(values, "flip", "none");
    if !matches!(flip, "none" | "horizontal" | "vertical") {
        return Err("flip must be none, horizontal, or vertical".into());
    }

    let destination = unique_path(
        &source_parent(source),
        &format!("{}-edited", source_stem(source)),
        &extension(source),
    );
    let mut command = magick_command()?;
    command.arg(source).arg("-auto-orient");
    if brightness != 0 || contrast != 0 {
        command
            .arg("-brightness-contrast")
            .arg(format!("{brightness}x{contrast}"));
    }
    if saturation != 0 {
        command
            .arg("-modulate")
            .arg(format!("100,{},100", 100 + saturation));
    }
    if rotate != "0" {
        command.arg("-rotate").arg(rotate);
    }
    match flip {
        "horizontal" => {
            command.arg("-flop");
        }
        "vertical" => {
            command.arg("-flip");
        }
        _ => {}
    }
    command.arg(&destination);
    run(&mut command, "ImageMagick edit")?;
    Ok(vec![destination])
}

fn annotate(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let text = value(values, "text", "");
    if text.is_empty() {
        return Err("annotation text cannot be empty".into());
    }
    let gravity = value(values, "gravity", "South");
    if !matches!(gravity, "North" | "Center" | "South") {
        return Err("position must be North, Center, or South".into());
    }
    let size = parse_u32(value(values, "size", "32"), "text size")?;
    if !(1..=500).contains(&size) {
        return Err("text size must be between 1 and 500".into());
    }
    let color = value(values, "color", "#ff3b30");
    if !valid_color(color) {
        return Err("color must be #rrggbb or a CSS color name".into());
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-annotated", source_stem(source)),
        &extension(source),
    );
    let mut command = magick_command()?;
    command
        .arg(source)
        .arg("-auto-orient")
        .arg("-gravity")
        .arg(gravity)
        .arg("-fill")
        .arg(color)
        .arg("-pointsize")
        .arg(size.to_string())
        .arg("-annotate")
        .arg("+0+24")
        .arg(text)
        .arg(&destination);
    run(&mut command, "ImageMagick annotate")?;
    Ok(vec![destination])
}

fn frame(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let color = value(values, "color", "#ffffff");
    if !valid_color(color) {
        return Err("background must be #rrggbb or a CSS color name".into());
    }
    let padding = parse_u32(value(values, "padding", "32"), "padding")?;
    if padding > 5000 {
        return Err("padding must be 5000 pixels or less".into());
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-framed", source_stem(source)),
        &extension(source),
    );
    let mut command = magick_command()?;
    command
        .arg(source)
        .arg("-auto-orient")
        .arg("-background")
        .arg(color)
        .arg("-alpha")
        .arg("remove")
        .arg("-alpha")
        .arg("off")
        .arg("-bordercolor")
        .arg(color)
        .arg("-border")
        .arg(padding.to_string())
        .arg(&destination);
    run(&mut command, "ImageMagick background")?;
    Ok(vec![destination])
}

fn redact(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let (width, height) = image_dimensions(source)?;
    let (x, y, w, h) = rectangle(values, width, height)?;
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-redacted", source_stem(source)),
        &extension(source),
    );
    let mut command = magick_command()?;
    command
        .arg(source)
        .arg("-auto-orient")
        .arg("-fill")
        .arg("black")
        .arg("-draw")
        .arg(format!("rectangle {x},{y} {},{}", x + w, y + h))
        .arg(&destination);
    run(&mut command, "ImageMagick redact")?;
    Ok(vec![destination])
}

fn compress(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let quality = quality(values, "quality", 82)?;
    if !(1..=100).contains(&quality) {
        return Err("quality must be between 1 and 100".into());
    }
    let max_width = parse_u32(value(values, "max_width", "2560"), "max width")?;
    let format = extension(source);
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-compressed", source_stem(source)),
        &format,
    );
    let mut command = magick_command()?;
    command.arg(source).arg("-auto-orient");
    if max_width > 0 {
        command
            .arg("-resize")
            .arg(format!("{max_width}x{max_width}>"));
    }
    match format.as_str() {
        "png" => {
            command
                .arg("-strip")
                .arg("-define")
                .arg("png:compression-level=9");
        }
        "gif" => {
            command.arg("-layers").arg("optimize");
        }
        _ => {
            command
                .arg("-strip")
                .arg("-quality")
                .arg(quality.to_string());
        }
    }
    command.arg(&destination);
    run(&mut command, "ImageMagick compress")?;
    Ok(vec![destination])
}

fn strip_metadata(source: &Path) -> ToolResult<Vec<PathBuf>> {
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-clean", source_stem(source)),
        &extension(source),
    );
    let mut command = magick_command()?;
    command
        .arg(source)
        .arg("-auto-orient")
        .arg("-strip")
        .arg(&destination);
    run(&mut command, "ImageMagick metadata")?;
    Ok(vec![destination])
}

fn signed(
    values: &BTreeMap<String, String>,
    key: &str,
    fallback: i32,
    minimum: i32,
    maximum: i32,
) -> ToolResult<i32> {
    let raw = value(values, key, "");
    let value = if raw.is_empty() {
        fallback
    } else {
        raw.parse::<i32>()
            .map_err(|_| format!("{key} must be a whole number"))?
    };
    if value < minimum || value > maximum {
        return Err(format!("{key} must be between {minimum} and {maximum}"));
    }
    Ok(value)
}

fn valid_color(color: &str) -> bool {
    if let Some(hex) = color.strip_prefix('#') {
        return (hex.len() == 6 || hex.len() == 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    }
    !color.is_empty()
        && color
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() || ch.is_ascii_whitespace())
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
        return Ok(((total as f64 * percent / 100.0).round() as u32).max(1));
    }
    let value = raw
        .parse::<u32>()
        .map_err(|_| format!("{label} must be pixels or a percentage"))?;
    if value == 0 {
        return Err(format!("{label} must be greater than zero"));
    }
    Ok(value)
}

fn offset(raw: &str, total: u32, size: u32, label: &str) -> ToolResult<u32> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("center") {
        return Ok(total.saturating_sub(size) / 2);
    }
    if let Some(percent) = raw.strip_suffix('%') {
        let percent = percent
            .parse::<f64>()
            .map_err(|_| format!("{label} must be a percentage"))?;
        if !percent.is_finite() || percent < 0.0 || percent > 100.0 {
            return Err(format!("{label} percentage must be between 0 and 100"));
        }
        return Ok((total as f64 * percent / 100.0).round() as u32);
    }
    raw.parse::<u32>()
        .map_err(|_| format!("{label} must be pixels, a percentage, or center"))
}

fn rectangle(
    values: &BTreeMap<String, String>,
    width: u32,
    height: u32,
) -> ToolResult<(u32, u32, u32, u32)> {
    let w = dimension(value(values, "width", "40%"), width, "width")?;
    let h = dimension(value(values, "height", "30%"), height, "height")?;
    let x = offset(value(values, "x", "30%"), width, w, "x")?;
    let y = offset(value(values, "y", "35%"), height, h, "y")?;
    if w == 0 || h == 0 || x + w > width || y + h > height {
        return Err(format!(
            "redaction rectangle is outside the image ({width}×{height})"
        ));
    }
    Ok((x, y, w, h))
}
