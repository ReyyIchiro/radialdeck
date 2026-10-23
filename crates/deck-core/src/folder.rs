use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// A saved folder shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub label: String,
    pub path: PathBuf,
    #[serde(default)]
    pub color: Option<String>,
}

impl Folder {
    pub fn new(path: PathBuf, label: impl Into<String>, color: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            path,
            color,
        }
    }
}

/// A named collection of folder shortcuts shown as one wheel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub folders: Vec<Folder>,
}

impl Project {
    pub fn new(name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            color: normalize_color(&color.into()).unwrap_or_else(|_| "#3b82f6".into()),
            folders: Vec::new(),
        }
    }

    pub fn contains_path(&self, path: &Path) -> bool {
        let candidate = comparable_path(path);
        self.folders
            .iter()
            .any(|folder| comparable_path(&folder.path) == candidate)
    }

    /// Adds a validated folder at the end of the wheel.
    pub fn add_folder(
        &mut self,
        path: &Path,
        label: Option<&str>,
        color: Option<String>,
    ) -> Result<Uuid, FolderError> {
        if !path.is_absolute() {
            return Err(FolderError::NotAbsolute);
        }
        if !path.exists() {
            return Err(FolderError::NotFound);
        }
        if !path.is_dir() {
            return Err(FolderError::NotDirectory);
        }
        if self.contains_path(path) {
            return Err(FolderError::Duplicate);
        }

        let fallback = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| path.display().to_string());
        let label = label
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(&fallback)
            .to_owned();

        let folder = Folder::new(path.to_path_buf(), label, color);
        let id = folder.id;
        self.folders.push(folder);
        Ok(id)
    }

    pub fn remove_folder(&mut self, id: Uuid) -> bool {
        let before = self.folders.len();
        self.folders.retain(|folder| folder.id != id);
        self.folders.len() != before
    }

    pub fn move_folder(&mut self, id: Uuid, offset: isize) -> bool {
        let Some(index) = self.folders.iter().position(|folder| folder.id == id) else {
            return false;
        };
        let target = index as isize + offset;
        if target < 0 || target >= self.folders.len() as isize {
            return false;
        }
        self.folders.swap(index, target as usize);
        true
    }
}

/// Whether a shortcut can currently be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Available,
    /// The path itself is gone but its parent still exists.
    Missing,
    /// An ancestor directory is gone, which usually means a drive is not mounted.
    Offline,
}

pub fn availability(path: &Path) -> Availability {
    if path.is_dir() {
        return Availability::Available;
    }
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() || parent.is_dir() => Availability::Missing,
        _ => Availability::Offline,
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FolderError {
    #[error("path must be absolute")]
    NotAbsolute,
    #[error("path does not exist")]
    NotFound,
    #[error("path is not a directory")]
    NotDirectory,
    #[error("folder is already in this project")]
    Duplicate,
}

/// Lexical comparison key. Canonicalisation is attempted so symlinked paths
/// do not produce duplicates, but missing paths still compare as written.
pub fn comparable_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Accepts `#rrggbb` / `#rrggbbaa` and returns lowercase `#rrggbb`.
pub fn normalize_color(value: &str) -> Result<String, ColorError> {
    let hex = value.trim().trim_start_matches('#');
    let expanded = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 | 8 => hex.to_owned(),
        _ => return Err(ColorError),
    };
    if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ColorError);
    }
    Ok(format!("#{}", expanded[..6].to_ascii_lowercase()))
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid hex color")]
pub struct ColorError;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn add_folder_validates_and_derives_label() {
        let dir = tempdir().unwrap();
        let folder = dir.path().join("My Project");
        std::fs::create_dir(&folder).unwrap();

        let mut project = Project::new("Work", "#3B82F6");
        let id = project.add_folder(&folder, None, None).unwrap();
        assert_eq!(project.folders[0].id, id);
        assert_eq!(project.folders[0].label, "My Project");
        assert_eq!(
            project.add_folder(&folder, Some("again"), None),
            Err(FolderError::Duplicate)
        );
    }

    #[test]
    fn add_folder_rejects_relative_and_files() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "x").unwrap();

        let mut project = Project::new("Work", "#3B82F6");
        assert_eq!(
            project.add_folder(Path::new("relative"), None, None),
            Err(FolderError::NotAbsolute)
        );
        assert_eq!(
            project.add_folder(&file, None, None),
            Err(FolderError::NotDirectory)
        );
        assert_eq!(
            project.add_folder(&dir.path().join("missing"), None, None),
            Err(FolderError::NotFound)
        );
    }

    #[test]
    fn availability_distinguishes_missing_from_offline() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(availability(&nested), Availability::Available);
        assert_eq!(
            availability(&dir.path().join("gone")),
            Availability::Missing
        );
        let offline = dir
            .path()
            .join("not-mounted")
            .join("project")
            .join("deeper");
        assert_eq!(availability(&offline), Availability::Offline);
    }

    #[test]
    fn colors_are_normalised() {
        assert_eq!(normalize_color("#ABC").unwrap(), "#aabbcc");
        assert_eq!(normalize_color("3B82F6").unwrap(), "#3b82f6");
        assert_eq!(normalize_color("#3b82f6ff").unwrap(), "#3b82f6");
        assert!(normalize_color("nope").is_err());
    }

    #[test]
    fn moving_folders_is_bounded() {
        let dir = tempdir().unwrap();
        let mut project = Project::new("Work", "#3b82f6");
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        let id_a = project.add_folder(&a, None, None).unwrap();
        let id_b = project.add_folder(&b, None, None).unwrap();

        assert!(project.move_folder(id_b, -1));
        assert_eq!(project.folders[0].id, id_b);
        assert!(!project.move_folder(id_b, -1));
        assert!(project.remove_folder(id_a));
        assert_eq!(project.folders.len(), 1);
    }
}
