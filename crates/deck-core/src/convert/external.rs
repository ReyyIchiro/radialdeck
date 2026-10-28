//! Adapters for the external conversion tools available on Linux desktops.
//!
//! Every command is invoked with an argument vector; no shell is involved, so
//! paths and format names cannot be interpreted as commands.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::output::{unique_named, unique_path};
use super::registry::find_tool;
use super::ConvertError;

fn run(command: &mut Command, label: &str) -> Result<(), ConvertError> {
    crate::tools::run_command(command, label).map_err(ConvertError)
}

fn magick_bin() -> Result<PathBuf, ConvertError> {
    find_tool("magick")
        .or_else(|| find_tool("convert"))
        .ok_or_else(|| ConvertError("ImageMagick is not installed".into()))
}

fn require_tool(name: &str) -> Result<PathBuf, ConvertError> {
    find_tool(name).ok_or_else(|| ConvertError(format!("{name} is not installed")))
}

fn source_stem(source: &Path) -> String {
    source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into())
}

fn source_parent(source: &Path) -> PathBuf {
    crate::job::destination_dir(source)
}

fn move_into(source: &Path, destination: &Path) -> Result<(), ConvertError> {
    if std::fs::rename(source, destination).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, destination)
        .map_err(|error| ConvertError(format!("write {}: {error}", destination.display())))?;
    let _ = std::fs::remove_file(source);
    Ok(())
}

/// ImageMagick conversion, used for HEIC/AVIF/SVG and other formats the native
/// image crate cannot encode or decode.
pub fn magick_convert(source: &Path, target_extension: &str) -> Result<Vec<PathBuf>, ConvertError> {
    let bin = magick_bin()?;
    let destination = unique_path(
        &source_parent(source),
        &source_stem(source),
        target_extension,
    );
    run(
        Command::new(bin)
            .arg(source)
            .arg("-auto-orient")
            .arg("-quality")
            .arg("90")
            .arg(&destination),
        "ImageMagick",
    )?;
    Ok(vec![destination])
}

pub fn image_to_pdf(source: &Path) -> Result<Vec<PathBuf>, ConvertError> {
    let bin = magick_bin()?;
    let destination = unique_path(&source_parent(source), &source_stem(source), "pdf");
    run(
        Command::new(bin)
            .arg(source)
            .arg("-auto-orient")
            .arg(&destination),
        "ImageMagick",
    )?;
    Ok(vec![destination])
}

pub fn ffmpeg_convert(source: &Path, target_extension: &str) -> Result<Vec<PathBuf>, ConvertError> {
    let destination = unique_path(
        &source_parent(source),
        &source_stem(source),
        target_extension,
    );
    run(
        Command::new(require_tool("ffmpeg")?)
            .arg("-nostdin")
            .arg("-y")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(source)
            .arg(&destination),
        "ffmpeg",
    )?;
    Ok(vec![destination])
}

/// Renders every PDF page at 300 DPI, matching the reference app.
pub fn pdf_raster(source: &Path, target_extension: &str) -> Result<Vec<PathBuf>, ConvertError> {
    rasterize(
        source,
        &source_parent(source),
        &source_stem(source),
        target_extension,
    )
}

fn rasterize(
    pdf: &Path,
    output_dir: &Path,
    stem: &str,
    target_extension: &str,
) -> Result<Vec<PathBuf>, ConvertError> {
    let temp =
        tempfile::tempdir().map_err(|error| ConvertError(format!("create temp dir: {error}")))?;
    let prefix = temp.path().join("page");
    let mut command = Command::new(require_tool("pdftoppm")?);
    command.arg("-r").arg("300");
    match target_extension {
        "png" => {
            command.arg("-png");
        }
        "jpg" | "jpeg" => {
            command.arg("-jpeg").arg("-jpegopt").arg("quality=90");
        }
        other => return Err(ConvertError(format!("unsupported raster format: {other}"))),
    }
    command.arg(pdf).arg(&prefix);
    run(&mut command, "pdftoppm")?;

    let mut pages: Vec<(u32, PathBuf)> = std::fs::read_dir(temp.path())
        .map_err(|error| ConvertError(format!("read temp dir: {error}")))?
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            let page = path
                .file_stem()
                .and_then(|stem| {
                    stem.to_string_lossy()
                        .rsplit('-')
                        .next()?
                        .parse::<u32>()
                        .ok()
                })
                .unwrap_or(1);
            (page, path)
        })
        .collect();
    pages.sort_by_key(|(page, _)| *page);
    if pages.is_empty() {
        return Err(ConvertError("pdftoppm produced no pages".into()));
    }

    let mut outputs = Vec::new();
    let single = pages.len() == 1;
    for (page, path) in pages {
        let name = if single {
            format!("{stem}.{target_extension}")
        } else {
            format!("{stem}-{page}.{target_extension}")
        };
        let destination = unique_named(output_dir, &name);
        move_into(&path, &destination)?;
        outputs.push(destination);
    }
    Ok(outputs)
}

pub fn pdf_text(source: &Path, target_extension: &str) -> Result<Vec<PathBuf>, ConvertError> {
    let temp =
        tempfile::tempdir().map_err(|error| ConvertError(format!("create temp dir: {error}")))?;
    let raw = temp.path().join("text.txt");
    run(
        Command::new(require_tool("pdftotext")?)
            .arg("-layout")
            .arg(source)
            .arg(&raw),
        "pdftotext",
    )?;
    let destination = unique_path(
        &source_parent(source),
        &source_stem(source),
        target_extension,
    );
    move_into(&raw, &destination)?;
    Ok(vec![destination])
}

/// LibreOffice conversions. Documents, slides, spreadsheets and EPUB all go
/// through the same headless instance with a private profile so a running
/// LibreOffice session is never reused or disturbed.
/// Converts a document to Markdown through pandoc, which understands ODT,
/// DOCX, RTF, HTML and EPUB inputs.
pub fn pandoc_to_markdown(source: &Path) -> Result<Vec<PathBuf>, ConvertError> {
    let binary = require_tool("pandoc")?;
    let destination = unique_path(&source_parent(source), &source_stem(source), "md");
    run(
        Command::new(binary)
            .arg("--to")
            .arg("markdown")
            .arg("--output")
            .arg(&destination)
            .arg(source),
        "pandoc",
    )?;
    Ok(vec![destination])
}

pub fn libreoffice_convert(
    source: &Path,
    target_extension: &str,
) -> Result<Vec<PathBuf>, ConvertError> {
    let (filter, produced_extension) = libreoffice_filter(target_extension)?;
    let out =
        tempfile::tempdir().map_err(|error| ConvertError(format!("create temp dir: {error}")))?;
    let profile =
        tempfile::tempdir().map_err(|error| ConvertError(format!("create temp dir: {error}")))?;
    let profile_url = format!("file://{}", profile.path().to_string_lossy());
    let binary = find_tool("soffice")
        .or_else(|| find_tool("libreoffice"))
        .ok_or_else(|| ConvertError("LibreOffice is not installed".into()))?;

    let mut command = Command::new(binary);
    command
        .arg("--headless")
        .arg("--norestore")
        .arg("--nolockcheck")
        .arg(format!("-env:UserInstallation={profile_url}"));
    // LibreOffice imports HTML as a Writer/Web document by default, which has
    // no export filter for office formats. Import it as Writer instead.
    let html_source = source
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm"));
    if html_source && target_extension != "html" {
        command.arg("--infilter=HTML (StarWriter)");
    }
    run(
        command
            .arg("--convert-to")
            .arg(filter)
            .arg("--outdir")
            .arg(out.path())
            .arg(source),
        "LibreOffice",
    )?;

    let produced = find_output(out.path(), produced_extension).ok_or_else(|| {
        ConvertError(format!(
            "LibreOffice produced no .{produced_extension} file"
        ))
    })?;
    let destination = unique_path(
        &source_parent(source),
        &source_stem(source),
        target_extension,
    );
    move_into(&produced, &destination)?;
    Ok(vec![destination])
}

/// LibreOffice → PDF → 300 DPI raster for slide export.
pub fn libreoffice_to_images(
    source: &Path,
    target_extension: &str,
) -> Result<Vec<PathBuf>, ConvertError> {
    let pdf = libreoffice_convert(source, "pdf")?;
    let pdf = pdf
        .into_iter()
        .next()
        .ok_or_else(|| ConvertError("LibreOffice produced no PDF".into()))?;
    let result = rasterize(
        &pdf,
        &source_parent(source),
        &source_stem(source),
        target_extension,
    );
    let _ = std::fs::remove_file(&pdf);
    result
}

fn find_output(dir: &Path, extension: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        })
}

fn libreoffice_filter(
    target_extension: &str,
) -> Result<(&'static str, &'static str), ConvertError> {
    let pair = match target_extension {
        "pdf" => ("pdf", "pdf"),
        "docx" => ("docx:MS Word 2007 XML", "docx"),
        "odt" => ("odt:writer8", "odt"),
        "rtf" => ("rtf:Rich Text Format", "rtf"),
        "txt" => ("txt:Text (encoded):UTF8", "txt"),
        "html" => ("html:HTML (StarWriter)", "html"),
        "epub" => ("epub:EPUB", "epub"),
        "xlsx" => ("xlsx:Calc MS Excel 2007 XML", "xlsx"),
        "ods" => ("ods:calc8", "ods"),
        "pptx" => ("pptx:Impress MS PowerPoint 2007 XML", "pptx"),
        "odp" => ("odp:impress8", "odp"),
        "csv" => ("csv:Text - txt - csv (StarCalc):44,34,76,1", "csv"),
        "tsv" => ("csv:Text - txt - csv (StarCalc):9,34,76,1", "csv"),
        other => return Err(ConvertError(format!("no LibreOffice filter for {other}"))),
    };
    Ok(pair)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_table_covers_supported_targets() {
        assert_eq!(libreoffice_filter("pdf").unwrap().0, "pdf");
        assert!(libreoffice_filter("docx").is_ok());
        assert!(libreoffice_filter("nope").is_err());
    }
}
