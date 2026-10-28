//! Per-operation context shared by tools and converters.
//!
//! A worker thread enters a [`Scope`] before running a batch. Inside it, the
//! output-location policy, cancellation flag and progress reporter are
//! available without threading them through every helper signature. The scope
//! is thread-local, so concurrent operations never see each other's state, and
//! a call outside any scope behaves exactly like the old code (outputs next to
//! the source, no cancellation, no reporting).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Where new files are written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputLocation {
    /// Next to each source file.
    #[default]
    Alongside,
    /// In a subfolder of the source's directory.
    Subfolder(String),
    /// In one fixed folder.
    Folder(PathBuf),
}

impl OutputLocation {
    /// Resolves the directory for `source`. The caller creates it when needed;
    /// resolving never touches the filesystem so it stays cheap in hot paths.
    pub fn directory_for(&self, source: &Path) -> PathBuf {
        let parent = source
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        match self {
            Self::Alongside => parent,
            Self::Subfolder(name) => {
                let name = name.trim();
                let name = if name.is_empty() { "RadialDeck" } else { name };
                // A subfolder name is one directory, never a path traversal.
                let safe: String = name
                    .chars()
                    .map(|ch| match ch {
                        '/' | '\\' | '\0' => '_',
                        _ => ch,
                    })
                    .collect();
                parent.join(safe)
            }
            Self::Folder(folder) => folder.clone(),
        }
    }

    /// Creates the destination directory. Errors are returned so a tool can
    /// report them instead of failing later with a confusing message.
    pub fn prepare(&self, source: &Path) -> std::io::Result<PathBuf> {
        let directory = self.directory_for(source);
        std::fs::create_dir_all(&directory)?;
        Ok(directory)
    }

    pub fn label(&self) -> String {
        match self {
            Self::Alongside => "Next to the original".into(),
            Self::Subfolder(name) if name.trim().is_empty() => "Subfolder".into(),
            Self::Subfolder(name) => format!("Subfolder “{name}”"),
            Self::Folder(path) => path.display().to_string(),
        }
    }
}

/// Progress reported while a batch runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Files finished so far, starting at 1 for the first result.
    pub done: usize,
    pub total: usize,
    /// Name of the file being processed.
    pub current: String,
    /// Latest line from the child process, when it emits one.
    pub detail: Option<String>,
}

pub type Reporter = Arc<dyn Fn(Progress) + Send + Sync>;

#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
struct State {
    output: OutputLocation,
    cancel: Option<CancelToken>,
    reporter: Option<Reporter>,
    /// Paths created by this operation, for cleanup after a cancel.
    registered: Vec<PathBuf>,
    total: usize,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Runs `body` inside a fresh operation scope. Nested scopes replace the
/// outer one and restore it afterwards, which keeps tests predictable.
pub fn scope<T>(
    output: OutputLocation,
    cancel: Option<CancelToken>,
    body: impl FnOnce() -> T,
) -> T {
    let previous = STATE.with(|state| state.borrow_mut().take());
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            output,
            cancel,
            reporter: None,
            registered: Vec::new(),
            total: 0,
        });
    });
    let result = body();
    STATE.with(|state| {
        *state.borrow_mut() = previous;
    });
    result
}

/// Sets the reporter for the current scope. Ignored outside a scope.
pub fn set_reporter(reporter: Reporter) {
    STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut() {
            state.reporter = Some(reporter);
        }
    });
}

/// Declares how many files the current batch contains.
pub fn set_total(total: usize) {
    STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut() {
            state.total = total;
        }
    });
}

pub fn cancelled() -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .and_then(|state| state.cancel.as_ref())
            .is_some_and(CancelToken::is_cancelled)
    })
}

/// Reports a finished file. `current` names the file that just completed, so
/// the UI can show "3 of 5 · clip.mp4" while the next one starts.
pub fn report_file(current: &Path) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        let Some(reporter) = state.reporter.clone() else {
            return;
        };
        let name = current
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| current.display().to_string());
        reporter(Progress {
            done: 1,
            total: state.total.max(1),
            current: name,
            detail: None,
        });
    });
}

/// Forwards a line of child-process output. FFmpeg writes live statistics to
/// stderr; the last line is enough to show that work is still happening.
pub fn report_detail(line: &str) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        let Some(reporter) = state.reporter.clone() else {
            return;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        reporter(Progress {
            done: 0,
            total: state.total.max(1),
            current: String::new(),
            detail: Some(trimmed.chars().take(160).collect()),
        });
    });
}

/// Destination directory for `source` under the current policy, created if
/// needed. Outside a scope this is simply the source's parent.
pub fn destination_dir(source: &Path) -> PathBuf {
    let policy = STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.output.clone())
            .unwrap_or_default()
    });
    match policy.prepare(source) {
        Ok(directory) => directory,
        Err(error) => {
            tracing::warn!(
                "could not create output folder {}: {error}",
                policy.directory_for(source).display()
            );
            policy.directory_for(source)
        }
    }
}

/// Records a path created by the current operation so a cancel can remove it.
/// Outside a scope this is a no-op.
pub fn register(path: &Path) {
    STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut() {
            state.registered.push(path.to_path_buf());
        }
    });
}

/// Deletes everything registered in the current scope. Used after a cancel;
/// errors are logged because the original error is more useful to the user.
pub fn cleanup_registered() {
    let paths = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state
            .as_mut()
            .map(|state| std::mem::take(&mut state.registered))
            .unwrap_or_default()
    });
    for path in paths.into_iter().rev() {
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {
                if let Err(error) = std::fs::remove_dir_all(&path) {
                    tracing::warn!("could not remove {}: {error}", path.display());
                }
            }
            Ok(_) => {
                if let Err(error) = std::fs::remove_file(&path) {
                    tracing::warn!("could not remove {}: {error}", path.display());
                }
            }
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn policy_resolves_expected_directories() {
        let source = Path::new("/home/user/Photos/clip.mp4");
        assert_eq!(
            OutputLocation::Alongside.directory_for(source),
            PathBuf::from("/home/user/Photos")
        );
        assert_eq!(
            OutputLocation::Subfolder("converted".into()).directory_for(source),
            PathBuf::from("/home/user/Photos/converted")
        );
        assert_eq!(
            OutputLocation::Folder("/tmp/out".into()).directory_for(source),
            PathBuf::from("/tmp/out")
        );
    }

    #[test]
    fn subfolder_name_cannot_escape_its_parent() {
        let source = Path::new("/home/user/a.txt");
        let directory = OutputLocation::Subfolder("../escape".into()).directory_for(source);
        assert_eq!(directory, PathBuf::from("/home/user/.._escape"));
    }

    #[test]
    fn prepare_creates_the_directory() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("file.txt");
        let policy = OutputLocation::Subfolder("out".into());
        let created = policy.prepare(&source).unwrap();
        assert!(created.is_dir());
        assert_eq!(created, dir.path().join("out"));
    }

    #[test]
    fn scope_isolates_state_and_restores_the_previous_one() {
        let cancel = CancelToken::new();
        let source = Path::new("/tmp/x/y.png");
        let inside = scope(
            OutputLocation::Folder("/tmp/deck-out".into()),
            Some(cancel.clone()),
            || {
                assert_eq!(destination_dir(source), PathBuf::from("/tmp/deck-out"));
                cancel.cancel();
                assert!(cancelled());
                "done"
            },
        );
        assert_eq!(inside, "done");
        assert!(!cancelled(), "state is gone after the scope");
        assert_eq!(destination_dir(source), PathBuf::from("/tmp/x"));
    }

    #[test]
    fn registered_paths_are_cleaned_up() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("partial.bin");
        std::fs::write(&file, "x").unwrap();
        let folder = dir.path().join("partial-dir");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("inner"), "x").unwrap();

        scope(OutputLocation::Alongside, None, || {
            register(&file);
            register(&folder);
            cleanup_registered();
        });
        assert!(!file.exists());
        assert!(!folder.exists());
    }

    #[test]
    fn progress_reaches_the_reporter() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        scope(OutputLocation::Alongside, None, || {
            set_total(3);
            set_reporter(Arc::new(move |progress| {
                sink.lock().unwrap().push(progress);
            }));
            report_file(Path::new("/tmp/one.png"));
            report_detail("frame= 42 fps=30");
        });
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].current, "one.png");
        assert_eq!(seen[0].total, 3);
        assert_eq!(seen[1].detail.as_deref(), Some("frame= 42 fps=30"));
    }
}
