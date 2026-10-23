use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use deck_core::{Category, Config, FolderError, Store, StoreError, Theme, Tools};
use gtk::glib;
use gtk::prelude::*;
use uuid::Uuid;

use crate::action_launcher::{ActionLauncher, ActionRequest, ActionView};
use crate::action_wheel::WheelTarget;
use crate::actions;
use crate::autostart;
use crate::hotkey::{self, HotkeyEvent, HotkeyService};
use crate::launcher::Launcher;
use crate::manage::ManageWindow;
use crate::settings::SettingsWindow;
use crate::theme;
use crate::tray::{Tray, TrayCommand};
use crate::wheel::{Action, Intent, WheelData, WheelFolder};

#[derive(Debug, Default, Clone)]
pub struct Cli {
    pub hidden: bool,
    pub show: bool,
    pub manage: bool,
    pub settings: bool,
    pub reset: bool,
    pub version: bool,
    pub help: bool,
    pub files: Vec<PathBuf>,
}

impl Cli {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut cli = Self::default();
        for arg in args {
            match arg.as_str() {
                "--hidden" => cli.hidden = true,
                "--show" => cli.show = true,
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
    pub effective: String,
    pub error: Option<String>,
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
    action_category: std::cell::Cell<Category>,
    action_status: RefCell<Option<String>>,
    tools: Tools,
    converting: std::cell::Cell<bool>,
    hotkey: RefCell<Option<HotkeyService>>,
    tray: RefCell<Option<ksni::blocking::Handle<Tray>>>,
    hold_guard: RefCell<Option<gtk::gio::ApplicationHoldGuard>>,
    pub hotkey_state: RefCell<HotkeyState>,
    cli_handled: std::cell::Cell<bool>,
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

        Rc::new(Self {
            app: RefCell::new(None),
            store,
            config: RefCell::new(config),
            launcher: RefCell::new(None),
            manage: RefCell::new(None),
            settings: RefCell::new(None),
            action_launcher: RefCell::new(None),
            action_sources: RefCell::new(Vec::new()),
            action_category: std::cell::Cell::new(Category::Images),
            action_status: RefCell::new(None),
            tools: Tools::detect(),
            converting: std::cell::Cell::new(false),
            hotkey: RefCell::new(None),
            tray: RefCell::new(None),
            hold_guard: RefCell::new(None),
            hotkey_state: RefCell::new(HotkeyState::default()),
            cli_handled: std::cell::Cell::new(false),
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
        );
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
                    TrayCommand::Quit => controller.quit(),
                }
            }
        });
    }

    fn start_hotkey(self: &Rc<Self>) {
        let (tx, rx) = async_channel::unbounded();
        let preferred = self.config.borrow().preferences.hotkey.clone();
        let service = hotkey::start(preferred, tx);
        *self.hotkey.borrow_mut() = Some(service);

        let controller = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                let Some(controller) = controller.upgrade() else {
                    break;
                };
                match event {
                    HotkeyEvent::Activated => controller.toggle_actions(),
                    HotkeyEvent::Bound(description) => {
                        controller.hotkey_state.borrow_mut().effective = description.clone();
                        controller.hotkey_state.borrow_mut().error = None;
                        if let Some(settings) = controller.settings.borrow().as_ref() {
                            settings.refresh();
                        }
                        controller.update_tray();
                        tracing::info!("global shortcut active: {description}");
                    }
                    HotkeyEvent::Error(error) => {
                        controller.hotkey_state.borrow_mut().error = Some(error.clone());
                        if let Some(settings) = controller.settings.borrow().as_ref() {
                            settings.refresh();
                        }
                        tracing::warn!("global shortcut error: {error}");
                    }
                }
            }
        });
    }

    pub fn display_shortcut(&self) -> String {
        let state = self.hotkey_state.borrow();
        if state.effective.is_empty() {
            deck_core::config::pretty_hotkey(&self.config.borrow().preferences.hotkey)
        } else {
            state.effective.clone()
        }
    }

    pub fn hotkey_error(&self) -> Option<String> {
        self.hotkey_state.borrow().error.clone()
    }

    pub fn configure_hotkey(&self) {
        if let Some(service) = self.hotkey.borrow().as_ref() {
            service.configure();
        }
    }

    pub fn wheel_data(&self) -> WheelData {
        let config = self.config.borrow();
        let project = config.active_project();
        let index = config
            .projects
            .iter()
            .position(|candidate| candidate.id == project.id)
            .unwrap_or(0);
        WheelData {
            project_name: project.name.clone(),
            project_color: project.color.clone(),
            project_total: config.projects.len(),
            project_index: index,
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
        }
    }

    pub fn refresh_launcher(&self) {
        if let Some(launcher) = self.launcher.borrow().as_ref() {
            launcher.set_data(self.wheel_data());
        }
    }

    pub fn show_launcher(self: &Rc<Self>) {
        if self.launcher.borrow().is_none() {
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

    pub fn open_manage(self: &Rc<Self>) {
        self.hide_launcher();
        if self.manage.borrow().is_none() {
            let Some(app) = self.app.borrow().clone() else {
                return;
            };
            *self.manage.borrow_mut() = Some(ManageWindow::new(&app, Rc::downgrade(self)));
        }
        if let Some(manage) = self.manage.borrow().as_ref() {
            manage.refresh();
            manage.present();
        }
    }

    pub fn open_settings(self: &Rc<Self>) {
        self.hide_launcher();
        if self.settings.borrow().is_none() {
            let Some(app) = self.app.borrow().clone() else {
                return;
            };
            *self.settings.borrow_mut() = Some(SettingsWindow::new(&app, Rc::downgrade(self)));
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
            Action::CycleProject(offset) => {
                self.config.borrow_mut().cycle_project(offset);
                self.save();
                self.refresh_launcher();
            }
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
            tray.update(|tray| {
                tray.autostart = enabled;
                tray.shortcut = shortcut;
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
        self.cli_handled.set(true);
        if cli.manage {
            self.open_manage();
        } else if cli.settings {
            self.open_settings();
        } else if !cli.files.is_empty() {
            self.show_actions(cli.files);
        } else if cli.show || !cli.hidden {
            self.show_actions(Vec::new());
        }
    }

    pub fn handle_activate(self: &Rc<Self>) {
        if self.cli_handled.replace(false) {
            return;
        }
        // Gaining focus while the wheel is already open must not reset the
        // dropped files (a plain click would otherwise clear the selection).
        let visible = self
            .action_launcher
            .borrow()
            .as_ref()
            .is_some_and(ActionLauncher::is_visible);
        if visible {
            return;
        }
        self.show_actions(Vec::new());
    }

    pub fn show_actions(self: &Rc<Self>, files: Vec<PathBuf>) {
        let files = normalize_paths(files);
        if let Some(category) = files.first().and_then(|file| category_for(file)) {
            self.action_category.set(category);
        }
        // An empty invocation (hotkey or tray) starts from a clean slate; only
        // an explicit drop or file-manager launch seeds sources.
        *self.action_sources.borrow_mut() = files;
        self.action_status.replace(None);

        if self.action_launcher.borrow().is_none() {
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
            self.show_actions(Vec::new());
        }
    }

    fn action_view(&self) -> ActionView {
        let sources = self.action_sources.borrow().clone();
        let category = self.action_category.get();
        let matching: Vec<&PathBuf> = sources
            .iter()
            .filter(|path| category_for(path) == Some(category))
            .collect();
        let source = matching
            .first()
            .map(|path| path.as_path())
            .or_else(|| sources.first().map(PathBuf::as_path));
        let targets = deck_core::convert::registry::targets(category, source, &self.tools)
            .into_iter()
            .map(|target| WheelTarget {
                available: target.is_available(),
                id: target.id.clone(),
                label: target.label.clone(),
                reason: target.reason.clone(),
            })
            .collect();

        // The hub shows the dropped source even when the selected tab does not
        // match it, so switching tabs never looks like an empty drop.
        let label_source = matching
            .first()
            .map(|path| path.as_path())
            .or_else(|| sources.first().map(PathBuf::as_path));
        let label_count = if matching.is_empty() {
            sources.len()
        } else {
            matching.len()
        };
        let source_label = match (label_count, label_source) {
            (0, _) => String::new(),
            (1, Some(path)) => deck_core::convert::registry::extension(path)
                .map(|ext| ext.to_uppercase())
                .unwrap_or_else(|| "FILE".into()),
            (count, _) => format!("{count} FILES"),
        };

        ActionView {
            category,
            title: category.heading().into(),
            source_label,
            targets,
            shortcut: self.display_shortcut(),
            status: self.action_status.borrow().clone().or_else(|| {
                (!sources.is_empty() && matching.is_empty())
                    .then(|| "This tab has no formats for the dropped files.".to_owned())
            }),
            empty_hint: if sources.is_empty() {
                "Drop files".into()
            } else if matching.is_empty() {
                "Switch to a matching tab".into()
            } else {
                "Choose a format".into()
            },
        }
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
                self.action_status.replace(None);
                self.refresh_actions();
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
                self.action_status.replace(None);
                self.refresh_actions();
            }
            ActionRequest::Run(target) => self.run_conversion(&target),
            ActionRequest::Unavailable(reason) => {
                self.action_status.replace(Some(reason));
                self.refresh_actions();
            }
            ActionRequest::Dismiss => self.hide_actions(),
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
                    Ok(outcome) => controller.report_conversion(&outcome),
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

    fn report_conversion(&self, outcome: &deck_core::Outcome) {
        let created = outcome.created.len();
        let failed = outcome.failures.len();
        tracing::info!("conversion finished: {created} created, {failed} failed");
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

fn next_color(controller: &Controller) -> String {
    const PALETTE: [&str; 8] = [
        "#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#a855f7", "#06b6d4", "#ec4899", "#84cc16",
    ];
    let count = controller.config.borrow().projects.len();
    PALETTE[count % PALETTE.len()].to_string()
}
