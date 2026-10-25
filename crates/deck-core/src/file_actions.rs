//! Filesystem actions shared by every frontend. Clipboard and tray integration
//! stay in the platform layer; moving and path calculations are portable.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum FileActionError {
    #[error("select at least one file")]
    Empty,
    #[error("the destination does not exist or is not a folder")]
    InvalidDestination,
    #[error("{0} already exists in the destination")]
    Exists(PathBuf),
    #[error("cannot move a folder into itself")]
    IntoItself,
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
}

/// The deepest folder that contains every selected path. For a single folder
/// the folder itself is returned; for files, their shared parent.
pub fn common_parent(paths: &[PathBuf]) -> Option<PathBuf> {
    let first = paths.first()?;
    let mut parent = if first.is_dir() {
        first.clone()
    } else {
        first.parent()?.to_path_buf()
    };
    for path in &paths[1..] {
        let candidate = if path.is_dir() {
            path.clone()
        } else {
            path.parent()?.to_path_buf()
        };
        while !candidate.starts_with(&parent) {
            if !parent.pop() {
                return None;
            }
        }
    }
    Some(parent)
}

/// Moves files or folders without overwriting anything. Cross-device moves
/// fall back to copy-then-delete.
pub fn move_files(
    sources: &[PathBuf],
    destination: &Path,
) -> Result<Vec<PathBuf>, FileActionError> {
    if sources.is_empty() {
        return Err(FileActionError::Empty);
    }
    if !destination.is_dir() {
        return Err(FileActionError::InvalidDestination);
    }
    for source in sources {
        let Some(name) = source.file_name() else {
            return Err(FileActionError::Io(
                source.clone(),
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing file name"),
            ));
        };
        let target = destination.join(name);
        if source == &target || target.starts_with(source) {
            return Err(FileActionError::IntoItself);
        }
        if target.exists() {
            return Err(FileActionError::Exists(target));
        }
    }

    let mut moved = Vec::new();
    for source in sources {
        let name = source.file_name().expect("validated above");
        let target = destination.join(name);
        if fs::rename(source, &target).is_ok() {
            moved.push(target);
            continue;
        }
        if source.is_dir() {
            copy_dir(source, &target)
                .map_err(|error| FileActionError::Io(source.clone(), error))?;
            fs::remove_dir_all(source)
                .map_err(|error| FileActionError::Io(source.clone(), error))?;
        } else {
            fs::copy(source, &target)
                .map_err(|error| FileActionError::Io(source.clone(), error))?;
            fs::remove_file(source).map_err(|error| FileActionError::Io(source.clone(), error))?;
        }
        moved.push(target);
    }
    Ok(moved)
}

fn copy_dir(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_files_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let destination = dir.path().join("dest");
        fs::write(&source, b"hello").unwrap();
        fs::create_dir(&destination).unwrap();
        let moved = move_files(std::slice::from_ref(&source), &destination).unwrap();
        assert_eq!(moved, vec![destination.join("a.txt")]);
        assert!(!source.exists());
    }

    #[test]
    fn refuses_an_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let destination = dir.path().join("dest");
        fs::write(&source, b"hello").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("a.txt"), b"old").unwrap();
        assert!(matches!(
            move_files(&[source], &destination),
            Err(FileActionError::Exists(_))
        ));
    }

    #[test]
    fn common_parent_walks_up() {
        let paths = vec![
            PathBuf::from("/tmp/one/a.jpg"),
            PathBuf::from("/tmp/two/b.jpg"),
        ];
        assert_eq!(common_parent(&paths), Some(PathBuf::from("/tmp")));
    }
}
