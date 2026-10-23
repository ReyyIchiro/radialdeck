//! Archive creation and extraction (ZIP, TAR, TAR.GZ).

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::output::unique_path;
use super::registry::{archive_kind, ArchiveKind};
use super::ConvertError;

pub fn compress(source: &Path, extension: &str) -> Result<PathBuf, ConvertError> {
    // GZIP means a plain .gz for a single file, and a gzipped tarball when a
    // directory or another archive has to be repacked.
    let kind = match extension {
        "zip" => ArchiveKind::Zip,
        "tar" => ArchiveKind::Tar,
        "gzip" => {
            if source.is_dir() || archive_kind(source).is_some_and(|kind| kind != ArchiveKind::Gzip)
            {
                ArchiveKind::TarGz
            } else {
                ArchiveKind::Gzip
            }
        }
        other => return Err(ConvertError(format!("unsupported archive target: {other}"))),
    };
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = stem_without_archive_ext(source);

    // Converting an existing archive means unpacking it first, then repacking
    // its contents without an extra wrapper directory.
    let destination = unique_path(parent, &stem, archive_extension(kind));
    if kind == ArchiveKind::Gzip {
        if source.is_dir() || archive_kind(source) == Some(ArchiveKind::Gzip) {
            return Err(ConvertError("source is already gzip-compressed".into()));
        }
        write_gzip_file(source, &destination)?;
        return Ok(destination);
    }
    if !source.is_dir() && archive_kind(source).is_some() {
        let temp = tempfile::tempdir()
            .map_err(|error| ConvertError(format!("create temp dir: {error}")))?;
        extract_into(source, temp.path())?;
        match kind {
            ArchiveKind::Zip => write_zip(temp.path(), None, &destination)?,
            ArchiveKind::Tar => write_tar_children(temp.path(), &destination, false)?,
            ArchiveKind::TarGz => write_tar_children(temp.path(), &destination, true)?,
            ArchiveKind::Gzip => {
                return Err(ConvertError("gzip needs a single file".into()));
            }
        }
        return Ok(destination);
    }

    let base_name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| stem.clone());
    archive_directory(source, Some(&base_name), &destination, kind)?;
    Ok(destination)
}

pub fn extract(source: &Path) -> Result<PathBuf, ConvertError> {
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = stem_without_archive_ext(source);
    let mut destination = parent.join(&stem);
    let mut counter = 1;
    while destination.exists() {
        destination = parent.join(format!("{stem}-{counter}"));
        counter += 1;
    }
    fs::create_dir_all(&destination)
        .map_err(|error| ConvertError(format!("create {}: {error}", destination.display())))?;
    extract_into(source, &destination)?;
    Ok(destination)
}

fn write_gzip_file(source: &Path, destination: &Path) -> Result<(), ConvertError> {
    let mut input = File::open(source)
        .map_err(|error| ConvertError(format!("open {}: {error}", source.display())))?;
    let output = File::create(destination)
        .map_err(|error| ConvertError(format!("create {}: {error}", destination.display())))?;
    let mut encoder = GzEncoder::new(output, Compression::default());
    std::io::copy(&mut input, &mut encoder)
        .map_err(|error| ConvertError(format!("compress: {error}")))?;
    encoder
        .finish()
        .map_err(|error| ConvertError(format!("finish gzip: {error}")))?;
    Ok(())
}

fn archive_extension(kind: ArchiveKind) -> &'static str {
    match kind {
        ArchiveKind::Zip => "zip",
        ArchiveKind::Tar => "tar",
        ArchiveKind::TarGz => "tar.gz",
        ArchiveKind::Gzip => "gz",
    }
}

fn stem_without_archive_ext(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    for suffix in [".tar.gz", ".tgz", ".tar", ".zip", ".gz"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "archive".into())
}

/// Archives `root`. When `root` is a file, `entry_name` is its name inside the
/// archive; when it is a directory, the directory is stored under `entry_name`.
fn archive_directory(
    root: &Path,
    entry_name: Option<&str>,
    destination: &Path,
    kind: ArchiveKind,
) -> Result<(), ConvertError> {
    match kind {
        ArchiveKind::Zip => write_zip(root, entry_name, destination),
        ArchiveKind::Tar => write_tar(root, entry_name, destination, false),
        ArchiveKind::TarGz => write_tar(root, entry_name, destination, true),
        ArchiveKind::Gzip => Err(ConvertError("gzip needs a single file".into())),
    }
}

fn write_zip(
    root: &Path,
    entry_name: Option<&str>,
    destination: &Path,
) -> Result<(), ConvertError> {
    let file = File::create(destination)
        .map_err(|error| ConvertError(format!("create {}: {error}", destination.display())))?;
    let mut zip = ZipWriter::new(file);
    let file_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let dir_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o755);

    if root.is_file() {
        let name = entry_name
            .map(str::to_owned)
            .or_else(|| {
                root.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "file".into());
        zip.start_file(&name, file_options)
            .map_err(|error| ConvertError(format!("add {name}: {error}")))?;
        copy_into(&mut zip, root)?;
    } else {
        let prefix = entry_name.map(PathBuf::from);
        if let Some(prefix) = &prefix {
            zip.add_directory(format!("{}/", prefix.to_string_lossy()), dir_options)
                .map_err(|error| ConvertError(format!("add directory: {error}")))?;
        }
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path == root {
                continue;
            }
            if entry.file_type().is_symlink() {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(path);
            let name = match &prefix {
                Some(prefix) => prefix.join(relative),
                None => relative.to_path_buf(),
            };
            let name = name.to_string_lossy().replace('\\', "/");
            if entry.file_type().is_dir() {
                zip.add_directory(format!("{name}/"), dir_options)
                    .map_err(|error| ConvertError(format!("add {name}: {error}")))?;
            } else {
                zip.start_file(&name, file_options)
                    .map_err(|error| ConvertError(format!("add {name}: {error}")))?;
                copy_into(&mut zip, path)?;
            }
        }
    }

    zip.finish()
        .map_err(|error| ConvertError(format!("finish archive: {error}")))?;
    Ok(())
}

fn copy_into(writer: &mut impl Write, source: &Path) -> Result<(), ConvertError> {
    let mut input = File::open(source)
        .map_err(|error| ConvertError(format!("open {}: {error}", source.display())))?;
    std::io::copy(&mut input, writer)
        .map_err(|error| ConvertError(format!("write archive: {error}")))?;
    Ok(())
}

/// Stores the immediate children of `directory` at the archive root, which is
/// what converting an existing archive is expected to do.
fn write_tar_children(
    directory: &Path,
    destination: &Path,
    gzip: bool,
) -> Result<(), ConvertError> {
    let file = File::create(destination)
        .map_err(|error| ConvertError(format!("create {}: {error}", destination.display())))?;
    if gzip {
        let encoder = GzEncoder::new(file, Compression::default());
        write_children_inner(directory, encoder)?;
    } else {
        write_children_inner(directory, file)?;
    }
    Ok(())
}

fn write_children_inner<W: Write>(directory: &Path, writer: W) -> Result<(), ConvertError> {
    let mut builder = tar::Builder::new(writer);
    builder.follow_symlinks(false);
    let entries = fs::read_dir(directory)
        .map_err(|error| ConvertError(format!("read {}: {error}", directory.display())))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = PathBuf::from(entry.file_name());
        if path.is_dir() {
            builder
                .append_dir_all(&name, &path)
                .map_err(|error| ConvertError(format!("add directory: {error}")))?;
        } else {
            builder
                .append_path_with_name(&path, &name)
                .map_err(|error| ConvertError(format!("add file: {error}")))?;
        }
    }
    let mut inner = builder
        .into_inner()
        .map_err(|error| ConvertError(format!("finish archive: {error}")))?;
    inner
        .flush()
        .map_err(|error| ConvertError(format!("finish archive: {error}")))?;
    Ok(())
}

fn write_tar(
    root: &Path,
    entry_name: Option<&str>,
    destination: &Path,
    gzip: bool,
) -> Result<(), ConvertError> {
    let file = File::create(destination)
        .map_err(|error| ConvertError(format!("create {}: {error}", destination.display())))?;
    if gzip {
        let encoder = GzEncoder::new(file, Compression::default());
        write_tar_inner(root, entry_name, encoder)?;
    } else {
        write_tar_inner(root, entry_name, file)?;
    }
    Ok(())
}

fn write_tar_inner<W: Write>(
    root: &Path,
    entry_name: Option<&str>,
    writer: W,
) -> Result<(), ConvertError> {
    let mut builder = tar::Builder::new(writer);
    builder.follow_symlinks(false);

    if root.is_dir() {
        let base = entry_name
            .map(PathBuf::from)
            .or_else(|| root.file_name().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("archive"));
        builder
            .append_dir_all(&base, root)
            .map_err(|error| ConvertError(format!("add directory: {error}")))?;
    } else {
        let name = entry_name
            .map(PathBuf::from)
            .or_else(|| root.file_name().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("file"));
        builder
            .append_path_with_name(root, name)
            .map_err(|error| ConvertError(format!("add file: {error}")))?;
    }

    let mut inner = builder
        .into_inner()
        .map_err(|error| ConvertError(format!("finish archive: {error}")))?;
    inner
        .flush()
        .map_err(|error| ConvertError(format!("finish archive: {error}")))?;
    Ok(())
}

fn extract_into(source: &Path, destination: &Path) -> Result<(), ConvertError> {
    match archive_kind(source) {
        Some(ArchiveKind::Zip) => extract_zip(source, destination),
        Some(ArchiveKind::Tar) => extract_tar(source, destination, false),
        Some(ArchiveKind::TarGz) => extract_tar(source, destination, true),
        Some(ArchiveKind::Gzip) => extract_gzip(source, destination),
        None => Err(ConvertError(format!(
            "not an archive: {}",
            source.display()
        ))),
    }
}

fn extract_zip(source: &Path, destination: &Path) -> Result<(), ConvertError> {
    let file = File::open(source)
        .map_err(|error| ConvertError(format!("open {}: {error}", source.display())))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| ConvertError(format!("open zip: {error}")))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ConvertError(format!("read zip entry: {error}")))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| ConvertError(format!("create {}: {error}", target.display())))?;
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            continue; // symlinks can escape the destination
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| ConvertError(format!("create {}: {error}", parent.display())))?;
        }
        let mut output = File::create(&target)
            .map_err(|error| ConvertError(format!("create {}: {error}", target.display())))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| ConvertError(format!("extract {}: {error}", target.display())))?;
    }
    Ok(())
}

fn extract_tar(source: &Path, destination: &Path, gzip: bool) -> Result<(), ConvertError> {
    let file = File::open(source)
        .map_err(|error| ConvertError(format!("open {}: {error}", source.display())))?;
    let reader: Box<dyn Read> = if gzip {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut archive = tar::Archive::new(reader);
    archive.set_preserve_permissions(false);
    let entries = archive
        .entries()
        .map_err(|error| ConvertError(format!("read tar: {error}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| ConvertError(format!("read tar entry: {error}")))?;
        let path = entry
            .path()
            .map_err(|error| ConvertError(format!("read tar path: {error}")))?
            .into_owned();
        if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            continue;
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            continue; // skip links, devices and other special entries
        }
        entry
            .unpack_in(destination)
            .map_err(|error| ConvertError(format!("extract {}: {error}", path.display())))?;
    }
    Ok(())
}

fn extract_gzip(source: &Path, destination: &Path) -> Result<(), ConvertError> {
    let file = File::open(source)
        .map_err(|error| ConvertError(format!("open {}: {error}", source.display())))?;
    let mut decoder = GzDecoder::new(file);
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = name.strip_suffix(".gz").unwrap_or(&name);
    let target = destination.join(stem);
    let mut output = File::create(&target)
        .map_err(|error| ConvertError(format!("create {}: {error}", target.display())))?;
    std::io::copy(&mut decoder, &mut output)
        .map_err(|error| ConvertError(format!("decompress: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_roundtrip_and_convert_to_tar() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("notes");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("a.txt"), "alpha").unwrap();
        fs::write(source.join("b.txt"), "beta").unwrap();

        let zip = compress(&source, "zip").unwrap();
        assert_eq!(zip.file_name().unwrap(), "notes.zip");

        let tar = compress(&zip, "tar").unwrap();
        assert_eq!(tar.file_name().unwrap(), "notes.tar");

        let out = extract(&tar).unwrap();
        assert_eq!(
            fs::read_to_string(out.join("notes/a.txt")).unwrap(),
            "alpha"
        );
        assert_eq!(fs::read_to_string(out.join("notes/b.txt")).unwrap(), "beta");
    }

    #[test]
    fn gzip_roundtrip_for_a_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("note.txt");
        fs::write(&source, "hello gzip").unwrap();

        let gz = compress(&source, "gzip").unwrap();
        assert_eq!(gz.file_name().unwrap(), "note.gz");

        let out = extract(&gz).unwrap();
        assert_eq!(fs::read_to_string(out.join("note")).unwrap(), "hello gzip");
    }

    #[test]
    fn rejects_zip_slip_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("evil.zip");
        {
            let file = File::create(&archive_path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("../escape.txt", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"nope").unwrap();
            zip.finish().unwrap();
        }
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();
        extract_into(&archive_path, &out).unwrap();
        assert!(!dir.path().join("escape.txt").exists());
        assert!(!out.join("../escape.txt").exists());
    }
}
