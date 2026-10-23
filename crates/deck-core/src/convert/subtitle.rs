//! Plain-text extraction from SubRip and WebVTT tracks.

use std::path::Path;

use super::ConvertError;

pub fn to_text(source: &Path, destination: &Path) -> Result<(), ConvertError> {
    let raw = std::fs::read_to_string(source)
        .map_err(|error| ConvertError(format!("read {}: {error}", source.display())))?;
    let text = extract(&raw);
    std::fs::write(destination, text)
        .map_err(|error| ConvertError(format!("write {}: {error}", destination.display())))
}

/// Strips cue numbering, timing lines, tags and speaker markup.
pub fn extract(raw: &str) -> String {
    let mut out = String::new();
    let mut lines = raw.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            continue;
        }
        let next_is_timing = lines.peek().is_some_and(|next| next.contains("-->"));
        if line.eq_ignore_ascii_case("WEBVTT")
            || line.starts_with("NOTE")
            || line.starts_with("STYLE")
            || line.contains("-->")
            || line.chars().all(|ch| ch.is_ascii_digit())
            || next_is_timing
        {
            continue;
        }
        let cleaned = strip_tags(line);
        let cleaned = decode_entities(&cleaned);
        if cleaned.trim().is_empty() {
            continue;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push(' ');
        }
        out.push_str(cleaned.trim());
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_tag = false;
    for ch in line.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn decode_entities(line: &str) -> String {
    line.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt_without_timings_and_markup() {
        let raw = "1\n00:00:01,000 --> 00:00:03,000\nHello <i>world</i>\n\n2\n00:00:04,000 --> 00:00:05,000\nSecond &amp; last\n";
        assert_eq!(extract(raw), "Hello world\nSecond & last\n");
    }

    #[test]
    fn vtt_skips_headers_and_cue_ids() {
        let raw = "WEBVTT\n\ncue-1\n00:00.000 --> 00:01.000\nFirst line\nsecond line\n";
        assert_eq!(extract(raw), "First line second line\n");
    }
}
