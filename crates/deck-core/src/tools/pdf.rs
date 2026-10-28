//! PDF and archive tools built on Poppler, Ghostscript and 7-Zip.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::shell::{
    create_dir, extension, require, run, source_parent, source_stem, unique_dir, unique_path,
    ToolResult,
};
use super::{value, ToolFailure, ToolId, ToolOutcome};
use crate::convert::{Category, Tools};

pub fn execute(id: ToolId, sources: &[PathBuf], values: &BTreeMap<String, String>) -> ToolOutcome {
    match id {
        ToolId::MergePdf => merge(sources),
        ToolId::SplitPdf => each(sources, |source| split(source, values)),
        ToolId::OrganizePdf => each(sources, |source| organize(source, values)),
        ToolId::Compress => each(sources, |source| compress(source, values)),
        ToolId::RemoveMetadata => each(sources, clean_metadata),
        _ => ToolOutcome::failure(
            sources
                .first()
                .map(PathBuf::as_path)
                .unwrap_or(Path::new("")),
            "unsupported PDF tool".to_owned(),
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

fn pdfunite() -> ToolResult<PathBuf> {
    require("pdfunite")
}

fn pdfseparate() -> ToolResult<PathBuf> {
    require("pdfseparate")
}

fn merge(sources: &[PathBuf]) -> ToolOutcome {
    let first = &sources[0];
    if sources.len() < 2 {
        return ToolOutcome::failure(first, "select at least two PDFs".to_owned());
    }
    let destination = unique_path(&source_parent(first), "merged", "pdf");
    let binary = match pdfunite() {
        Ok(binary) => binary,
        Err(message) => return ToolOutcome::failure(first, message),
    };
    let mut command = Command::new(binary);
    for source in sources {
        command.arg(source);
    }
    command.arg(&destination);
    match run(&mut command, "pdfunite") {
        Ok(()) => ToolOutcome::success(vec![destination]),
        Err(message) => {
            let _ = fs::remove_file(&destination);
            ToolOutcome::failure(first, message)
        }
    }
}

fn split(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let temp = tempfile::tempdir().map_err(|error| format!("create temp dir: {error}"))?;
    let pages = separate_pages(source, temp.path())?;
    let ranges = parse_ranges(value(values, "ranges", ""), pages.len())?;
    let directory = unique_dir(
        &source_parent(source),
        &format!("{}-split", source_stem(source)),
    );
    create_dir(&directory)?;
    let unite = pdfunite()?;
    let mut created = Vec::new();
    for range in ranges {
        let name = if range.len() == 1 {
            format!("{}-page-{}.pdf", source_stem(source), range[0])
        } else {
            format!(
                "{}-{}-{}.pdf",
                source_stem(source),
                range.first().copied().unwrap_or(1),
                range.last().copied().unwrap_or(1)
            )
        };
        let destination = unique_path(&directory, &name, "");
        let mut command = Command::new(&unite);
        for page in &range {
            command.arg(&pages[*page - 1]);
        }
        command.arg(&destination);
        run(&mut command, "pdfunite")?;
        created.push(destination);
    }
    Ok(created)
}

fn organize(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let temp = tempfile::tempdir().map_err(|error| format!("create temp dir: {error}"))?;
    let pages = separate_pages(source, temp.path())?;
    let order = parse_order(value(values, "order", ""), pages.len())?;
    let selected = temp.path().join("selected");
    create_dir(&selected)?;
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-organized", source_stem(source)),
        "pdf",
    );
    let mut command = Command::new(pdfunite()?);
    for (index, page) in order.iter().enumerate() {
        // Copying each page also makes duplicate page numbers valid input.
        let path = selected.join(format!("{index:04}.pdf"));
        fs::copy(&pages[*page - 1], &path)
            .map_err(|error| format!("copy page {}: {error}", page))?;
        command.arg(path);
    }
    command.arg(&destination);
    run(&mut command, "pdfunite")?;
    Ok(vec![destination])
}

fn compress(source: &Path, values: &BTreeMap<String, String>) -> ToolResult<Vec<PathBuf>> {
    let preset = value(values, "preset", "ebook");
    if !matches!(preset, "screen" | "ebook" | "printer" | "prepress") {
        return Err("preset must be screen, ebook, printer, or prepress".into());
    }
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-compressed", source_stem(source)),
        "pdf",
    );
    ghostscript(source, &destination, preset)?;
    Ok(vec![destination])
}

fn clean_metadata(source: &Path) -> ToolResult<Vec<PathBuf>> {
    // Rebuilding the document through separate + unite drops document info,
    // XMP metadata and bookmarks while leaving page content intact.
    let temp = tempfile::tempdir().map_err(|error| format!("create temp dir: {error}"))?;
    let pages = separate_pages(source, temp.path())?;
    let destination = unique_path(
        &source_parent(source),
        &format!("{}-clean", source_stem(source)),
        "pdf",
    );
    let mut command = Command::new(pdfunite()?);
    for page in pages {
        command.arg(page);
    }
    command.arg(&destination);
    run(&mut command, "pdfunite")?;
    Ok(vec![destination])
}

fn ghostscript(source: &Path, destination: &Path, preset: &str) -> ToolResult<()> {
    let binary = require("gs")?;
    let mut command = Command::new(binary);
    command
        .arg("-sDEVICE=pdfwrite")
        .arg("-dCompatibilityLevel=1.7")
        .arg(format!("-dPDFSETTINGS=/{preset}"))
        .arg("-dNOPAUSE")
        .arg("-dQUIET")
        .arg("-dBATCH")
        .arg(format!("-sOutputFile={}", destination.display()))
        .arg(source);
    match run(&mut command, "Ghostscript") {
        Ok(()) => Ok(()),
        Err(message) => {
            let _ = fs::remove_file(destination);
            Err(message)
        }
    }
}

fn separate_pages(source: &Path, directory: &Path) -> ToolResult<Vec<PathBuf>> {
    let pattern = directory.join("page-%d.pdf");
    let mut command = Command::new(pdfseparate()?);
    command.arg(source).arg(&pattern);
    run(&mut command, "pdfseparate")?;
    let mut pages = fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("page-") && name.ends_with(".pdf"))
        })
        .collect::<Vec<_>>();
    pages.sort_by_key(|path| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.strip_prefix("page-"))
            .and_then(|number| number.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    if pages.is_empty() {
        return Err("the PDF has no pages".into());
    }
    Ok(pages)
}

fn parse_ranges(raw: &str, page_count: usize) -> ToolResult<Vec<Vec<usize>>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("enter a page range such as 1-3,4-6".into());
    }
    if raw.eq_ignore_ascii_case("every") {
        return Ok((1..=page_count).map(|page| vec![page]).collect());
    }
    let mut ranges = Vec::new();
    for piece in raw.split(',') {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }
        let (start, end) = match piece.split_once('-') {
            Some((start, end)) => (start.trim(), end.trim()),
            None => (piece, piece),
        };
        let start = page_number(start, page_count)?;
        let end = page_number(end, page_count)?;
        if end < start {
            return Err(format!("range {piece} is backwards"));
        }
        ranges.push((start..=end).collect());
    }
    if ranges.is_empty() {
        return Err("enter at least one page range".into());
    }
    Ok(ranges)
}

fn parse_order(raw: &str, page_count: usize) -> ToolResult<Vec<usize>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("enter a page order such as 3,1,2".into());
    }
    let mut order = Vec::new();
    for piece in raw.split(',') {
        if piece.trim().is_empty() {
            continue;
        }
        order.push(page_number(piece, page_count)?);
    }
    if order.is_empty() {
        return Err("enter at least one page number".into());
    }
    Ok(order)
}

fn page_number(raw: &str, page_count: usize) -> ToolResult<usize> {
    let page = raw
        .trim()
        .parse::<usize>()
        .map_err(|_| format!("invalid page number: {raw}"))?;
    if page == 0 || page > page_count {
        return Err(format!("page {page} is outside 1–{page_count}"));
    }
    Ok(page)
}

pub fn extract_archive(sources: &[PathBuf]) -> ToolOutcome {
    let mut outcome = ToolOutcome::default();
    for source in sources {
        let ext = extension(source);
        let result = match ext.as_str() {
            "zip" | "tar" | "gz" | "tgz" => {
                let result = crate::convert::convert(
                    std::slice::from_ref(source),
                    Category::Archives,
                    "extract",
                    &Tools::default(),
                );
                if result.failures.is_empty() {
                    Ok(result.created)
                } else {
                    Err(result
                        .failures
                        .into_iter()
                        .next()
                        .map(|failure| failure.message)
                        .unwrap_or_else(|| "extraction failed".into()))
                }
            }
            "rar" | "7z" => extract_with_7z(source),
            _ => Err("choose a ZIP, TAR, GZ, RAR, or 7Z archive".into()),
        };
        match result {
            Ok(created) => outcome.created.extend(created),
            Err(message) => outcome.failures.push(ToolFailure {
                source: source.clone(),
                message,
            }),
        }
    }
    outcome
}

fn extract_with_7z(source: &Path) -> ToolResult<Vec<PathBuf>> {
    let directory = unique_dir(&source_parent(source), &source_stem(source));
    create_dir(&directory)?;
    let binary = require("7z").or_else(|_| require("7zz"))?;
    let mut command = Command::new(binary);
    command
        .arg("x")
        .arg("-y")
        .arg("-bd")
        .arg(format!("-o{}", directory.display()))
        .arg("--")
        .arg(source);
    run(&mut command, "7-Zip")?;
    let mut created = Vec::new();
    for entry in fs::read_dir(&directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
    {
        created.push(entry.path());
    }
    created.sort();
    created.insert(0, directory.clone());
    Ok(created)
}
