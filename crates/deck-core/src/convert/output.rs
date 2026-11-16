//! Collision-free output naming.

use std::path::{Path, PathBuf};

/// Returns `dir/stem.ext`, or `dir/stem` when `extension` is empty, adding
/// `-N` when the name is taken. Existing files are never overwritten.
pub fn unique_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let stem = sanitize(stem);
    let extension = extension.trim_start_matches('.');
    let mut candidate = if extension.is_empty() {
        dir.join(&stem)
    } else {
        dir.join(format!("{stem}.{extension}"))
    };
    let mut counter = 1;
    loop {
        if !candidate.exists() {
            break;
        }
        candidate = if extension.is_empty() {
            dir.join(format!("{stem}-{counter}"))
        } else {
            dir.join(format!("{stem}-{counter}.{extension}"))
        };
        counter += 1;
    }
    crate::job::register(&candidate);
    candidate
}

/// Claims `path` exclusively: the file is created only when absent, closing
/// the window between choosing a collision-free name and writing it. A losing
/// racer gets an error instead of silently truncating another operation's
/// output, which keeps the never-overwrite promise honest under concurrency.
pub fn create_exclusive(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// Returns a path in `dir` named `name`, disambiguated if necessary.
pub fn unique_named(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        crate::job::register(&candidate);
        return candidate;
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into());
    let extension = path
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if extension.is_empty() {
        unique_path(dir, &stem, "out")
    } else {
        unique_path(dir, &stem, &extension)
    }
}

/// Strips path separators and emptiness from a user-derived file stem.
pub(crate) fn sanitize(stem: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avoids_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("photo.png"), b"x").unwrap();
        let path = unique_path(dir.path(), "photo", "png");
        assert_eq!(path.file_name().unwrap(), "photo-1.png");

        std::fs::write(&path, b"x").unwrap();
        let path = unique_path(dir.path(), "photo", "png");
        assert_eq!(path.file_name().unwrap(), "photo-2.png");
    }

    #[test]
    fn sanitizes_path_separators() {
        let dir = tempfile::tempdir().unwrap();
        let path = unique_path(dir.path(), "a/b", "txt");
        assert_eq!(path.file_name().unwrap(), "a_b.txt");
    }
}
