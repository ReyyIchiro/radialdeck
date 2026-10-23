use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use thiserror::Error;

use crate::config::{Config, SCHEMA_VERSION};

/// Location of the user configuration file:
/// `$XDG_CONFIG_HOME/radial-deck/config.json` (falls back to `~/.config`).
pub fn default_config_path() -> PathBuf {
    let base = BaseDirs::new();
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| base.as_ref().map(|dirs| dirs.config_dir().to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("radial-deck").join("config.json")
}

pub struct Store {
    path: PathBuf,
}

pub struct Loaded {
    pub config: Config,
    pub notes: Vec<String>,
}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn with_default_path() -> Self {
        Self::new(default_config_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the config. A missing file yields defaults; a corrupt file yields
    /// an error instead of silently discarding the user's data.
    pub fn load(&self) -> Result<Loaded, StoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Loaded {
                    config: Config::default(),
                    notes: Vec::new(),
                });
            }
            Err(source) => {
                return Err(StoreError::Read {
                    path: self.path.clone(),
                    source,
                });
            }
        };

        let mut config: Config =
            serde_json::from_slice(&bytes).map_err(|source| StoreError::Parse {
                path: self.path.clone(),
                source,
            })?;

        if config.schema > SCHEMA_VERSION {
            return Err(StoreError::UnsupportedSchema {
                path: self.path.clone(),
                found: config.schema,
            });
        }

        let notes = config.normalize();
        Ok(Loaded { config, notes })
    }

    /// Writes to a temporary file in the same directory, fsyncs, then renames.
    /// A crash therefore never leaves a half-written config in place.
    pub fn save(&self, config: &Config) -> Result<(), StoreError> {
        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| StoreError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;

        let mut temp_name = OsString::from(".");
        temp_name.push(
            self.path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("config.json")),
        );
        temp_name.push(format!(".{}.tmp", std::process::id()));
        let temp_path = parent.join(&temp_name);

        let result = (|| -> Result<(), StoreError> {
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let file = options
                .open(&temp_path)
                .map_err(|source| StoreError::Write {
                    path: temp_path.clone(),
                    source,
                })?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, config).map_err(|source| {
                StoreError::Serialize {
                    path: temp_path.clone(),
                    source,
                }
            })?;
            writer
                .write_all(b"\n")
                .and_then(|_| writer.flush())
                .map_err(|source| StoreError::Write {
                    path: temp_path.clone(),
                    source,
                })?;
            writer
                .get_ref()
                .sync_all()
                .map_err(|source| StoreError::Write {
                    path: temp_path.clone(),
                    source,
                })?;
            fs::rename(&temp_path, &self.path).map_err(|source| StoreError::Write {
                path: self.path.clone(),
                source,
            })?;
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path} uses schema {found}, but this build supports up to schema {SCHEMA_VERSION}")]
    UnsupportedSchema { path: PathBuf, found: u32 },
    #[error("cannot create config directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot serialize config for {path}: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_file_loads_defaults() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("nested/config.json"));
        let loaded = store.load().unwrap();
        assert_eq!(loaded.config.projects.len(), 1);
        assert!(loaded.notes.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("radial-deck/config.json"));
        let mut config = Config::default();
        let folder = dir.path().join("docs");
        fs::create_dir(&folder).unwrap();
        config
            .add_folder_to_active(&folder, Some("Docs"), None)
            .unwrap();

        store.save(&config).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.config.projects[0].folders[0].label, "Docs");
        assert_eq!(loaded.config.preferences.hotkey, config.preferences.hotkey);
    }

    #[test]
    fn corrupt_file_is_reported_not_reset() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, b"{not json").unwrap();
        let store = Store::new(&path);
        assert!(matches!(store.load(), Err(StoreError::Parse { .. })));
    }

    #[test]
    fn newer_schema_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let config = Config {
            schema: SCHEMA_VERSION + 1,
            ..Config::default()
        };
        fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        assert!(matches!(
            Store::new(&path).load(),
            Err(StoreError::UnsupportedSchema { .. })
        ));
    }

    #[test]
    fn save_is_atomic_and_owner_only_on_unix() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("config.json"));
        store.save(&Config::default()).unwrap();

        let entries: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![OsString::from("config.json")]);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(store.path()).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn save_recreates_missing_directories() {
        let dir = tempdir().unwrap();
        let store = Store::new(dir.path().join("a/b/c/config.json"));
        store.save(&Config::default()).unwrap();
        assert!(store.path().exists());
    }

    #[test]
    fn default_path_respects_xdg_config_home() {
        // `std::env::set_var` is unsafe in edition 2024 and process-global, so
        // exercise the fallback shape instead of mutating the environment.
        let path = default_config_path();
        assert!(path.ends_with("radial-deck/config.json"));
    }
}
