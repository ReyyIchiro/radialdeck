use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use deck_core::tools::{self, ToolId, ToolOutcome, ToolRequest};
use deck_core::{Category, Config, FolderError, Store, StoreError, Theme, Tools};
use gtk::glib;
use gtk::prelude::*;
use uuid::Uuid;

use crate::action_launcher::{ActionLauncher, ActionRequest, ActionView, FormView, WheelMode};
use crate::action_wheel::WheelTarget;
use crate::actions;
use crate::autostart;
use crate::float_shelf::FloatShelf;
use crate::hotkey::{self, HotkeyEvent, HotkeyService};
use crate::launcher::Launcher;
use crate::manage::ManageWindow;
use crate::settings::SettingsWindow;
use crate::theme;
use crate::tray::{Tray, TrayCommand};
use crate::wheel::{Action, Intent, ProjectTab, WheelData, WheelFolder};

#[derive(Debug, Default, Clone)]
pub struct Cli {
    pub hidden: bool,
    pub show: bool,
    pub projects: bool,
    pub manage: bool,
    pub settings: bool,
    pub reset: bool,
    pub version: bool,
    pub help: bool,
    pub mode: Option<WheelMode>,
    pub files: Vec<PathBuf>,
}

impl Cli {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut cli = Self::default();
        for arg in args {
            match arg.as_str() {
                "--hidden" => cli.hidden = true,
                "--show" => cli.show = true,
                "--convert" | "--mode=convert" => cli.mode = Some(WheelMode::Convert),
                "--tools" | "--mode=tools" => cli.mode = Some(WheelMode::Tools),
                "--actions" | "--mode=actions" => cli.mode = Some(WheelMode::Actions),
                "--projects" | "--folders" => cli.projects = true,
                "--manage" => cli.manage = true,
                "--settings" => cli.settings = true,
                "--reset-config" => cli.reset = true,
                "--version" | "-V" => cli.version = true,
                "--help" | "-h" => cli.help = true,
                other if !other.starts_with('-') => cli.files.push(PathBuf::from(other)),
                _ => {}
            }
        }
        cli
    }
}

#[derive(Default)]
pub struct HotkeyState {
    pub quick: String,
    pub projects: String,
    pub quick_error: Option<String>,
    pub projects_error: Option<String>,
}

pub struct Controller {
    pub app: RefCell<Option<gtk::Application>>,
    pub store: Store,
    pub config: RefCell<Config>,
    pub launcher: RefCell<Option<Launcher>>,
    pub manage: RefCell<Option<ManageWindow>>,
    pub settings: RefCell<Option<SettingsWindow>>,
    pub action_launcher: RefCell<Option<ActionLauncher>>,
    action_sources: RefCell<Vec<PathBuf>>,
    action_category: Cell<Category>,
    action_mode: Cell<WheelMode>,
    action_tool: RefCell<Option<ToolId>>,
    action_status: RefCell<Option<String>>,
    airdrop_devices: RefCell<Vec<actions::KdeDevice>>,
    float: RefCell<Option<Rc<FloatShelf>>>,
    tools: Tools,
    converting: Cell<bool>,
    history: RefCell<deck_core::History>,
    hotkey: RefCell<Option<HotkeyService>>,
    tray: RefCell<Option<ksni::blocking::Handle<Tray>>>,
    hold_guard: RefCell<Option<gtk::gio::ApplicationHoldGuard>>,
    pub hotkey_state: RefCell<HotkeyState>,
    cli_handled: Cell<bool>,
    startup_notice: RefCell<Option<String>>,
    fatal_error: RefCell<Option<String>>,
}

impl Controller {
    pub fn new(flags: Cli) -> Rc<Self> {
        let store = Store::with_default_path();
        let mut startup_notice = None;
        let mut fatal_error = None;

        if flags.reset {
            let _ = std::fs::remove_file(store.path());
        }

        let config = match store.load() {
            Ok(loaded) => {
                if !loaded.notes.is_empty() {
                    startup_notice = Some(format!(
                        "Configuration was repaired:\n{}",
                        loaded.notes.join("\n")
                    ));
                }
                loaded.config
            }
            Err(error @ StoreError::Parse { .. }) => {
                let backup = store.path().with_extension("json.corrupt");
                let recovered = std::fs::rename(store.path(), &backup).is_ok();
                if recovered {
                    startup_notice = Some(format!(
                        "Could not read the configuration ({error}).\nThe old file was kept at {}.\nStarting with defaults.",
                        backup.display()
                    ));
                    Config::default()
                } else {
                    fatal_error = Some(format!(
                        "Could not read the configuration:\n{error}\n\nFile: {}",
                        store.path().display()
                    ));
                    Config::default()
                }
            }
            Err(error) => {
                fatal_error = Some(format!(
                    "Could not read the configuration:\n{error}\n\nFile: {}",
                    store.path().display()
                ));
                Config::default()
            }
        };

        let history_path = deck_core::history::default_history_path();
        let history = deck_core::History::load(&history_path);

        Rc::new(Self {
            app: RefCell::new(None),
            store,
            config: RefCell::new(config),
            launcher: RefCell::new(None),
            manage: RefCell::new(None),
            settings: RefCell::new(None),
            action_launcher: RefCell::new(None),
            action_sources: RefCell::new(Vec::new()),
            action_category: Cell::new(Category::Images),
            action_mode: Cell::new(WheelMode::Convert),
            action_tool: RefCell::new(None),
            action_status: RefCell::new(None),
            airdrop_devices: RefCell::new(Vec::new()),
            float: RefCell::new(None),
            tools: Tools::detect(),
            converting: Cell::new(false),
            history: RefCell::new(history),
            hotkey: RefCell::new(None),
            tray: RefCell::new(None),
            hold_guard: RefCell::new(None),
            hotkey_state: RefCell::new(HotkeyState::default()),
            cli_handled: Cell::new(false),
            startup_notice: RefCell::new(startup_notice),
            fatal_error: RefCell::new(fatal_error),
        })
    }

    pub fn start(self: &Rc<Self>, app: &gtk::Application) {
        *self.app.borrow_mut() = Some(app.clone());
        let guard = app.hold();
        self.hold_guard.replace(Some(guard));

        theme::apply_theme(self.config.borrow().preferences.theme);
        self.sync_autostart_preference();

        if let Some(error) = self.fatal_error.borrow().clone() {
            self.alert("RadialDeck cannot start", &error);
            app.quit();
            return;
        }

        if let Some(notice) = self.startup_notice.borrow_mut().take() {
            self.alert("Configuration notice", &notice);
        }

        self.start_tray();
        self.start_hotkey();
    }

    pub fn config(&self) -> std::cell::Ref<'_, Config> {
        self.config.borrow()
    }

    fn sync_autostart_preference(&self) {
        let enabled = autostart::is_enabled();
        if self.config.borrow().preferences.launch_at_login != enabled {
            self.config.borrow_mut().preferences.launch_at_login = enabled;
            let _ = self.store.save(&self.config.borrow());
        }
    }

    fn start_tray(self: &Rc<Self>) {
        let (tx, rx) = async_channel::unbounded();
        let tray = Tray::new(
            tx,
            self.config.borrow().preferences.launch_at_login,
            self.display_shortcut(),
            self.display_projects_shortcut(),
        );
        let mut tray = tray;
        tray.set_recent(self.history.borrow().recent(5).to_vec());
        use ksni::blocking::TrayMethods;
        match tray.spawn() {
            Ok(handle) => {
                *self.tray.borrow_mut() = Some(handle);
            }
            Err(error) => {
                tracing::warn!("system tray unavailable: {error}");
            }
        }

        let controller = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(command) = rx.recv().await {
                let Some(controller) = controller.upgrade() else {
                    break;
                };
                match command {
                    TrayCommand::Show => controller.toggle_actions(),
                    TrayCommand::Projects => controller.show_launcher(),
                    TrayCommand::Manage => controller.open_manage(),
                    TrayCommand::Settings => controller.open_settings(),
                    TrayCommand::ToggleAutostart => controller.toggle_autostart(),
                    TrayCommand::Undo(id) => controller.undo_operation(id),
                    TrayCommand::Quit => controller.quit(),
                }
            }
        });
    }

    fn start_hotkey(self: &Rc<Self>) {
        if std::env::var_os("RADIAL_DECK_NO_HOTKEY").is_some() {
            tracing::info!("global shortcuts disabled by RADIAL_DECK_NO_HOTKEY");
            return;
        }
        let (tx, rx) = async_channel::unbounded();
        let (quick, projects) = {
            let config = self.config.borrow();
            (
                config.preferences.hotkey.clone(),
                config.preferences.projects_hotkey.clone(),
            )
        };
        let service = hotkey::start(quick, projects, tx);
        *self.hotkey.borrow_mut() = Some(service);

        let controller = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                let Some(controller) = controller.upgrade() else {
                    break;
                };
                match event {
                    HotkeyEvent::Activated(id) => {
                        if id == hotkey::PROJECTS_ID {
                            controller.toggle_launcher();
                        } else {
                            controller.toggle_actions();
                        }
                    }
                    HotkeyEvent::Bound { id, description } => {
                        controller.update_hotkey_state(&id, Ok(description));
                        tracing::info!(
                            "shortcut {id} active: {}",
                            controller.display_shortcut_for(&id)
                        );
                    }
                    HotkeyEvent::Error { id, message } => {
                        controller.update_hotkey_state(&id, Err(message));
                        tracing::warn!(
                            "shortcut {id} error: {}",
                            controller.hotkey_error_for(&id).unwrap_or_default()
                        );
                    }
                }
            }
        });
    }

    fn update_hotkey_state(&self, id: &str, result: Result<String, String>) {
        {
            let mut state = self.hotkey_state.borrow_mut();
            match (id, result) {
                (hotkey::PROJECTS_ID, Ok(description)) => {
                    state.projects = description;
                    state.projects_error = None;
                }
                (hotkey::PROJECTS_ID, Err(error)) => state.projects_error = Some(error),
                (hotkey::QUICK_ID, Ok(description)) => {
                    state.quick = description;
                    state.quick_error = None;
                }
                (hotkey::QUICK_ID, Err(error)) => state.quick_error = Some(error),
                (_, Ok(description)) => {
                    state.quick = description;
                    state.quick_error = None;
                }
                (_, Err(error)) => {
                    state.quick_error = Some(error.clone());
                    state.projects_error = Some(error);
                }
            }
        }
        if let Some(settings) = self.settings.borrow().as_ref() {
            settings.refresh();
        }
        self.update_tray();
    }

    pub fn display_shortcut(&self) -> String {
        self.display_shortcut_for(hotkey::QUICK_ID)
    }

    pub fn display_projects_shortcut(&self) -> String {
        self.display_shortcut_for(hotkey::PROJECTS_ID)
    }

    fn display_shortcut_for(&self, id: &str) -> String {
        let state = self.hotkey_state.borrow();
        if id == hotkey::PROJECTS_ID {
            if state.projects.is_empty() {
                deck_core::config::pretty_hotkey(&self.config.borrow().preferences.projects_hotkey)
            } else {
                state.projects.clone()
            }
        } else if state.quick.is_empty() {
            deck_core::config::pretty_hotkey(&self.config.borrow().preferences.hotkey)
        } else {
            state.quick.clone()
        }
    }

    pub fn hotkey_error_for(&self, id: &str) -> Option<String> {
        let state = self.hotkey_state.borrow();
        if id == hotkey::PROJECTS_ID {
            state.projects_error.clone()
        } else {
            state.quick_error.clone()
        }
    }

    pub fn configure_hotkey(&self) {
        if let Some(service) = self.hotkey.borrow().as_ref() {
            service.configure();
        }
    }

    pub fn wheel_data(&self) -> WheelData {
        let config = self.config.borrow();
        let project = config.active_project();
        WheelData {
            active_project: project.id,
            project_name: project.name.clone(),
            project_color: project.color.clone(),
            shortcut: self.display_projects_shortcut(),
            folders: project
                .folders
                .iter()
                .map(|folder| WheelFolder {
                    id: folder.id,
                    label: folder.label.clone(),
                    color: folder.color.clone(),
                    availability: config.availability_of(folder),
                })
                .collect(),
            projects: config
                .projects
                .iter()
                .map(|project| ProjectTab {
                    id: project.id,
                    name: project.name.clone(),
                    color: project.color.clone(),
                    folder_count: project.folders.len(),
                })
                .collect(),
        }
    }

    pub fn refresh_launcher(&self) {
        if let Some(launcher) = self.launcher.borrow().as_ref() {
            launcher.set_data(self.wheel_data());
        }
    }

    pub fn show_launcher(self: &Rc<Self>) {
        self.hide_actions();
        let recreate = self
            .launcher
            .borrow()
            .as_ref()
            .is_none_or(Launcher::is_destroyed);
        if recreate {
            *self.launcher.borrow_mut() = None;
            let Some(app) = self.app.borrow().clone() else {
                return;
            };
            let launcher = Launcher::new(&app, Rc::downgrade(self));
            *self.launcher.borrow_mut() = Some(launcher);
        }
        if let Some(launcher) = self.launcher.borrow().as_ref() {
            launcher.show(self.wheel_data());
        }
    }

    pub fn hide_launcher(&self) {
        if let Some(launcher) = self.launcher.borrow().as_ref() {
            launcher.hide();
        }
    }

    pub fn toggle_launcher(self: &Rc<Self>) {
        let visible = self
            .launcher
            .borrow()
            .as_ref()
            .is_some_and(Launcher::is_visible);
        tracing::debug!("toggle project folders: visible={visible}");
        if visible {
            self.hide_launcher();
        } else {
            self.show_launcher();
        }
    }

    pub fn open_manage(self: &Rc<Self>) {
        self.hide_launcher();
        self.hide_actions();
        if self.manage.borrow().is_none() {
            let Some(app) = self.app.borrow().clone() else {
                return;
            };
            let manage = ManageWindow::new(&app, Rc::downgrade(self));
            let window = manage.window().clone();
            *self.manage.borrow_mut() = Some(manage);
            let weak = Rc::downgrade(self);
            window.connect_destroy(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.manage.borrow_mut().take();
                }
            });
        }
        if let Some(manage) = self.manage.borrow().as_ref() {
            manage.refresh();
            manage.present();
        }
    }

    pub fn open_settings(self: &Rc<Self>) {
        self.hide_launcher();
        self.hide_actions();
        if self.settings.borrow().is_none() {
            let Some(app) = self.app.borrow().clone() else {
                return;
            };
            let settings = SettingsWindow::new(&app, Rc::downgrade(self));
            let window = settings.window().clone();
            *self.settings.borrow_mut() = Some(settings);
            let weak = Rc::downgrade(self);
            window.connect_destroy(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.settings.borrow_mut().take();
                }
            });
        }
        if let Some(settings) = self.settings.borrow().as_ref() {
            settings.refresh();
            settings.present();
        }
    }

    pub fn handle_wheel_action(self: &Rc<Self>, action: Action) {
        match action {
            Action::Launch(folder, intent) => self.launch(folder, intent),
            Action::Dismiss => self.hide_launcher(),
            Action::Manage => {
                self.hide_launcher();
                self.open_manage();
            }
            Action::Remove(folder_id) => {
                let project_id = self.config.borrow().active_project().id;
                self.remove_folder(project_id, folder_id);
            }
        }
    }

    pub fn launch(self: &Rc<Self>, folder_id: Uuid, intent: Intent) {
        let (folder, preferences) = {
            let config = self.config.borrow();
            let project = config.active_project();
            let Some(folder) = project
                .folders
                .iter()
                .find(|folder| folder.id == folder_id)
                .cloned()
            else {
                return;
            };
            (folder, config.preferences.clone())
        };

        match deck_core::availability(&folder.path) {
            deck_core::Availability::Available => {}
            deck_core::Availability::Missing => {
                self.notify(
                    "Folder unavailable",
                    &format!("{} was not found:\n{}", folder.label, folder.path.display()),
                );
                return;
            }
            deck_core::Availability::Offline => {
                self.notify(
                    "Drive not connected",
                    &format!(
                        "{} is on a drive that is not mounted:\n{}",
                        folder.label,
                        folder.path.display()
                    ),
                );
                return;
            }
        }

        if preferences.close_after_launch {
            self.hide_launcher();
        }

        let result = match intent {
            Intent::Open => actions::open_folder(&folder.path),
            Intent::Terminal => {
                match actions::open_terminal(&folder.path, preferences.terminal.as_deref()) {
                    Ok(()) => Ok(()),
                    Err(error) => Err(error),
                }
            }
            Intent::CopyPath => {
                actions::copy_path_to_clipboard(&folder.path);
                Ok(())
            }
        };

        if let Err(error) = result {
            self.notify(
                "Could not open folder",
                &format!("{}: {error}", folder.path.display()),
            );
        }
    }

    pub fn add_folder(self: &Rc<Self>, path: PathBuf) -> Result<(), FolderError> {
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        let id = self
            .config
            .borrow_mut()
            .add_folder_to_active(&path, label.as_deref(), None)?;
        tracing::info!("added folder {id} ({})", path.display());
        self.save();
        self.refresh_launcher();
        self.refresh_manage();
        Ok(())
    }

    pub fn remove_folder(&self, project_id: Uuid, folder_id: Uuid) {
        if self
            .config
            .borrow_mut()
            .remove_folder(project_id, folder_id)
        {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn move_folder(&self, project_id: Uuid, folder_id: Uuid, offset: isize) {
        if self
            .config
            .borrow_mut()
            .move_folder(project_id, folder_id, offset)
        {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn rename_folder(&self, project_id: Uuid, folder_id: Uuid, label: &str) {
        if self
            .config
            .borrow_mut()
            .rename_folder(project_id, folder_id, label)
        {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn set_folder_color(&self, project_id: Uuid, folder_id: Uuid, color: Option<&str>) {
        if self
            .config
            .borrow_mut()
            .set_folder_color(project_id, folder_id, color)
        {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn set_project_color(&self, id: Uuid, color: &str) {
        if self.config.borrow_mut().set_project_color(id, color) {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn add_project(self: &Rc<Self>, name: &str) {
        self.config
            .borrow_mut()
            .add_project(name, &next_color(self));
        self.save();
        self.refresh_launcher();
        self.refresh_manage();
    }

    pub fn rename_project(&self, id: Uuid, name: &str) {
        if self.config.borrow_mut().rename_project(id, name) {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn delete_project(self: &Rc<Self>, id: Uuid) {
        if self.config.borrow_mut().remove_project(id) {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn set_active_project(self: &Rc<Self>, id: Uuid) {
        if self.config.borrow_mut().set_active_project(id) {
            self.save();
            self.refresh_launcher();
            self.refresh_manage();
        }
    }

    pub fn set_close_after_launch(&self, value: bool) {
        self.config.borrow_mut().preferences.close_after_launch = value;
        self.save();
    }

    pub fn set_live_modifier(&self, enabled: bool) {
        self.config.borrow_mut().preferences.live_modifier = enabled;
        self.save();
    }

    pub fn set_theme(&self, theme_value: Theme) {
        self.config.borrow_mut().preferences.theme = theme_value;
        theme::apply_theme(theme_value);
        self.save();
        if let Some(launcher) = self.launcher.borrow().as_ref() {
            launcher.update_theme();
        }
        self.refresh_manage();
    }

    pub fn set_terminal(&self, terminal: Option<String>) {
        self.config.borrow_mut().preferences.terminal = terminal;
        self.save();
    }

    pub fn toggle_autostart(self: &Rc<Self>) {
        let enabled = !self.config.borrow().preferences.launch_at_login;
        self.set_autostart(enabled);
    }

    pub fn set_autostart(&self, enabled: bool) {
        let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("radial-deck"));
        match autostart::set_enabled(enabled, &executable) {
            Ok(()) => {
                self.config.borrow_mut().preferences.launch_at_login = enabled;
                self.save();
                self.update_tray();
                if let Some(settings) = self.settings.borrow().as_ref() {
                    settings.refresh();
                }
            }
            Err(error) => {
                self.alert(
                    "Could not change autostart",
                    &format!("{}: {error}", autostart::autostart_path().display()),
                );
            }
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.store.path().to_path_buf()
    }

    pub fn open_config_folder(self: &Rc<Self>) {
        if let Some(parent) = self.store.path().parent() {
            if let Err(error) = actions::open_folder(parent) {
                self.alert("Could not open config folder", &error.to_string());
            }
        }
    }

    pub fn log_folder(&self) -> Option<PathBuf> {
        crate::logging::log_dir()
    }

    pub fn open_log_folder(self: &Rc<Self>) {
        let Some(dir) = self.log_folder() else {
            self.alert("Logs unavailable", "No XDG state directory is available.");
            return;
        };
        if let Err(error) = actions::open_folder(&dir) {
            self.alert("Could not open log folder", &error.to_string());
        }
    }

    pub fn refresh_manage(&self) {
        if let Some(manage) = self.manage.borrow().as_ref() {
            manage.refresh();
        }
        if let Some(settings) = self.settings.borrow().as_ref() {
            settings.refresh();
        }
    }

    fn update_tray(&self) {
        if let Some(tray) = self.tray.borrow().as_ref() {
            let enabled = self.config.borrow().preferences.launch_at_login;
            let shortcut = self.display_shortcut();
            let projects_shortcut = self.display_projects_shortcut();
            tray.update(|tray| {
                tray.autostart = enabled;
                tray.shortcut = shortcut;
                tray.projects_shortcut = projects_shortcut;
            });
        }
    }

    fn save(&self) -> bool {
        match self.store.save(&self.config.borrow()) {
            Ok(()) => true,
            Err(error) => {
                self.alert("Could not save configuration", &error.to_string());
                false
            }
        }
    }

    pub fn alert(&self, title: &str, body: &str) {
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message(title)
            .detail(body)
            .build();
        let parent = self
            .app
            .borrow()
            .as_ref()
            .and_then(|app| app.active_window());
        dialog.show(parent.as_ref());
    }

    pub fn notify(&self, title: &str, body: &str) {
        let title = title.to_owned();
        let body = body.to_owned();
        std::thread::spawn(move || {
            if let Err(error) = notify_rust::Notification::new()
                .summary(&title)
                .body(&body)
                .appname("RadialDeck")
                .icon("io.github.raihan.RadialDeck")
                .timeout(notify_rust::Timeout::Milliseconds(4000))
                .show()
            {
                tracing::warn!("notification failed: {error}");
            }
        });
    }

    pub fn quit(&self) {
        if let Some(app) = self.app.borrow().as_ref() {
            app.quit();
        }
    }

    /// Applies one command line invocation. Called for the first launch and
    /// for later invocations forwarded to the running instance.
    pub fn handle_cli(self: &Rc<Self>, cli: Cli) {
        tracing::debug!("handle cli: files={:?} mode={:?}", cli.files, cli.mode);
        self.cli_handled.set(true);
        if cli.manage {
            self.open_manage();
        } else if cli.settings {
            self.open_settings();
        } else if cli.projects {
            self.show_launcher();
        } else if !cli.files.is_empty() || cli.mode.is_some() {
            self.show_actions_mode(cli.files, cli.mode.unwrap_or(WheelMode::Convert));
        } else if cli.show || !cli.hidden {
            self.show_actions(Vec::new());
        }
    }

    pub fn handle_activate(self: &Rc<Self>) {
        if self.cli_handled.replace(false) {
            return;
        }
        // Gaining focus while an overlay is already open must not reset the
        // dropped files (a plain click would otherwise clear the selection).
        let actions_visible = self
            .action_launcher
            .borrow()
            .as_ref()
            .is_some_and(ActionLauncher::is_visible);
        let folders_visible = self
            .launcher
            .borrow()
            .as_ref()
            .is_some_and(Launcher::is_visible);
        if actions_visible || folders_visible {
            return;
        }
        self.show_actions(Vec::new());
    }

    pub fn show_actions(self: &Rc<Self>, files: Vec<PathBuf>) {
        self.show_actions_mode(files, WheelMode::Convert);
    }

    pub fn show_actions_mode(self: &Rc<Self>, files: Vec<PathBuf>, mode: WheelMode) {
        self.hide_launcher();
        tracing::debug!("show actions raw files={files:?}");
        let files = normalize_paths(files);
        tracing::debug!("show actions normalized={files:?}");
        if let Some(category) = files.first().and_then(|file| category_for(file)) {
            self.action_category.set(category);
        }
        // An empty invocation (hotkey or tray) starts from a clean slate; only
        // an explicit drop or file-manager launch seeds sources.
        *self.action_sources.borrow_mut() = files;
        tracing::debug!(
            "show actions: mode={mode:?} sources={:?}",
            self.action_sources.borrow()
        );
        self.action_mode.set(mode);
        self.action_tool.replace(None);
        self.action_status.replace(None);
        if mode == WheelMode::Actions {
            *self.airdrop_devices.borrow_mut() = actions::kdeconnect_devices();
        }

        let recreate = self
            .action_launcher
            .borrow()
            .as_ref()
            .is_none_or(ActionLauncher::is_destroyed);
        if recreate {
            *self.action_launcher.borrow_mut() = None;
            let Some(app) = self.app.borrow().clone() else {
                return;
            };
            let controller = Rc::downgrade(self);
            let handler = Rc::new(move |request: ActionRequest| {
                if let Some(controller) = controller.upgrade() {
                    controller.handle_action_request(request);
                }
            });
            *self.action_launcher.borrow_mut() = Some(ActionLauncher::new(&app, handler));
        }
        self.refresh_actions();
        if let Some(launcher) = self.action_launcher.borrow().as_ref() {
            launcher.present();
        }
    }

    pub fn hide_actions(&self) {
        if let Some(launcher) = self.action_launcher.borrow().as_ref() {
            launcher.hide();
        }
    }

    pub fn toggle_actions(self: &Rc<Self>) {
        let visible = self
            .action_launcher
            .borrow()
            .as_ref()
            .is_some_and(ActionLauncher::is_visible);
        tracing::debug!("toggle actions: visible={visible}");
        if visible {
            self.hide_actions();
        } else {
            self.show_actions_mode(Vec::new(), self.action_mode.get());
        }
    }

    fn action_view(&self) -> ActionView {
        let sources = self.action_sources.borrow().clone();
        let category = self.action_category.get();
        let mode = self.action_mode.get();
        let matching: Vec<&PathBuf> = sources
            .iter()
            .filter(|path| category_for(path) == Some(category))
            .collect();
        let source = matching
            .first()
            .map(|path| path.as_path())
            .or_else(|| sources.first().map(PathBuf::as_path));

        let targets = match mode {
            WheelMode::Convert => {
                deck_core::convert::registry::targets(category, source, &self.tools)
                    .into_iter()
                    .map(|target| WheelTarget {
                        available: target.is_available(),
                        id: target.id.clone(),
                        label: target.label.clone(),
                        reason: target.reason.clone(),
                    })
                    .collect::<Vec<_>>()
            }
            WheelMode::Tools => {
                let matching_paths = matching
                    .iter()
                    .map(|path| (*path).clone())
                    .collect::<Vec<_>>();
                ToolId::available_for(category)
                    .iter()
                    .map(|id| {
                        let reason = tools::availability(*id, category, &matching_paths);
                        WheelTarget {
                            available: reason.is_none(),
                            id: id.id().to_owned(),
                            label: id.short_label().to_owned(),
                            reason,
                        }
                    })
                    .collect()
            }
            WheelMode::Actions => {
                let devices = self.airdrop_devices.borrow();
                let airdrop_reason = if sources.is_empty() {
                    Some("Drop files first".to_owned())
                } else if devices.is_empty() {
                    Some("No reachable KDE Connect device".to_owned())
                } else {
                    None
                };
                [
                    ("copy", "Copy", "Copy files to the clipboard", None),
                    ("move", "Move", "Move files to a folder", None),
                    ("copy_path", "Copy Path", "Copy absolute paths", None),
                    ("terminal", "Terminal", "Open a terminal at the files", None),
                    ("airdrop", "Send", "Send with KDE Connect", airdrop_reason),
                    ("float", "Float", "Keep files on a floating shelf", None),
                ]
                .into_iter()
                .map(|(id, label, _description, reason)| {
                    let reason = if sources.is_empty() {
                        Some("Drop files first".to_owned())
                    } else {
                        reason
                    };
                    WheelTarget {
                        available: reason.is_none(),
                        id: id.to_owned(),
                        label: label.to_owned(),
                        reason,
                    }
                })
                .collect::<Vec<_>>()
            }
        };

        // Actions apply to every dropped file, including ones no converter
        // recognises, so the hub counts the selection rather than matches.
        let label_count = if mode == WheelMode::Actions || matching.is_empty() {
            sources.len()
        } else {
            matching.len()
        };
        let label_source = matching
            .first()
            .map(|path| path.as_path())
            .or_else(|| sources.first().map(PathBuf::as_path));
        let source_label = match (label_count, label_source) {
            (0, _) => String::new(),
            (1, Some(path)) => deck_core::convert::registry::extension(path)
                .map(|ext| ext.to_uppercase())
                .unwrap_or_else(|| "FILE".into()),
            (count, _) => format!("{count} FILES"),
        };

        let form = if mode == WheelMode::Tools {
            self.action_tool.borrow().and_then(|id| {
                let fields = tools::fields(id, category);
                (!fields.is_empty()).then(|| FormView {
                    tool: id,
                    title: format!("{} · {}", id.label(), category.label()),
                    fields,
                })
            })
        } else {
            None
        };

        let (title, empty_hint) = match mode {
            WheelMode::Convert => (
                category.heading().to_owned(),
                if sources.is_empty() {
                    "Drop files".into()
                } else if matching.is_empty() {
                    "Switch to a matching tab".into()
                } else {
                    "Choose a format".into()
                },
            ),
            WheelMode::Tools => (
                format!("{} tools", category.label()),
                if sources.is_empty() {
                    "Drop files".into()
                } else if matching.is_empty() {
                    "Switch to a matching tab".into()
                } else if targets.is_empty() {
                    "No quick tools here — use Convert".into()
                } else {
                    "Choose a tool".into()
                },
            ),
            WheelMode::Actions => (
                "File actions".into(),
                if sources.is_empty() {
                    "Drop files".into()
                } else {
                    "Choose an action".into()
                },
            ),
        };

        let status = self.action_status.borrow().clone().or_else(|| {
            (mode != WheelMode::Actions && !sources.is_empty() && matching.is_empty())
                .then(|| "These files do not match the selected category.".to_owned())
        });

        ActionView {
            category,
            mode,
            title,
            source_label,
            targets,
            shortcut: mode.key_label().to_owned(),
            global_shortcut: format!("{} opens this wheel", self.display_shortcut()),
            status,
            empty_hint,
            form,
        }
    }

    fn apply_action_mode(&self, mode: WheelMode) {
        self.action_mode.set(mode);
        self.action_tool.replace(None);
        self.action_status.replace(None);
        if mode == WheelMode::Actions {
            *self.airdrop_devices.borrow_mut() = actions::kdeconnect_devices();
        }
        self.refresh_actions();
    }

    fn refresh_actions(&self) {
        if let Some(launcher) = self.action_launcher.borrow().as_ref() {
            launcher.render(self.action_view());
        }
    }

    fn handle_action_request(self: &Rc<Self>, request: ActionRequest) {
        match request {
            ActionRequest::Category(category) => {
                self.action_category.set(category);
                self.action_tool.replace(None);
                self.action_status.replace(None);
                self.refresh_actions();
            }
            ActionRequest::Mode(mode) => {
                self.apply_action_mode(mode);
            }
            ActionRequest::LiveMode(mode) => {
                if !self.config.borrow().preferences.live_modifier {
                    return;
                }
                // A modifier key is a momentary gesture: switching modes must
                // keep the seeded sources, never reset them to an empty slate.
                let sources = self.action_sources.borrow().clone();
                tracing::info!("live-modifier switch to {mode:?}");
                self.apply_action_mode(mode);
                *self.action_sources.borrow_mut() = sources;
            }
            ActionRequest::Dropped(paths) => {
                let files = normalize_paths(paths);
                if files.is_empty() {
                    return;
                }
                if let Some(category) = category_for(&files[0]) {
                    self.action_category.set(category);
                }
                *self.action_sources.borrow_mut() = files;
                self.action_tool.replace(None);
                self.action_status.replace(None);
                self.refresh_actions();
            }
            ActionRequest::Run(target) => match self.action_mode.get() {
                WheelMode::Convert => self.run_conversion(&target),
                WheelMode::Tools => self.start_tool(&target),
                WheelMode::Actions => self.run_file_action(&target),
            },
            ActionRequest::ToolRun { id, values } => {
                self.action_tool.replace(None);
                self.execute_tool(id, values);
            }
            ActionRequest::ToolCancel => {
                self.action_tool.replace(None);
                self.refresh_actions();
            }
            ActionRequest::Unavailable(reason) => {
                self.action_status.replace(Some(reason));
                self.refresh_actions();
            }
            ActionRequest::Dismiss => self.hide_actions(),
        }
    }

    fn matching_sources(&self, category: Category) -> Vec<PathBuf> {
        self.action_sources
            .borrow()
            .iter()
            .filter(|path| category_for(path) == Some(category))
            .cloned()
            .collect()
    }

    fn start_tool(self: &Rc<Self>, id: &str) {
        let Some(id) = ToolId::from_id(id) else {
            return;
        };
        let category = self.action_category.get();
        let sources = self.matching_sources(category);
        if let Some(reason) = tools::availability(id, category, &sources) {
            self.action_status.replace(Some(reason));
            self.refresh_actions();
            return;
        }
        if tools::fields(id, category).is_empty() {
            self.execute_tool(id, BTreeMap::new());
        } else {
            self.action_tool.replace(Some(id));
            self.action_status.replace(None);
            self.refresh_actions();
        }
    }

    fn execute_tool(self: &Rc<Self>, id: ToolId, values: BTreeMap<String, String>) {
        if self.converting.get() {
            self.action_status
                .replace(Some("Another operation is already running".into()));
            self.refresh_actions();
            return;
        }
        let category = self.action_category.get();
        let sources = self.matching_sources(category);
        if let Some(reason) = tools::availability(id, category, &sources) {
            self.action_status.replace(Some(reason));
            self.refresh_actions();
            return;
        }
        self.converting.set(true);
        self.hide_actions();
        tracing::info!("tool {} on {} file(s)", id.id(), sources.len());

        let request = ToolRequest {
            id: Some(id),
            sources,
            values,
        };
        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let outcome = tools::execute(&request);
            let _ = sender.send_blocking(outcome);
        });
        let controller = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let outcome = receiver.recv().await;
            if let Some(controller) = controller.upgrade() {
                controller.converting.set(false);
                match outcome {
                    Ok(outcome) => controller.report_tool(id, &outcome),
                    Err(_) => controller.notify(
                        "Tool failed",
                        "The tool worker stopped unexpectedly. Check the log.",
                    ),
                }
            }
        });
    }

    fn report_tool(&self, id: ToolId, outcome: &ToolOutcome) {
        let created = outcome.created.len();
        let failed = outcome.failures.len();
        tracing::info!(
            "tool {} finished: {created} created, {failed} failed",
            id.id()
        );
        if created > 0 {
            self.record_history(id.label(), outcome.created.clone());
        }
        if failed == 0 && created > 0 {
            self.notify(
                id.label(),
                &format!(
                    "{} file{} created next to the original{}.",
                    created,
                    if created == 1 { "" } else { "s" },
                    if created == 1 { "" } else { "s" }
                ),
            );
            return;
        }
        if let Some(failure) = outcome.failures.first() {
            let name = failure
                .source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| failure.source.display().to_string());
            self.notify(id.label(), &format!("{name}: {}", failure.message));
        } else if let Some(message) = &outcome.message {
            self.notify(id.label(), message);
        } else {
            self.notify(id.label(), "No files were produced.");
        }
    }

    fn record_history(&self, label: &str, created: Vec<PathBuf>) {
        let path = deck_core::history::default_history_path();
        let mut history = self.history.borrow_mut();
        let sources = self.action_sources.borrow().clone();
        if history.record(label, sources, created).is_some() {
            if let Err(error) = history.save(&path) {
                tracing::warn!("could not save history: {error}");
            }
            self.refresh_tray();
        }
    }

    pub fn undo_operation(self: &Rc<Self>, id: u64) {
        let path = deck_core::history::default_history_path();
        let mut history = self.history.borrow_mut();
        let Some(report) = history.undo(id) else {
            return;
        };
        if let Err(error) = history.save(&path) {
            tracing::warn!("could not save history: {error}");
        }
        drop(history);
        self.refresh_tray();
        if report.is_clean() {
            self.notify(
                "Undone",
                &format!(
                    "{} item{} removed.",
                    report.removed.len(),
                    if report.removed.len() == 1 { "" } else { "s" }
                ),
            );
        } else {
            let (path, error) = &report.failures[0];
            self.notify(
                "Could not undo everything",
                &format!("{}: {error}", path.display()),
            );
        }
    }

    fn refresh_tray(&self) {
        let recent = self.history.borrow().recent(5).to_vec();
        if let Some(handle) = self.tray.borrow().as_ref() {
            handle.update(move |tray: &mut Tray| tray.set_recent(recent));
        }
    }

    fn run_file_action(self: &Rc<Self>, action: &str) {
        let sources = self.action_sources.borrow().clone();
        if sources.is_empty() {
            return;
        }
        match action {
            "copy" => {
                self.hide_actions();
                match actions::copy_files_to_clipboard(&sources) {
                    Ok(()) => self.notify(
                        "Copied",
                        &format!(
                            "{} file{} on the clipboard.",
                            sources.len(),
                            if sources.len() == 1 { "" } else { "s" }
                        ),
                    ),
                    Err(error) => self.notify("Could not copy files", &error),
                }
            }
            "copy_path" => {
                self.hide_actions();
                actions::copy_paths_to_clipboard(&sources);
                self.notify(
                    "Paths copied",
                    &format!(
                        "{} path{} on the clipboard.",
                        sources.len(),
                        if sources.len() == 1 { "" } else { "s" }
                    ),
                );
            }
            "terminal" => {
                let directory = deck_core::file_actions::common_parent(&sources)
                    .or_else(|| std::env::current_dir().ok());
                let Some(directory) = directory else {
                    self.notify("Terminal unavailable", "No folder could be resolved.");
                    return;
                };
                self.hide_actions();
                let terminal = self.config.borrow().preferences.terminal.clone();
                if let Err(error) = actions::open_terminal(&directory, terminal.as_deref()) {
                    self.notify("Could not open terminal", &error.to_string());
                }
            }
            "move" => self.choose_move_destination(&sources),
            "airdrop" => self.choose_airdrop_device(&sources),
            "float" => {
                self.hide_actions();
                self.show_float(&sources);
            }
            _ => {}
        }
    }

    fn choose_move_destination(self: &Rc<Self>, sources: &[PathBuf]) {
        let Some(window) = self
            .action_launcher
            .borrow()
            .as_ref()
            .map(ActionLauncher::window)
            .cloned()
        else {
            return;
        };
        self.hide_actions();
        let start = deck_core::file_actions::common_parent(sources)
            .or_else(home_dir)
            .map(gtk::gio::File::for_path);
        let dialog = gtk::FileDialog::builder()
            .title("Move to folder")
            .modal(true)
            .build();
        if let Some(start) = start {
            dialog.set_initial_folder(Some(&start));
        }
        let controller = Rc::downgrade(self);
        let sources = sources.to_vec();
        dialog.select_folder(
            Some(&window),
            None::<&gtk::gio::Cancellable>,
            move |result| {
                let Some(controller) = controller.upgrade() else {
                    return;
                };
                let Ok(folder) = result else {
                    return;
                };
                let Some(destination) = folder.path() else {
                    controller.notify("Move failed", "The selected folder is not local.");
                    return;
                };
                controller.move_async(&sources, destination);
            },
        );
    }

    fn move_async(self: &Rc<Self>, sources: &[PathBuf], destination: PathBuf) {
        if self.converting.get() {
            self.notify("Busy", "Another operation is already running.");
            return;
        }
        self.converting.set(true);
        let sources = sources.to_vec();
        let (sender, receiver) = async_channel::bounded(1);
        let worker_destination = destination.clone();
        std::thread::spawn(move || {
            let outcome = deck_core::file_actions::move_files(&sources, &worker_destination);
            let _ = sender.send_blocking(outcome);
        });
        let controller = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let outcome = receiver.recv().await;
            if let Some(controller) = controller.upgrade() {
                controller.converting.set(false);
                match outcome {
                    Ok(Ok(moved)) => controller.notify(
                        "Moved",
                        &format!(
                            "{} file{} moved to {}.",
                            moved.len(),
                            if moved.len() == 1 { "" } else { "s" },
                            destination.display()
                        ),
                    ),
                    Ok(Err(error)) => controller.notify("Could not move files", &error.to_string()),
                    Err(_) => controller.notify("Could not move files", "The worker stopped."),
                }
            }
        });
    }

    fn choose_airdrop_device(self: &Rc<Self>, sources: &[PathBuf]) {
        let devices = self.airdrop_devices.borrow().clone();
        if devices.is_empty() {
            self.notify(
                "KDE Connect",
                "No reachable device. Pair one in KDE Connect first.",
            );
            return;
        }
        let Some(window) = self
            .action_launcher
            .borrow()
            .as_ref()
            .map(ActionLauncher::window)
            .cloned()
        else {
            return;
        };
        self.hide_actions();
        let mut buttons = devices
            .iter()
            .map(|device| format!("Send to {}", device.name))
            .collect::<Vec<_>>();
        buttons.push("Cancel".into());
        let labels = buttons.iter().map(String::as_str).collect::<Vec<_>>();
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message("Send with KDE Connect")
            .detail(format!(
                "{} file{}",
                sources.len(),
                if sources.len() == 1 { "" } else { "s" }
            ))
            .buttons(labels)
            .build();
        let controller = Rc::downgrade(self);
        let sources = sources.to_vec();
        dialog.choose(
            Some(&window),
            None::<&gtk::gio::Cancellable>,
            move |result| {
                let Ok(index) = result else {
                    return;
                };
                let Some(device) = devices.get(index as usize).cloned() else {
                    return;
                };
                let Some(controller) = controller.upgrade() else {
                    return;
                };
                controller.share_async(sources, device);
            },
        );
    }

    fn share_async(self: &Rc<Self>, sources: Vec<PathBuf>, device: actions::KdeDevice) {
        let (sender, receiver) = async_channel::bounded(1);
        let id = device.id.clone();
        std::thread::spawn(move || {
            let outcome = actions::share_via_kdeconnect(&sources, &id);
            let _ = sender.send_blocking(outcome);
        });
        let controller = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let outcome = receiver.recv().await;
            if let Some(controller) = controller.upgrade() {
                match outcome {
                    Ok(Ok(())) => {
                        controller.notify("Sent", &format!("Files sent to {}.", device.name))
                    }
                    Ok(Err(error)) => controller.notify("KDE Connect failed", &error),
                    Err(_) => controller.notify("KDE Connect failed", "The worker stopped."),
                }
            }
        });
    }

    fn show_float(self: &Rc<Self>, sources: &[PathBuf]) {
        let recreate = self
            .float
            .borrow()
            .as_ref()
            .is_none_or(|shelf| shelf.is_destroyed());
        if recreate {
            *self.float.borrow_mut() = None;
            let Some(app) = self.app.borrow().clone() else {
                return;
            };
            *self.float.borrow_mut() = Some(FloatShelf::new(&app));
        }
        if let Some(shelf) = self.float.borrow().as_ref() {
            shelf.add(sources.iter().cloned());
            shelf.present();
        }
    }

    fn run_conversion(self: &Rc<Self>, target_id: &str) {
        tracing::debug!(
            "run requested: {target_id}, sources={}",
            self.action_sources.borrow().len()
        );
        if self.converting.get() {
            self.action_status
                .replace(Some("A conversion is already running".into()));
            self.refresh_actions();
            return;
        }
        let category = self.action_category.get();
        let sources: Vec<PathBuf> = self
            .action_sources
            .borrow()
            .iter()
            .filter(|path| category_for(path) == Some(category))
            .cloned()
            .collect();
        if sources.is_empty() {
            self.action_status.replace(Some(
                "No files match this category. Switch tabs or drop different files.".into(),
            ));
            self.refresh_actions();
            return;
        }
        let targets = deck_core::convert::registry::targets(
            category,
            sources.first().map(PathBuf::as_path),
            &self.tools,
        );
        let Some(target) = targets.iter().find(|target| target.id == target_id) else {
            return;
        };
        if let Some(reason) = &target.reason {
            self.action_status.replace(Some(reason.clone()));
            self.refresh_actions();
            return;
        }

        let target_id = target_id.to_owned();
        self.converting.set(true);
        self.hide_actions();
        tracing::info!(
            "converting {} file(s) from {:?} to {}",
            sources.len(),
            category,
            target_id
        );

        let (sender, receiver) = async_channel::bounded(1);
        let thread_sources = sources.clone();
        let tools = self.tools;
        let thread_target = target_id.clone();
        std::thread::spawn(move || {
            let outcome = deck_core::convert(&thread_sources, category, &thread_target, &tools);
            let _ = sender.send_blocking(outcome);
        });

        let controller = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let outcome = receiver.recv().await;
            if let Some(controller) = controller.upgrade() {
                controller.converting.set(false);
                match outcome {
                    Ok(outcome) => controller.report_conversion(&target_id, &outcome),
                    Err(_) => {
                        tracing::error!("conversion worker stopped without a result");
                        controller.notify(
                            "Conversion failed",
                            "The converter stopped unexpectedly. Check the log for details.",
                        );
                    }
                }
            }
        });
    }

    fn report_conversion(&self, target_id: &str, outcome: &deck_core::Outcome) {
        let created = outcome.created.len();
        let failed = outcome.failures.len();
        tracing::info!("conversion finished: {created} created, {failed} failed");
        if created > 0 {
            self.record_history(target_id, outcome.created.clone());
        }
        if created == 0 && failed == 0 {
            self.notify("Nothing converted", "No files matched the selected format.");
            return;
        }
        if failed == 0 {
            self.notify(
                "Conversion complete",
                &format!(
                    "{} file{} written next to the original{}.",
                    created,
                    if created == 1 { "" } else { "s" },
                    if created == 1 { "" } else { "s" }
                ),
            );
            return;
        }
        let first = &outcome.failures[0];
        let name = first
            .source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| first.source.display().to_string());
        let summary = if created == 0 {
            format!("Could not convert {name}: {}", first.message)
        } else {
            format!(
                "{created} written, {failed} failed. {name}: {}",
                first.message
            )
        };
        self.notify("Conversion issues", &summary);
    }
}

fn category_for(path: &std::path::Path) -> Option<Category> {
    if path.is_dir() {
        Some(Category::Archives)
    } else {
        Category::of(path)
    }
}

fn normalize_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let cwd = std::env::current_dir().ok();
    paths
        .into_iter()
        .filter(|path| {
            if path.is_absolute() {
                path.exists()
            } else {
                cwd.as_ref()
                    .map(|cwd| cwd.join(path).exists())
                    .unwrap_or(false)
            }
        })
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.as_ref().map(|cwd| cwd.join(&path)).unwrap_or(path)
            }
        })
        .collect()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn next_color(controller: &Controller) -> String {
    const PALETTE: [&str; 8] = [
        "#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#a855f7", "#06b6d4", "#ec4899", "#84cc16",
    ];
    let count = controller.config.borrow().projects.len();
    PALETTE[count % PALETTE.len()].to_string()
}
