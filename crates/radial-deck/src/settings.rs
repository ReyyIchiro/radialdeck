use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use deck_core::Theme;
use gtk::pango;
use gtk::prelude::*;

use crate::actions;
use crate::controller::Controller;
use crate::theme;

pub struct SettingsWindow {
    window: gtk::ApplicationWindow,
    quick_shortcut: gtk::Label,
    projects_shortcut: gtk::Label,
    quick_error: gtk::Label,
    projects_error: gtk::Label,
    close_switch: gtk::Switch,
    autostart_switch: gtk::Switch,
    terminal: gtk::DropDown,
    terminal_options: Rc<RefCell<Vec<String>>>,
    theme: gtk::DropDown,
    config_path: gtk::Label,
    log_path: gtk::Label,
    controller: Weak<Controller>,
    suppress: Rc<Cell<bool>>,
}

impl SettingsWindow {
    pub fn new(app: &gtk::Application, controller: Weak<Controller>) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("RadialDeck — Settings")
            .default_width(660)
            .default_height(780)
            .build();
        window.add_css_class("rd-dialog");

        let header = gtk::HeaderBar::new();
        let title = gtk::Label::new(Some("Settings"));
        title.add_css_class("rd-title");
        header.set_title_widget(Some(&title));

        let quick_shortcut = gtk::Label::new(None);
        quick_shortcut.add_css_class("rd-badge");
        let projects_shortcut = gtk::Label::new(None);
        projects_shortcut.add_css_class("rd-badge");

        let quick_error = shortcut_error_label();
        let projects_error = shortcut_error_label();

        let shortcut_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        shortcut_box.append(&setting_title("Shortcuts"));
        shortcut_box.append(&setting_subtitle(
            "Two independent global shortcuts, registered through the desktop portal. The system may ask you to confirm them once.",
        ));
        shortcut_box.append(&shortcut_line(
            "Quick actions",
            "Files, conversions, and archive actions.",
            &quick_shortcut,
        ));
        shortcut_box.append(&quick_error);
        shortcut_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        shortcut_box.append(&shortcut_line(
            "Project folders",
            "Opens the radial project-folder launcher directly.",
            &projects_shortcut,
        ));
        shortcut_box.append(&projects_error);
        let change = gtk::Button::with_label("Change Shortcuts…");
        change.set_halign(gtk::Align::End);
        shortcut_box.append(&change);

        let manage_button = gtk::Button::with_label("Manage…");
        let manage_row = setting_row(
            "Projects & folders",
            "Create projects, rename folders, pick colors, and reorder the wheel.",
            &manage_button,
        );

        let close_switch = gtk::Switch::new();
        close_switch.set_halign(gtk::Align::End);
        close_switch.set_valign(gtk::Align::Center);
        let close_row = setting_row(
            "Close after launching",
            "Hide the launcher as soon as a folder action starts.",
            &close_switch,
        );

        let autostart_switch = gtk::Switch::new();
        autostart_switch.set_halign(gtk::Align::End);
        autostart_switch.set_valign(gtk::Align::Center);
        let autostart_row = setting_row(
            "Launch at login",
            "Start hidden in the tray when you sign in (XDG autostart).",
            &autostart_switch,
        );

        let terminal = gtk::DropDown::from_strings(&["Auto-detect"]);
        terminal.set_halign(gtk::Align::End);
        terminal.set_valign(gtk::Align::Center);
        let terminal_row = setting_row(
            "Terminal",
            "Used for Ctrl+Enter and Ctrl+Click. Auto-detect prefers Konsole.",
            &terminal,
        );

        let theme_dropdown = gtk::DropDown::from_strings(&["System", "Light", "Dark"]);
        theme_dropdown.set_halign(gtk::Align::End);
        theme_dropdown.set_valign(gtk::Align::Center);
        let theme_row = setting_row(
            "Appearance",
            "System follows your GTK/KDE theme; Light and Dark request a variant.",
            &theme_dropdown,
        );

        let config_path = gtk::Label::new(None);
        config_path.add_css_class("rd-subtle");
        config_path.set_halign(gtk::Align::Start);
        config_path.set_ellipsize(pango::EllipsizeMode::Middle);
        config_path.set_selectable(true);
        let open_folder = gtk::Button::with_label("Open Folder");
        let config_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let config_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        config_labels.set_hexpand(true);
        config_labels.append(&setting_title("Configuration"));
        config_labels.append(&config_path);
        config_row.append(&config_labels);
        config_row.append(&open_folder);

        let log_path = gtk::Label::new(None);
        log_path.add_css_class("rd-subtle");
        log_path.set_halign(gtk::Align::Start);
        log_path.set_ellipsize(pango::EllipsizeMode::Middle);
        log_path.set_selectable(true);
        let open_logs = gtk::Button::with_label("Open Logs");
        let log_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let log_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        log_labels.set_hexpand(true);
        log_labels.append(&setting_title("Diagnostics"));
        log_labels.append(&log_path);
        log_row.append(&log_labels);
        log_row.append(&open_logs);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_halign(gtk::Align::Center);
        content.set_width_request(560);
        content.append(&card(shortcut_box));
        content.append(&card(manage_row));
        content.append(&card(close_row));
        content.append(&card(autostart_row));
        content.append(&card(terminal_row));
        content.append(&card(theme_row));
        content.append(&card(config_row));
        content.append(&card(log_row));

        let footer = gtk::Label::new(Some(&format!(
            "RadialDeck {} · MIT · folders stay on this device",
            env!("CARGO_PKG_VERSION")
        )));
        footer.add_css_class("rd-footer");
        content.append(&footer);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&content)
            .build();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&scrolled);
        window.set_child(Some(&root));

        let settings = Self {
            window,
            quick_shortcut,
            projects_shortcut,
            quick_error,
            projects_error,
            close_switch,
            autostart_switch,
            terminal,
            terminal_options: Rc::new(RefCell::new(Vec::new())),
            theme: theme_dropdown,
            config_path,
            log_path,
            controller: controller.clone(),
            suppress: Rc::new(Cell::new(false)),
        };

        settings.connect_signals(&change, &manage_button, &open_folder, &open_logs);
        settings.refresh();
        settings
    }

    fn connect_signals(
        &self,
        change: &gtk::Button,
        manage: &gtk::Button,
        open_folder: &gtk::Button,
        open_logs: &gtk::Button,
    ) {
        {
            let controller = self.controller.clone();
            change.connect_clicked(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.configure_hotkey();
                }
            });
        }
        {
            let controller = self.controller.clone();
            manage.connect_clicked(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.open_manage();
                }
            });
        }
        {
            let controller = self.controller.clone();
            open_folder.connect_clicked(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.open_config_folder();
                }
            });
        }
        {
            let controller = self.controller.clone();
            open_logs.connect_clicked(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.open_log_folder();
                }
            });
        }
        {
            let controller = self.controller.clone();
            let suppress = Rc::clone(&self.suppress);
            self.close_switch.connect_active_notify(move |switch| {
                if suppress.get() {
                    return;
                }
                if let Some(controller) = controller.upgrade() {
                    controller.set_close_after_launch(switch.is_active());
                }
            });
        }
        {
            let controller = self.controller.clone();
            let suppress = Rc::clone(&self.suppress);
            self.autostart_switch.connect_active_notify(move |switch| {
                if suppress.get() {
                    return;
                }
                if let Some(controller) = controller.upgrade() {
                    controller.set_autostart(switch.is_active());
                }
            });
        }
        {
            let controller = self.controller.clone();
            let suppress = Rc::clone(&self.suppress);
            let terminal_options = Rc::clone(&self.terminal_options);
            self.terminal.connect_selected_notify(move |dropdown| {
                if suppress.get() {
                    return;
                }
                let options = terminal_options.borrow();
                let selected = dropdown.selected() as usize;
                let value = if selected == 0 {
                    None
                } else {
                    options.get(selected).cloned()
                };
                drop(options);
                if let Some(controller) = controller.upgrade() {
                    controller.set_terminal(value);
                }
            });
        }
        {
            let controller = self.controller.clone();
            let suppress = Rc::clone(&self.suppress);
            self.theme.connect_selected_notify(move |dropdown| {
                if suppress.get() {
                    return;
                }
                let theme = match dropdown.selected() {
                    1 => Theme::Light,
                    2 => Theme::Dark,
                    _ => Theme::System,
                };
                if let Some(controller) = controller.upgrade() {
                    controller.set_theme(theme);
                }
            });
        }
        self.window.connect_close_request(|window| {
            window.set_visible(false);
            gtk::glib::Propagation::Stop
        });
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn window(&self) -> &gtk::ApplicationWindow {
        &self.window
    }

    pub fn refresh(&self) {
        let Some(controller) = self.controller.upgrade() else {
            return;
        };
        let (close_after, autostart, preferred_terminal, theme_pref) = {
            let config = controller.config();
            (
                config.preferences.close_after_launch,
                config.preferences.launch_at_login,
                config.preferences.terminal.clone(),
                config.preferences.theme,
            )
        };

        self.suppress.set(true);

        self.quick_shortcut.set_text(&controller.display_shortcut());
        set_shortcut_error(
            &self.quick_error,
            controller
                .hotkey_error_for(crate::hotkey::QUICK_ID)
                .as_deref(),
        );
        self.projects_shortcut
            .set_text(&controller.display_projects_shortcut());
        set_shortcut_error(
            &self.projects_error,
            controller
                .hotkey_error_for(crate::hotkey::PROJECTS_ID)
                .as_deref(),
        );

        self.close_switch.set_active(close_after);
        self.autostart_switch.set_active(autostart);

        let mut options = vec!["Auto-detect".to_string()];
        options.extend(
            actions::available_terminals()
                .into_iter()
                .map(str::to_string),
        );
        if let Some(preferred) = preferred_terminal.as_deref() {
            if !options.iter().any(|option| option == preferred) {
                options.push(preferred.to_string());
            }
        }
        let refs: Vec<&str> = options.iter().map(String::as_str).collect();
        self.terminal.set_model(Some(&gtk::StringList::new(&refs)));
        let selected = preferred_terminal
            .as_deref()
            .and_then(|preferred| options.iter().position(|option| option == preferred))
            .unwrap_or(0);
        self.terminal.set_selected(selected as u32);
        *self.terminal_options.borrow_mut() = options;

        self.theme.set_selected(match theme_pref {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        });
        self.config_path
            .set_text(&controller.config_path().to_string_lossy());
        self.config_path
            .set_tooltip_text(Some(&controller.config_path().to_string_lossy()));
        if let Some(log_dir) = controller.log_folder() {
            self.log_path.set_text(&log_dir.to_string_lossy());
            self.log_path
                .set_tooltip_text(Some(&log_dir.to_string_lossy()));
        } else {
            self.log_path.set_text("Unavailable");
        }

        theme::apply_theme(theme_pref);
        self.suppress.set(false);
    }
}

fn shortcut_line(title: &str, subtitle: &str, badge: &gtk::Label) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.append(&setting_title(title));
    labels.append(&setting_subtitle(subtitle));
    badge.set_valign(gtk::Align::Center);
    row.append(&labels);
    row.append(badge);
    row
}

fn shortcut_error_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("rd-error");
    label.set_halign(gtk::Align::Start);
    label.set_wrap(true);
    label.set_visible(false);
    label
}

fn set_shortcut_error(label: &gtk::Label, error: Option<&str>) {
    match error {
        Some(error) if !error.is_empty() => {
            label.set_text(error);
            label.set_visible(true);
        }
        _ => {
            label.set_text("");
            label.set_visible(false);
        }
    }
}

fn setting_title(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_halign(gtk::Align::Start);
    label
}

fn setting_subtitle(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("rd-subtle");
    label.set_halign(gtk::Align::Start);
    label.set_wrap(true);
    label
}

fn setting_row(title: &str, subtitle: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    labels.append(&setting_title(title));
    labels.append(&setting_subtitle(subtitle));
    row.append(&labels);
    row.append(control);
    row
}

fn card(child: impl IsA<gtk::Widget>) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("rd-card");
    card.append(&child);
    card
}
