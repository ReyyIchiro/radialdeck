//! File conversion engine shared by every frontend.
//!
//! Native Rust paths cover images, archives and subtitle text. Formats that
//! depend on system libraries are delegated to ImageMagick, ffmpeg, pdftoppm /
//! pdftotext and LibreOffice, each invoked without a shell.

mod archive;
mod external;
mod image;
mod output;
pub mod registry;
mod subtitle;

use std::path::{Path, PathBuf};

pub use registry::{ArchiveKind, Category, Target, Tools};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ConvertError(pub String);

#[derive(Debug, Clone)]
pub struct Failure {
    pub source: PathBuf,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub created: Vec<PathBuf>,
    pub failures: Vec<Failure>,
}

impl Outcome {
    fn push(&mut self, result: Result<Vec<PathBuf>, ConvertError>, source: &Path) {
        match result {
            Ok(paths) => self.created.extend(paths),
            Err(error) => self.failures.push(Failure {
                source: source.to_path_buf(),
                message: error.0,
            }),
        }
    }
}

/// Converts every source to `target` within `category`.
///
/// Files that cannot be converted are reported individually; one failure never
/// aborts the rest of the batch.
/// Converts with an explicit output location and cancellation token.
pub fn convert_with(
    sources: &[PathBuf],
    category: Category,
    target: &str,
    tools: &Tools,
    output: crate::OutputLocation,
    cancel: crate::CancelToken,
    reporter: crate::job::Reporter,
) -> Outcome {
    crate::job::scope(output, Some(cancel), || {
        crate::job::set_reporter(reporter);
        let mut outcome = convert(sources, category, target, tools);
        if crate::job::cancelled() {
            crate::job::cleanup_registered();
            outcome.created.clear();
        }
        outcome
    })
}

pub fn convert(sources: &[PathBuf], category: Category, target: &str, _tools: &Tools) -> Outcome {
    let mut outcome = Outcome::default();
    crate::job::set_total(sources.len());
    for source in sources {
        if crate::job::cancelled() {
            outcome.failures.push(Failure {
                source: source.clone(),
                message: "Cancelled".into(),
            });
            break;
        }
        if !source.exists() {
            outcome.push(
                Err(ConvertError(format!(
                    "file not found: {}",
                    source.display()
                ))),
                source,
            );
            continue;
        }
        let result = convert_one(source, category, target);
        outcome.push(result, source);
        crate::job::report_file(source);
    }
    outcome
}

fn convert_one(
    source: &Path,
    category: Category,
    target: &str,
) -> Result<Vec<PathBuf>, ConvertError> {
    let extension = registry::extension(source).unwrap_or_default();
    match category {
        Category::Images => convert_image(source, &extension, target),
        Category::Video | Category::Audio => external::ffmpeg_convert(source, target),
        Category::Pdf => match target {
            "png" | "jpg" | "jpeg" => external::pdf_raster(source, target),
            "txt" | "md" => external::pdf_text(source, target),
            other => Err(ConvertError(format!(
                "PDF → {other} needs an OCR pipeline, which is not bundled"
            ))),
        },
        Category::Documents | Category::Epub => {
            if target == "md" {
                external::pandoc_to_markdown(source)
            } else {
                external::libreoffice_convert(source, target)
            }
        }
        Category::Spreadsheets => external::libreoffice_convert(source, target),
        Category::Slides => match target {
            "jpg" | "jpeg" | "png" => external::libreoffice_to_images(source, target),
            _ => external::libreoffice_convert(source, target),
        },
        Category::Subtitles => match target {
            "txt" => {
                let destination = output::unique_path(
                    &crate::job::destination_dir(source),
                    &source
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "subtitle".into()),
                    "txt",
                );
                subtitle::to_text(source, &destination)?;
                Ok(vec![destination])
            }
            "srt" | "vtt" => external::ffmpeg_convert(source, target),
            other => Err(ConvertError(format!(
                "unsupported subtitle target: {other}"
            ))),
        },
        Category::Archives => match target {
            "extract" => archive::extract(source).map(|path| vec![path]),
            "zip" | "tar" | "gzip" => archive::compress(source, target).map(|path| vec![path]),
            other => Err(ConvertError(format!("unsupported archive target: {other}"))),
        },
    }
}

fn convert_image(
    source: &Path,
    source_extension: &str,
    target: &str,
) -> Result<Vec<PathBuf>, ConvertError> {
    let native_source = image::is_native(source_extension);
    let magick_source = registry::IMAGE_MAGICK.contains(&source_extension);
    if !native_source && !magick_source {
        return Err(ConvertError(format!(
            "unsupported image source: .{source_extension}"
        )));
    }

    if target == "pdf" {
        return external::image_to_pdf(source);
    }
    if image::is_native(target) && native_source {
        return image::convert(source, target).map(|path| vec![path]);
    }
    if magick_source || matches!(target, "heic" | "avif") {
        return external::magick_convert(source, target);
    }
    Err(ConvertError(format!("unsupported image target: {target}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_sources_are_reported_without_aborting() {
        let tools = Tools::default();
        let sources = vec![PathBuf::from("/definitely/missing.png")];
        let outcome = convert(&sources, Category::Images, "webp", &tools);
        assert!(outcome.created.is_empty());
        assert_eq!(outcome.failures.len(), 1);
    }

    #[test]
    fn conversion_honours_the_scope_output_location() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("one.png");
        ::image::RgbImage::from_pixel(4, 4, ::image::Rgb([1, 2, 3]))
            .save(&source)
            .unwrap();
        let target = dir.path().join("converted");
        let tools = Tools::default();
        let outcome = crate::job::scope(
            crate::OutputLocation::Subfolder("converted".into()),
            None,
            || {
                convert(
                    std::slice::from_ref(&source),
                    Category::Images,
                    "webp",
                    &tools,
                )
            },
        );
        assert_eq!(outcome.created.len(), 1);
        assert_eq!(outcome.created[0].parent().unwrap(), target);
        assert!(target.join("one.webp").exists());
    }

    #[test]
    fn cancelled_conversion_stops_before_the_next_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut sources = Vec::new();
        for name in ["one", "two"] {
            let path = dir.path().join(format!("{name}.png"));
            ::image::RgbImage::from_pixel(4, 4, ::image::Rgb([9, 9, 9]))
                .save(&path)
                .unwrap();
            sources.push(path);
        }
        let tools = Tools::default();
        let cancel = crate::CancelToken::new();
        cancel.cancel();
        let outcome = crate::job::scope(crate::OutputLocation::Alongside, Some(cancel), || {
            convert(&sources, Category::Images, "webp", &tools)
        });
        assert!(outcome.created.is_empty());
        assert!(outcome
            .failures
            .iter()
            .any(|failure| failure.message.contains("Cancelled")));
    }

    #[test]
    fn native_batch_converts_every_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut sources = Vec::new();
        for name in ["one", "two"] {
            let path = dir.path().join(format!("{name}.png"));
            ::image::RgbImage::from_pixel(4, 4, ::image::Rgb([1, 2, 3]))
                .save(&path)
                .unwrap();
            sources.push(path);
        }
        let tools = Tools::default();
        let outcome = convert(&sources, Category::Images, "webp", &tools);
        assert_eq!(outcome.created.len(), 2);
        assert!(outcome.failures.is_empty());
        assert!(outcome
            .created
            .iter()
            .all(|path| path.extension().unwrap() == "webp"));
    }
}
