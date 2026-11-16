//! Single source of truth for external binary names.
//!
//! Tool requirements, dependency detection and install hints all name the
//! same executables; before this module the names lived as bare strings in
//! three tables and had already drifted once (`CreateCollage` required
//! `montage` while its availability check looked at `magick`). New binaries
//! are added here and referenced everywhere else.

pub const CONVERT: &str = "convert";
pub const FFMPEG: &str = "ffmpeg";
pub const FFPROBE: &str = "ffprobe";
pub const GHOSTSCRIPT: &str = "gs";
pub const IMAGEMAGICK: &str = "magick";
pub const LIBREOFFICE: &str = "libreoffice";
pub const MONTAGE: &str = "montage";
pub const PANDOC: &str = "pandoc";
pub const PDFSEPARATE: &str = "pdfseparate";
pub const PDFTOPPM: &str = "pdftoppm";
pub const PDFTOTEXT: &str = "pdftotext";
pub const PDFUNITE: &str = "pdfunite";
pub const SEVENZIP: &str = "7z";
pub const SEVENZIP_ALT: &str = "7zz";
pub const SOFFICE: &str = "soffice";

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name must be a plain executable name: no paths, no flags, no
    /// whitespace. These strings become `Command::new` arguments.
    #[test]
    fn names_are_plain_executables() {
        for name in [
            CONVERT,
            FFMPEG,
            FFPROBE,
            GHOSTSCRIPT,
            IMAGEMAGICK,
            LIBREOFFICE,
            MONTAGE,
            PANDOC,
            PDFSEPARATE,
            PDFTOPPM,
            PDFTOTEXT,
            PDFUNITE,
            SEVENZIP,
            SEVENZIP_ALT,
            SOFFICE,
        ] {
            assert!(!name.is_empty(), "{name}");
            assert!(
                !name.contains('/') && !name.contains(' ') && !name.contains('\0'),
                "{name}"
            );
            assert!(!name.starts_with('-'), "{name}");
        }
    }
}
