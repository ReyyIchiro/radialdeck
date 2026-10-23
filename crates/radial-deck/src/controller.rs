use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use deck_core::{Config, FolderError, Store, StoreError, Theme};
use gtk::glib;
use gtk::prelude::*;
use uuid::Uuid;

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
    hotkey: RefCell<Option<HotkeyService>>,
    tray: RefCell<Option<ksni::blocking::Handle<Tray>>>,
    hold_guard: RefCell<Option<gtk::gio::ApplicationHoldGuard>>,
    pub hotkey_state: RefCell<HotkeyState>,
    activated: std::cell::Cell<bool>,
    startup_notice: RefCell<Option<String>>,
    fatal_error: RefCell<Option<String>>,
    flags: Cli,
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
            hotkey: RefCell::new(None),
            tray: RefCell::new(None),
            hold_guard: RefCell::new(None),
            hotkey_state: RefCell::new(HotkeyState::default()),
            activated: std::cell::Cell::new(false),
            startup_notice: RefCell::new(startup_notice),
            fatal_error: RefCell::new(fatal_error),
            flags,
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
                    TrayCommand::Show => controller.show_launcher(),
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
                    HotkeyEvent::Activated => controller.toggle_launcher(),
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

    pub fn toggle_launcher(self: &Rc<Self>) {
        let visible = self
            .launcher
            .borrow()
            .as_ref()
            .is_some_and(|launcher| launcher.is_visible());
        if visible {
            self.hide_launcher();
        } else {
            self.show_launcher();
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
                .icon("folder")
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

    pub fn handle_activate(self: &Rc<Self>) {
        if self.activated.replace(true) {
            self.open_manage();
            return;
        }
        if self.flags.manage {
            self.open_manage();
        } else if self.flags.settings {
            self.open_settings();
        } else if self.flags.show {
            self.show_launcher();
        } else if !self.flags.hidden {
            self.open_manage();
        }
    }
}

fn next_color(controller: &Controller) -> String {
    const PALETTE: [&str; 8] = [
        "#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#a855f7", "#06b6d4", "#ec4899", "#84cc16",
    ];
    let count = controller.config.borrow().projects.len();
    PALETTE[count % PALETTE.len()].to_string()
}
