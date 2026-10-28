//! A short, on-disk log of outputs this app created.
//!
//! Only paths returned by `unique_path`/`unique_dir` are recorded, so every
//! entry names a file or folder that did not exist before the operation. Undo
//! therefore means "delete what we created", never "restore an overwritten
//! file" — nothing is ever overwritten in the first place.
//!
//! The log lives beside the logs rather than in the configuration: it changes
//! on every operation and should not churn the config file or its backups.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

/// Newest entries kept. Older ones are dropped; outputs stay on disk.
pub const MAX_ENTRIES: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: u64,
    /// Human label of the operation, e.g. "Compress" or "PNG".
    pub label: String,
    /// Files that were selected.
    #[serde(default)]
    pub sources: Vec<PathBuf>,
    /// Files and folders this operation created.
    #[serde(default)]
    pub created: Vec<PathBuf>,
    /// Unix seconds.
    pub timestamp: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub entries: Vec<HistoryEntry>,
    /// Monotonic counter; kept out of the file so ids stay unique after a
    /// restart without trusting the wall clock.
    #[serde(skip)]
    next_id: u64,
}

#[derive(Debug, Default)]
pub struct UndoReport {
    pub removed: Vec<PathBuf>,
    pub missing: Vec<PathBuf>,
    pub failures: Vec<(PathBuf, String)>,
}

impl UndoReport {
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }
}

pub fn default_history_path() -> PathBuf {
    let base = BaseDirs::new();
    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            base.as_ref()
                .and_then(|dirs| dirs.state_dir().map(Path::to_path_buf))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    state_dir.join("radial-deck").join("history.json")
}

impl History {
    /// Reads the log. A missing or corrupt file yields an empty log: history
    /// is a convenience, never a source of truth, so it must not block startup.
    pub fn load(path: &Path) -> Self {
        let Ok(bytes) = fs::read(path) else {
            return Self::default();
        };
        let mut history: Self = serde_json::from_slice(&bytes).unwrap_or_default();
        history.entries.retain(|entry| !entry.created.is_empty());
        history.entries.truncate(MAX_ENTRIES);
        history.next_id = history
            .entries
            .iter()
            .map(|entry| entry.id + 1)
            .max()
            .unwrap_or(1);
        history
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, path)
    }

    /// Adds an entry at the front. Empty output lists are ignored so failed
    /// operations never leave an undo item that does nothing.
    pub fn record(
        &mut self,
        label: impl Into<String>,
        sources: Vec<PathBuf>,
        created: Vec<PathBuf>,
    ) -> Option<u64> {
        if created.is_empty() {
            return None;
        }
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.entries.insert(
            0,
            HistoryEntry {
                id,
                label: label.into(),
                sources,
                created,
                timestamp: now(),
            },
        );
        self.entries.truncate(MAX_ENTRIES);
        Some(id)
    }

    pub fn recent(&self, count: usize) -> &[HistoryEntry] {
        let count = count.min(self.entries.len());
        &self.entries[..count]
    }

    /// Removes the entry and deletes every output it created. Missing outputs
    /// are reported but do not stop the rest; only real deletion errors keep
    /// the entry so the user can try again.
    pub fn undo(&mut self, id: u64) -> Option<UndoReport> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        let entry = self.entries.remove(index);
        let mut report = UndoReport::default();
        for path in &entry.created {
            match fs::symlink_metadata(path) {
                Ok(metadata) => {
                    let result = if metadata.is_dir() {
                        fs::remove_dir_all(path)
                    } else {
                        fs::remove_file(path)
                    };
                    match result {
                        Ok(()) => report.removed.push(path.clone()),
                        Err(error) => report.failures.push((path.clone(), error.to_string())),
                    }
                }
                Err(_) => report.missing.push(path.clone()),
            }
        }
        if !report.is_clean() {
            // Put the entry back so a retry is possible; ids stay stable.
            self.entries.insert(index, entry);
        }
        Some(report)
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Short, human age for menus and rows: "just now", "4 min ago", "2 h ago".
pub fn relative_time(timestamp: u64, now: u64) -> String {
    if timestamp == 0 || timestamp > now {
        return "just now".into();
    }
    let seconds = now - timestamp;
    match seconds {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86_399 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn record_and_undo_deletes_outputs() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("out.txt");
        fs::write(&output, "hello").unwrap();

        let mut history = History::default();
        let id = history
            .record(
                "Test",
                vec![dir.path().join("in.txt")],
                vec![output.clone()],
            )
            .unwrap();
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].id, id);

        let report = history.undo(id).unwrap();
        assert_eq!(report.removed, vec![output.clone()]);
        assert!(!output.exists());
        assert!(history.entries.is_empty());
    }

    #[test]
    fn undo_removes_directories_and_reports_missing() {
        let dir = tempdir().unwrap();
        let folder = dir.path().join("extracted");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("a.txt"), "a").unwrap();
        let gone = dir.path().join("gone.txt");

        let mut history = History::default();
        let id = history
            .record("Extract", Vec::new(), vec![folder.clone(), gone.clone()])
            .unwrap();
        let report = history.undo(id).unwrap();
        assert!(!folder.exists());
        assert_eq!(report.removed, vec![folder]);
        assert_eq!(report.missing, vec![gone]);
        assert!(report.is_clean());
    }

    #[test]
    fn failed_deletion_keeps_the_entry() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let folder = dir.path().join("locked");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("inner"), "x").unwrap();
        // Deleting a child needs write permission on its parent.
        let mut permissions = fs::metadata(&folder).unwrap().permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(&folder, permissions).unwrap();

        let probe = dir.path().join("probe");
        fs::create_dir(&probe).unwrap();
        fs::write(probe.join("inner"), "x").unwrap();
        let mut probe_permissions = fs::metadata(&probe).unwrap().permissions();
        probe_permissions.set_mode(0o500);
        fs::set_permissions(&probe, probe_permissions).unwrap();
        let enforced = fs::remove_dir_all(&probe).is_err();
        let _ = fs::set_permissions(&probe, fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&probe);
        if !enforced {
            // Running with CAP_DAC_OVERRIDE (root); the failure path cannot be
            // provoked here, and the success path is covered above.
            let _ = fs::set_permissions(&folder, fs::Permissions::from_mode(0o700));
            return;
        }

        let mut history = History::default();
        let id = history
            .record("Locked", Vec::new(), vec![folder.clone()])
            .unwrap();
        let report = history.undo(id).unwrap();
        assert!(!report.is_clean());
        assert_eq!(history.entries.len(), 1, "entry kept for a retry");

        let _ = fs::set_permissions(&folder, fs::Permissions::from_mode(0o700));
    }

    #[test]
    fn pruning_keeps_the_newest_entries() {
        let mut history = History::default();
        for index in 0..(MAX_ENTRIES + 5) {
            history
                .record(format!("op {index}"), Vec::new(), vec![PathBuf::from("x")])
                .unwrap();
        }
        assert_eq!(history.entries.len(), MAX_ENTRIES);
        assert_eq!(history.entries[0].label, format!("op {}", MAX_ENTRIES + 4));
    }

    #[test]
    fn empty_output_is_not_recorded() {
        let mut history = History::default();
        assert!(history.record("nothing", Vec::new(), Vec::new()).is_none());
        assert!(history.entries.is_empty());
    }

    #[test]
    fn roundtrip_and_corrupt_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut history = History::default();
        history
            .record("One", Vec::new(), vec![dir.path().join("a")])
            .unwrap();
        history.save(&path).unwrap();

        let loaded = History::load(&path);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].label, "One");
        // Ids continue after the highest stored id.
        assert_eq!(loaded.next_id, loaded.entries[0].id + 1);

        fs::write(&path, b"not json").unwrap();
        assert!(History::load(&path).entries.is_empty());
    }

    #[test]
    fn relative_time_buckets() {
        assert_eq!(relative_time(100, 100), "just now");
        assert_eq!(relative_time(100, 130), "just now");
        assert_eq!(relative_time(100, 100 + 60), "1 min ago");
        assert_eq!(relative_time(100, 100 + 7200), "2 h ago");
        assert_eq!(relative_time(100, 100 + 90_000), "1 d ago");
    }
}
