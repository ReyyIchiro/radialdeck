use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::folder::{normalize_color, Availability, Folder, FolderError, Project};

pub const SCHEMA_VERSION: u32 = 5;

/// Persisted application state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schema: u32,
    pub active_project: Uuid,
    pub projects: Vec<Project>,
    #[serde(default)]
    pub preferences: Preferences,
}

impl Default for Config {
    fn default() -> Self {
        let project = Project::new("My Project", "#3b82f6");
        Self {
            schema: SCHEMA_VERSION,
            active_project: project.id,
            projects: vec![project],
            preferences: Preferences::default(),
        }
    }
}

impl Config {
    pub fn active_project(&self) -> &Project {
        self.projects
            .iter()
            .find(|project| project.id == self.active_project)
            .unwrap_or(&self.projects[0])
    }

    pub fn active_project_mut(&mut self) -> &mut Project {
        let index = self
            .projects
            .iter()
            .position(|project| project.id == self.active_project)
            .unwrap_or(0);
        let id = self.projects[index].id;
        self.active_project = id;
        &mut self.projects[index]
    }

    pub fn active_folders(&self) -> &[Folder] {
        &self.active_project().folders
    }

    pub fn folder(&self, project_id: Uuid, folder_id: Uuid) -> Option<&Folder> {
        self.projects
            .iter()
            .find(|project| project.id == project_id)?
            .folders
            .iter()
            .find(|folder| folder.id == folder_id)
    }

    pub fn set_active_project(&mut self, id: Uuid) -> bool {
        if self.projects.iter().any(|project| project.id == id) {
            self.active_project = id;
            true
        } else {
            false
        }
    }

    pub fn cycle_project(&mut self, offset: isize) {
        if self.projects.len() < 2 {
            return;
        }
        let index = self
            .projects
            .iter()
            .position(|project| project.id == self.active_project)
            .unwrap_or(0);
        let len = self.projects.len() as isize;
        let next = (index as isize + offset).rem_euclid(len) as usize;
        self.active_project = self.projects[next].id;
    }

    pub fn add_project(&mut self, name: &str, color: &str) -> Uuid {
        let name = name.trim();
        let name = if name.is_empty() { "New Project" } else { name };
        let project = Project::new(name, color);
        let id = project.id;
        self.projects.push(project);
        self.active_project = id;
        id
    }

    /// Removes a project. The last remaining project cannot be removed.
    pub fn remove_project(&mut self, id: Uuid) -> bool {
        if self.projects.len() <= 1 || !self.projects.iter().any(|project| project.id == id) {
            return false;
        }
        self.projects.retain(|project| project.id != id);
        if self.active_project == id {
            self.active_project = self.projects[0].id;
        }
        true
    }

    pub fn rename_project(&mut self, id: Uuid, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        if let Some(project) = self.projects.iter_mut().find(|project| project.id == id) {
            project.name = name.to_owned();
            true
        } else {
            false
        }
    }

    pub fn set_project_color(&mut self, id: Uuid, color: &str) -> bool {
        let Ok(color) = normalize_color(color) else {
            return false;
        };
        if let Some(project) = self.projects.iter_mut().find(|project| project.id == id) {
            project.color = color;
            true
        } else {
            false
        }
    }

    pub fn add_folder_to_active(
        &mut self,
        path: &Path,
        label: Option<&str>,
        color: Option<String>,
    ) -> Result<Uuid, FolderError> {
        self.active_project_mut().add_folder(path, label, color)
    }

    pub fn remove_folder(&mut self, project_id: Uuid, folder_id: Uuid) -> bool {
        self.projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .is_some_and(|project| project.remove_folder(folder_id))
    }

    pub fn rename_folder(&mut self, project_id: Uuid, folder_id: Uuid, label: &str) -> bool {
        let label = label.trim();
        if label.is_empty() {
            return false;
        }
        self.projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .and_then(|project| {
                project
                    .folders
                    .iter_mut()
                    .find(|folder| folder.id == folder_id)
            })
            .map(|folder| folder.label = label.to_owned())
            .is_some()
    }

    pub fn set_folder_color(
        &mut self,
        project_id: Uuid,
        folder_id: Uuid,
        color: Option<&str>,
    ) -> bool {
        let color = match color {
            Some(value) => match normalize_color(value) {
                Ok(color) => Some(color),
                Err(_) => return false,
            },
            None => None,
        };
        self.projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .and_then(|project| {
                project
                    .folders
                    .iter_mut()
                    .find(|folder| folder.id == folder_id)
            })
            .map(|folder| folder.color = color)
            .is_some()
    }

    pub fn move_folder(&mut self, project_id: Uuid, folder_id: Uuid, offset: isize) -> bool {
        self.projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .is_some_and(|project| project.move_folder(folder_id, offset))
    }

    pub fn availability_of(&self, folder: &Folder) -> Availability {
        crate::folder::availability(&folder.path)
    }

    /// Repairs invariants that corrupt or hand-edited files could break:
    /// schema at least 1, at least one project, unique project/folder ids,
    /// valid colors, absolute folder paths, and a valid active project.
    /// Returns human-readable notes about anything that was repaired.
    pub fn normalize(&mut self) -> Vec<String> {
        let previous_schema = self.schema;
        let mut notes = Vec::new();
        if self.schema > SCHEMA_VERSION {
            notes.push(format!(
                "config schema {} is newer than supported schema {SCHEMA_VERSION}",
                self.schema
            ));
        }
        self.schema = SCHEMA_VERSION;

        if self.preferences.hotkey.trim().is_empty() {
            self.preferences.hotkey = default_hotkey();
            notes.push("restored the default quick-actions shortcut".into());
        }
        if self.preferences.projects_hotkey.trim().is_empty() {
            self.preferences.projects_hotkey = default_projects_hotkey();
            notes.push("restored the default project-folders shortcut".into());
        }
        // Schema 3 moves the project wheel off the old P default. Only
        // migrate the old default, not a shortcut chosen after upgrading.
        if previous_schema < 3
            && self
                .preferences
                .projects_hotkey
                .eq_ignore_ascii_case("CTRL+ALT+P")
        {
            self.preferences.projects_hotkey = default_projects_hotkey();
            notes.push("project folders shortcut moved to CTRL+ALT+Q".into());
        }
        if self
            .preferences
            .hotkey
            .eq_ignore_ascii_case(&self.preferences.projects_hotkey)
        {
            let fallback = if self.preferences.hotkey.eq_ignore_ascii_case("CTRL+ALT+P") {
                "CTRL+ALT+F"
            } else {
                "CTRL+ALT+P"
            };
            self.preferences.projects_hotkey = fallback.into();
            notes.push(format!(
                "quick actions and project folders shared a shortcut; project folders now use {fallback}"
            ));
        }

        if self.preferences.output_subfolder.trim().is_empty() {
            self.preferences.output_subfolder = default_output_subfolder();
        }
        if let crate::OutputLocation::Folder(path) = &self.preferences.output_location {
            if !path.is_absolute() {
                self.preferences.output_location = crate::OutputLocation::Alongside;
                notes.push("output folder was not absolute; writing next to originals".into());
            }
        }

        if self.projects.is_empty() {
            notes.push("no projects found; created a default project".into());
            self.projects.push(Project::new("My Project", "#3b82f6"));
        }

        let mut seen_projects = std::collections::HashSet::new();
        self.projects
            .retain(|project| seen_projects.insert(project.id));
        for project in &mut self.projects {
            if project.name.trim().is_empty() {
                project.name = "Untitled Project".into();
                notes.push("renamed a project with an empty name".into());
            }
            if let Ok(color) = normalize_color(&project.color) {
                project.color = color;
            } else {
                project.color = "#3b82f6".into();
                notes.push(format!("reset invalid color on project '{}'", project.name));
            }

            let mut seen_folders = std::collections::HashSet::new();
            project.folders.retain_mut(|folder| {
                if !folder.path.is_absolute() {
                    notes.push(format!(
                        "dropped relative folder path '{}'",
                        folder.path.display()
                    ));
                    return false;
                }
                if !seen_folders.insert(folder.id) {
                    notes.push(format!("dropped duplicate folder id in '{}'", project.name));
                    return false;
                }
                if folder.label.trim().is_empty() {
                    folder.label = folder
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| folder.path.display().to_string());
                }
                if let Some(color) = folder.color.as_deref() {
                    match normalize_color(color) {
                        Ok(color) => folder.color = Some(color),
                        Err(_) => {
                            folder.color = None;
                            notes.push(format!("reset invalid color on '{}'", folder.label));
                        }
                    }
                }
                true
            });
        }

        if !self
            .projects
            .iter()
            .any(|project| project.id == self.active_project)
        {
            self.active_project = self.projects[0].id;
            notes.push("active project was missing; selected the first project".into());
        }

        notes
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    /// XDG shortcut syntax understood by the GlobalShortcuts portal.
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    /// Shortcut that opens the project-folder wheel.
    #[serde(default = "default_projects_hotkey")]
    pub projects_hotkey: String,
    #[serde(default = "default_true")]
    pub close_after_launch: bool,
    /// While the wheel is open and focused, Shift/Ctrl/Meta switch between
    /// the Convert, Tools and Actions wheels (Tangerine-style gestures).
    #[serde(default = "default_true")]
    pub live_modifier: bool,
    #[serde(default)]
    pub launch_at_login: bool,
    /// Terminal executable; `None` means auto-detect.
    #[serde(default)]
    pub terminal: Option<String>,
    #[serde(default)]
    pub theme: Theme,
    /// Where tool and conversion outputs are written.
    #[serde(default)]
    pub output_location: crate::OutputLocation,
    /// Folder name used when `output_location` is `Subfolder`.
    #[serde(default = "default_output_subfolder")]
    pub output_subfolder: String,
    /// Per-category tool order and visibility; a missing key means defaults.
    #[serde(default)]
    pub wheel_tools: std::collections::BTreeMap<String, Vec<String>>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            projects_hotkey: default_projects_hotkey(),
            close_after_launch: true,
            live_modifier: true,
            launch_at_login: false,
            terminal: None,
            theme: Theme::System,
            output_location: crate::OutputLocation::Alongside,
            output_subfolder: default_output_subfolder(),
            wheel_tools: std::collections::BTreeMap::new(),
        }
    }
}

fn default_output_subfolder() -> String {
    "RadialDeck".into()
}

fn default_hotkey() -> String {
    "CTRL+ALT+SPACE".into()
}

fn default_projects_hotkey() -> String {
    "CTRL+ALT+Q".into()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Turns `CTRL+ALT+SPACE` into `Ctrl + Alt + Space` for display.
pub fn pretty_hotkey(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|token| match token.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "super" | "meta" | "win" => "Super".to_string(),
            "space" => "Space".to_string(),
            "enter" | "return" => "Enter".to_string(),
            "escape" | "esc" => "Esc".to_string(),
            "tab" => "Tab".to_string(),
            "backspace" => "Backspace".to_string(),
            "delete" => "Delete".to_string(),
            "home" => "Home".to_string(),
            "end" => "End".to_string(),
            "pageup" => "PageUp".to_string(),
            "pagedown" => "PageDown".to_string(),
            "" => String::new(),
            other => {
                let mut chars = token.trim().chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => other.to_string(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_config_is_valid() {
        let mut config = Config::default();
        assert!(config.normalize().is_empty());
        assert_eq!(config.active_project().name, "My Project");
        assert_eq!(config.active_folders().len(), 0);
    }

    #[test]
    fn project_lifecycle_keeps_active_valid() {
        let mut config = Config::default();
        let work = config.add_project("Work", "#22c55e");
        assert_eq!(config.active_project, work);
        assert!(config.remove_project(work));
        assert_eq!(config.projects.len(), 1);
        assert!(!config.remove_project(config.active_project));
        assert_eq!(config.active_project().name, "My Project");
    }

    #[test]
    fn cycling_wraps() {
        let mut config = Config::default();
        let second = config.add_project("Second", "#ef4444");
        config.set_active_project(config.projects[0].id);
        config.cycle_project(-1);
        assert_eq!(config.active_project, second);
        config.cycle_project(1);
        assert_eq!(config.active_project().name, "My Project");
    }

    #[test]
    fn normalize_repairs_hand_edited_files() {
        let mut config = Config {
            schema: 99,
            active_project: Uuid::new_v4(),
            ..Config::default()
        };
        config.projects[0].color = "not-a-color".into();

        let dir = tempdir().unwrap();
        let mut invalid = Folder::new("relative/path".into(), "", None);
        invalid.color = Some("zzz".into());
        config.projects[0].folders.push(invalid);
        let missing = dir.path().join("gone");
        config.projects[0]
            .add_folder(&missing, None, None)
            .unwrap_err();
        let mut orphan = Folder::new(dir.path().to_path_buf(), "", None);
        orphan.color = Some("#ABCDEF".into());
        config.projects[0].folders.push(orphan);

        let notes = config.normalize();
        assert!(!notes.is_empty());
        assert_eq!(config.schema, SCHEMA_VERSION);
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].folders.len(), 1);
        assert_eq!(
            config.projects[0].folders[0].label,
            dir.path().file_name().unwrap().to_string_lossy()
        );
        assert_eq!(
            config.projects[0].folders[0].color.as_deref(),
            Some("#abcdef")
        );
        assert!(config.set_active_project(config.projects[0].id));
    }

    #[test]
    fn older_config_gets_project_shortcut_default() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["schema"] = serde_json::json!(1);
        value["preferences"]
            .as_object_mut()
            .unwrap()
            .remove("projects_hotkey");

        let mut config: Config = serde_json::from_value(value).unwrap();
        assert_eq!(config.schema, 1);
        assert_eq!(config.preferences.projects_hotkey, "CTRL+ALT+Q");
        assert!(config.normalize().is_empty());
        assert_eq!(config.schema, SCHEMA_VERSION);
    }

    #[test]
    fn old_project_shortcut_migrates_to_q() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["schema"] = serde_json::json!(2);
        value["preferences"]["projects_hotkey"] = serde_json::json!("CTRL+ALT+P");

        let mut config: Config = serde_json::from_value(value).unwrap();
        let notes = config.normalize();
        assert_eq!(config.preferences.projects_hotkey, "CTRL+ALT+Q");
        assert!(notes.iter().any(|note| note.contains("CTRL+ALT+Q")));
        assert_eq!(config.schema, SCHEMA_VERSION);
    }

    #[test]
    fn duplicate_shortcuts_are_repaired() {
        let mut config = Config::default();
        config.preferences.projects_hotkey = config.preferences.hotkey.clone();
        let notes = config.normalize();
        assert!(notes.iter().any(|note| note.contains("shared a shortcut")));
        assert_ne!(
            config.preferences.hotkey,
            config.preferences.projects_hotkey
        );
    }

    #[test]
    fn live_modifier_defaults_on_for_existing_configs() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["schema"] = serde_json::json!(3);
        value["preferences"]
            .as_object_mut()
            .unwrap()
            .remove("live_modifier");

        let mut config: Config = serde_json::from_value(value).unwrap();
        assert!(config.preferences.live_modifier);
        assert!(config.normalize().is_empty());
        assert_eq!(config.schema, SCHEMA_VERSION);
    }

    #[test]
    fn live_modifier_can_be_disabled() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["preferences"]["live_modifier"] = serde_json::json!(false);
        let config: Config = serde_json::from_value(value).unwrap();
        assert!(!config.preferences.live_modifier);
    }

    #[test]
    fn hotkey_is_pretty_printed() {
        assert_eq!(pretty_hotkey("CTRL+ALT+SPACE"), "Ctrl + Alt + Space");
        assert_eq!(pretty_hotkey("super+shift+k"), "Super + Shift + K");
    }
}
