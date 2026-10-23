//! Format registry and capability detection.
//!
//! The registry describes what a category can produce. Whether a target is
//! currently possible depends on the source kind and on which external tools
//! are installed, so availability is computed per invocation.

use std::path::Path;

/// Categories mirror the reference app's file families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Images,
    Video,
    Audio,
    Pdf,
    Documents,
    Slides,
    Spreadsheets,
    Epub,
    Subtitles,
    Archives,
}

impl Category {
    pub const ALL: [Category; 10] = [
        Category::Images,
        Category::Video,
        Category::Audio,
        Category::Pdf,
        Category::Documents,
        Category::Slides,
        Category::Spreadsheets,
        Category::Epub,
        Category::Subtitles,
        Category::Archives,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Images => "Images",
            Category::Video => "Video",
            Category::Audio => "Audio",
            Category::Pdf => "PDF",
            Category::Documents => "Documents",
            Category::Slides => "Slides",
            Category::Spreadsheets => "Spreadsheets",
            Category::Epub => "EPUB",
            Category::Subtitles => "Subtitles",
            Category::Archives => "Archives",
        }
    }

    /// Icon names from the freedesktop icon theme.
    pub fn icon(self) -> &'static str {
        match self {
            Category::Images => "image-x-generic-symbolic",
            Category::Video => "video-x-generic-symbolic",
            Category::Audio => "audio-x-generic-symbolic",
            Category::Pdf => "application-pdf-symbolic",
            Category::Documents => "x-office-document-symbolic",
            Category::Slides => "x-office-presentation-symbolic",
            Category::Spreadsheets => "x-office-spreadsheet-symbolic",
            Category::Epub => "application-epub+zip-symbolic",
            Category::Subtitles => "text-x-generic-symbolic",
            Category::Archives => "package-x-generic-symbolic",
        }
    }

    pub fn heading(self) -> &'static str {
        match self {
            Category::Images => "Image conversions",
            Category::Video => "Video conversions",
            Category::Audio => "Audio conversions",
            Category::Pdf => "PDF conversions",
            Category::Documents => "Document conversions",
            Category::Slides => "Presentation conversions",
            Category::Spreadsheets => "Spreadsheet conversions",
            Category::Epub => "EPUB conversions",
            Category::Subtitles => "Subtitle conversions",
            Category::Archives => "Archive tools",
        }
    }

    /// Category that best matches a file, used when the wheel opens with files.
    pub fn of(path: &Path) -> Option<Self> {
        let ext = extension(path)?;
        if IMAGE_NATIVE.contains(&ext.as_str()) || IMAGE_MAGICK.contains(&ext.as_str()) {
            Some(Category::Images)
        } else if VIDEO.contains(&ext.as_str()) {
            Some(Category::Video)
        } else if AUDIO.contains(&ext.as_str()) {
            Some(Category::Audio)
        } else if ext == "pdf" {
            Some(Category::Pdf)
        } else if DOCUMENTS.contains(&ext.as_str()) {
            Some(Category::Documents)
        } else if SLIDES.contains(&ext.as_str()) {
            Some(Category::Slides)
        } else if SHEETS.contains(&ext.as_str()) {
            Some(Category::Spreadsheets)
        } else if ext == "epub" {
            Some(Category::Epub)
        } else if SUBTITLES.contains(&ext.as_str()) {
            Some(Category::Subtitles)
        } else if archive_kind(path).is_some() {
            Some(Category::Archives)
        } else {
            None
        }
    }
}

/// External binaries the conversion engine can use.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tools {
    pub magick: bool,
    pub ffmpeg: bool,
    pub libreoffice: bool,
    pub pdftoppm: bool,
    pub pdftotext: bool,
    pub pandoc: bool,
}

impl Tools {
    pub fn detect() -> Self {
        Self {
            magick: command_exists("magick") || command_exists("convert"),
            ffmpeg: command_exists("ffmpeg"),
            libreoffice: command_exists("soffice") || command_exists("libreoffice"),
            pdftoppm: command_exists("pdftoppm"),
            pdftotext: command_exists("pdftotext"),
            pandoc: command_exists("pandoc"),
        }
    }
}

fn command_exists(name: &str) -> bool {
    find_tool(name).is_some()
}

/// Resolves an executable the same way detection does. Desktop launches often
/// omit the sbin directories even though ffmpeg, LibreOffice and ImageMagick
/// live there on Arch-based distributions, so include them as fallbacks.
pub fn find_tool(name: &str) -> Option<std::path::PathBuf> {
    let mut directories: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    for fallback in [
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/local/sbin",
        "/usr/sbin",
        "/sbin",
    ] {
        let path = std::path::PathBuf::from(fallback);
        if !directories.contains(&path) {
            directories.push(path);
        }
    }
    directories
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(candidate: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        candidate.is_file()
            && std::fs::metadata(candidate)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        candidate.is_file()
    }
}

#[derive(Debug, Clone)]
pub struct Target {
    pub id: String,
    pub label: String,
    pub extension: String,
    pub reason: Option<String>,
}

impl Target {
    pub fn is_available(&self) -> bool {
        self.reason.is_none()
    }
}

#[derive(Debug, Clone, Copy)]
struct Spec {
    ext: &'static str,
    label: &'static str,
}

const fn spec(ext: &'static str, label: &'static str) -> Spec {
    Spec { ext, label }
}

struct TargetSet {
    specs: &'static [Spec],
}

const IMAGE_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("png", "PNG"),
        spec("jpg", "JPG"),
        spec("webp", "WEBP"),
        spec("heic", "HEIC"),
        spec("tiff", "TIFF"),
        spec("avif", "AVIF"),
        spec("bmp", "BMP"),
        spec("pdf", "PDF"),
    ],
};
const VIDEO_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("mp4", "MP4"),
        spec("mov", "MOV"),
        spec("mkv", "MKV"),
        spec("webm", "WEBM"),
        spec("avi", "AVI"),
        spec("wmv", "WMV"),
        spec("gif", "GIF"),
    ],
};
const AUDIO_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("mp3", "MP3"),
        spec("m4a", "M4A"),
        spec("wav", "WAV"),
        spec("flac", "FLAC"),
        spec("ogg", "OGG"),
        spec("opus", "OPUS"),
        spec("aiff", "AIFF"),
        spec("wma", "WMA"),
    ],
};
const PDF_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("docx", "DOCX"),
        spec("pptx", "PPTX"),
        spec("xlsx", "XLSX"),
        spec("jpg", "JPG"),
        spec("png", "PNG"),
        spec("epub", "EPUB"),
        spec("txt", "TXT"),
        spec("md", "MD"),
    ],
};
const DOCUMENT_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("pdf", "PDF"),
        spec("docx", "DOCX"),
        spec("odt", "ODT"),
        spec("txt", "TXT"),
        spec("md", "MD"),
        spec("epub", "EPUB"),
        spec("html", "HTML"),
        spec("rtf", "RTF"),
    ],
};
const SLIDE_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("pdf", "PDF"),
        spec("pptx", "PPTX"),
        spec("odp", "ODP"),
        spec("jpg", "JPG"),
        spec("png", "PNG"),
    ],
};
const SHEET_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("pdf", "PDF"),
        spec("xlsx", "XLSX"),
        spec("ods", "ODS"),
        spec("csv", "CSV"),
        spec("tsv", "TSV"),
    ],
};
const EPUB_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("pdf", "PDF"),
        spec("html", "HTML"),
        spec("docx", "DOCX"),
        spec("md", "MD"),
        spec("txt", "TXT"),
    ],
};
const SUBTITLE_TARGETS: TargetSet = TargetSet {
    specs: &[spec("srt", "SRT"), spec("vtt", "VTT"), spec("txt", "TXT")],
};
const ARCHIVE_TARGETS: TargetSet = TargetSet {
    specs: &[
        spec("zip", "ZIP"),
        spec("tar", "TAR"),
        spec("gzip", "GZIP"),
        spec("extract", "EXTRACT"),
    ],
};

fn set_for(category: Category) -> &'static TargetSet {
    match category {
        Category::Images => &IMAGE_TARGETS,
        Category::Video => &VIDEO_TARGETS,
        Category::Audio => &AUDIO_TARGETS,
        Category::Pdf => &PDF_TARGETS,
        Category::Documents => &DOCUMENT_TARGETS,
        Category::Slides => &SLIDE_TARGETS,
        Category::Spreadsheets => &SHEET_TARGETS,
        Category::Epub => &EPUB_TARGETS,
        Category::Subtitles => &SUBTITLE_TARGETS,
        Category::Archives => &ARCHIVE_TARGETS,
    }
}

pub const IMAGE_NATIVE: [&str; 15] = [
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "qoi", "tga", "pnm", "ppm",
    "pgm", "pbm",
];
pub const IMAGE_MAGICK: [&str; 4] = ["heic", "heif", "avif", "svg"];
pub const VIDEO: [&str; 9] = [
    "mp4", "mov", "mkv", "webm", "avi", "wmv", "m4v", "mpg", "mpeg",
];
pub const AUDIO: [&str; 9] = [
    "mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "aif", "wma",
];
pub const DOCUMENTS: [&str; 8] = ["docx", "doc", "odt", "rtf", "txt", "md", "html", "htm"];
pub const SLIDES: [&str; 3] = ["pptx", "ppt", "odp"];
pub const SHEETS: [&str; 5] = ["xlsx", "xls", "ods", "csv", "tsv"];
pub const SUBTITLES: [&str; 2] = ["srt", "vtt"];

pub fn extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
}

/// True for `.tar.gz` and `.tgz` as well as plain extensions.
pub fn archive_kind(path: &Path) -> Option<ArchiveKind> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ArchiveKind::TarGz)
    } else if name.ends_with(".tar") {
        Some(ArchiveKind::Tar)
    } else if name.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if name.ends_with(".gz") {
        Some(ArchiveKind::Gzip)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    Gzip,
}

/// Computes the targets shown in the wheel for `category`.
///
/// `source` is the first selected file; the wheel reflects what can be done to
/// it. When it is `None` the wheel is empty and the UI shows the drop prompt.
pub fn targets(category: Category, source: Option<&Path>, tools: &Tools) -> Vec<Target> {
    let source_ext = source.and_then(extension);
    let source_is_dir = source.is_some_and(|path| path.is_dir());
    let source_archive = source.and_then(archive_kind);

    set_for(category)
        .specs
        .iter()
        .map(|spec| {
            let reason = availability(
                category,
                spec.ext,
                source,
                source_ext.as_deref(),
                source_is_dir,
                source_archive,
                tools,
            );
            Target {
                id: spec.ext.to_owned(),
                label: spec.label.to_owned(),
                extension: spec.ext.to_owned(),
                reason,
            }
        })
        .collect()
}

fn availability(
    category: Category,
    target: &str,
    _source: Option<&Path>,
    ext: Option<&str>,
    source_is_dir: bool,
    source_archive: Option<ArchiveKind>,
    tools: &Tools,
) -> Option<String> {
    if category == Category::Archives {
        return archive_availability(target, source_is_dir, source_archive);
    }

    let Some(ext) = ext else {
        return Some("Drop a file first".into());
    };

    match category {
        Category::Images => {
            let native_source = IMAGE_NATIVE.contains(&ext);
            let magick_source = IMAGE_MAGICK.contains(&ext);
            if !native_source && !magick_source {
                return Some("Not an image file".into());
            }
            if magick_source && !tools.magick {
                return Some("Needs ImageMagick for this format".into());
            }
            match target {
                "heic" | "avif" | "pdf" if !tools.magick => {
                    return Some("Needs ImageMagick".into());
                }
                _ => {}
            }
            None
        }
        Category::Video => {
            if !VIDEO.contains(&ext) {
                return Some("Not a video file".into());
            }
            (!tools.ffmpeg).then(|| "Needs ffmpeg".into())
        }
        Category::Audio => {
            if !AUDIO.contains(&ext) {
                return Some("Not an audio file".into());
            }
            (!tools.ffmpeg).then(|| "Needs ffmpeg".into())
        }
        Category::Pdf => {
            if ext != "pdf" {
                return Some("Drop a PDF first".into());
            }
            match target {
                "jpg" | "png" => (!tools.pdftoppm).then(|| "Needs pdftoppm (poppler)".into()),
                "txt" | "md" => (!tools.pdftotext).then(|| "Needs pdftotext (poppler)".into()),
                _ => Some("Needs OCR, which is not bundled".into()),
            }
        }
        Category::Documents => {
            if !DOCUMENTS.contains(&ext) {
                return Some("Not a document file".into());
            }
            if target == "md" {
                return (!tools.pandoc).then(|| "Needs pandoc".into());
            }
            if ext == "md" {
                return (!tools.pandoc).then(|| "Needs pandoc to read Markdown".into());
            }
            (!tools.libreoffice).then(|| "Needs LibreOffice".into())
        }
        Category::Slides => {
            if !SLIDES.contains(&ext) {
                return Some("Not a presentation file".into());
            }
            if target == "jpg" || target == "png" {
                if !tools.libreoffice || !tools.pdftoppm {
                    return Some("Needs LibreOffice + pdftoppm".into());
                }
            } else if !tools.libreoffice {
                return Some("Needs LibreOffice".into());
            }
            None
        }
        Category::Spreadsheets => {
            if !SHEETS.contains(&ext) {
                return Some("Not a spreadsheet file".into());
            }
            (!tools.libreoffice).then(|| "Needs LibreOffice".into())
        }
        Category::Epub => {
            if ext != "epub" {
                return Some("Drop an EPUB first".into());
            }
            if target == "md" {
                return (!tools.pandoc).then(|| "Needs pandoc".into());
            }
            (!tools.libreoffice).then(|| "Needs LibreOffice".into())
        }
        Category::Subtitles => {
            if !SUBTITLES.contains(&ext) {
                return Some("Not a subtitle file".into());
            }
            if target == "txt" {
                None
            } else {
                (!tools.ffmpeg).then(|| "Needs ffmpeg".into())
            }
        }
        Category::Archives => archive_availability(target, source_is_dir, source_archive),
    }
}

fn archive_availability(
    target: &str,
    source_is_dir: bool,
    source_archive: Option<ArchiveKind>,
) -> Option<String> {
    if source_is_dir {
        return (target == "extract").then(|| "Folders cannot be extracted".into());
    }
    if let Some(kind) = source_archive {
        let same = matches!(
            (kind, target),
            (ArchiveKind::Zip, "zip")
                | (ArchiveKind::Tar, "tar")
                | (ArchiveKind::TarGz, "gzip")
                | (ArchiveKind::Gzip, "gzip")
        );
        return same.then(|| "Already this format".into());
    }
    (target == "extract").then(|| "Select an archive to extract".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tools_all() -> Tools {
        Tools {
            magick: true,
            ffmpeg: true,
            libreoffice: true,
            pdftoppm: true,
            pdftotext: true,
            pandoc: true,
        }
    }

    #[test]
    fn detects_categories_by_extension() {
        assert_eq!(Category::of(Path::new("a.PNG")), Some(Category::Images));
        assert_eq!(
            Category::of(Path::new("a.tar.gz")),
            Some(Category::Archives)
        );
        assert_eq!(Category::of(Path::new("a.unknown")), None);
    }

    #[test]
    fn image_targets_require_a_source() {
        let list = targets(Category::Images, None, &tools_all());
        assert!(list.iter().all(|target| !target.is_available()));
    }

    #[test]
    fn missing_tools_disable_targets_with_reasons() {
        let tools = Tools::default();
        let png = PathBuf::from("/tmp/x.png");
        let list = targets(Category::Images, Some(&png), &tools);
        let pdf = list.iter().find(|t| t.id == "pdf").unwrap();
        assert!(!pdf.is_available());
        assert!(pdf.reason.as_deref().unwrap().contains("ImageMagick"));
        let webp = list.iter().find(|t| t.id == "webp").unwrap();
        assert!(webp.is_available());
    }

    #[test]
    fn pdf_targets_follow_source_kind() {
        let tools = tools_all();
        let png = PathBuf::from("/tmp/x.png");
        let list = targets(Category::Pdf, Some(&png), &tools);
        assert!(list.iter().all(|t| !t.is_available()));

        let pdf = PathBuf::from("/tmp/x.pdf");
        let list = targets(Category::Pdf, Some(&pdf), &tools);
        assert!(list.iter().find(|t| t.id == "png").unwrap().is_available());
        assert!(!list.iter().find(|t| t.id == "docx").unwrap().is_available());
    }

    #[test]
    fn archive_folder_targets_match_implemented_actions() {
        let tools = tools_all();
        let dir = tempfile::tempdir().unwrap();
        let list = targets(Category::Archives, Some(dir.path()), &tools);
        let by_id = |id: &str| list.iter().find(|target| target.id == id).unwrap();
        assert!(by_id("zip").is_available());
        assert!(by_id("tar").is_available());
        assert!(by_id("gzip").is_available());
        assert!(!by_id("extract").is_available());
    }

    #[test]
    fn archive_extract_needs_an_archive() {
        let tools = tools_all();
        let zip = PathBuf::from("/tmp/x.zip");
        let list = targets(Category::Archives, Some(&zip), &tools);
        assert!(list
            .iter()
            .find(|t| t.id == "extract")
            .unwrap()
            .is_available());

        let png = PathBuf::from("/tmp/x.png");
        let list = targets(Category::Archives, Some(&png), &tools);
        assert!(!list
            .iter()
            .find(|t| t.id == "extract")
            .unwrap()
            .is_available());
    }
}
