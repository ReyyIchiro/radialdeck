use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use deck_core::tools::ToolId;
use deck_core::{OutputLocation, PackageManager, Theme};
use gtk::glib;
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
    live_modifier_switch: gtk::Switch,
    autostart_switch: gtk::Switch,
    terminal: gtk::DropDown,
    terminal_options: Rc<RefCell<Vec<String>>>,
    theme: gtk::DropDown,
    output_mode: gtk::DropDown,
    output_subfolder: gtk::Entry,
    output_folder: gtk::Label,
    choose_output_folder: gtk::Button,
    tool_lists: gtk::Box,
    config_path: gtk::Label,
    log_path: gtk::Label,
    dependencies: gtk::Box,
    dependency_summary: gtk::Label,
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

        let live_modifier_switch = gtk::Switch::new();
        live_modifier_switch.set_halign(gtk::Align::End);
        live_modifier_switch.set_valign(gtk::Align::Center);
        let live_row = setting_row(
            "Live-modifier switching",
            "While the wheel is open, Shift/Ctrl/Super switch Convert/Tools/Actions instantly.",
            &live_modifier_switch,
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

        let output_mode = gtk::DropDown::from_strings(&[
            "Next to the original",
            "In a subfolder",
            "In one folder",
        ]);
        output_mode.set_halign(gtk::Align::End);
        output_mode.set_valign(gtk::Align::Center);
        let output_mode_row = setting_row(
            "Output location",
            "Where tools and conversions write new files. Originals are never overwritten.",
            &output_mode,
        );

        let output_subfolder = gtk::Entry::new();
        output_subfolder.set_placeholder_text(Some("RadialDeck"));
        output_subfolder.set_width_chars(16);
        let output_subfolder_row = setting_row(
            "Subfolder name",
            "Created inside each source's folder when the subfolder mode is active.",
            &output_subfolder,
        );

        let output_folder = gtk::Label::new(None);
        output_folder.add_css_class("rd-subtle");
        output_folder.set_ellipsize(pango::EllipsizeMode::Middle);
        output_folder.set_max_width_chars(28);
        let choose_output_folder = gtk::Button::with_label("Choose…");
        let output_folder_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let output_folder_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        output_folder_labels.set_hexpand(true);
        output_folder_labels.append(&setting_title("Output folder"));
        output_folder_labels.append(&output_folder);
        output_folder_row.append(&output_folder_labels);
        output_folder_row.append(&choose_output_folder);

        let output_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        output_box.append(&output_mode_row);
        output_box.append(&output_subfolder_row);
        output_box.append(&output_folder_row);

        let dependencies = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let dependency_summary = setting_subtitle("Checking installed programs…");
        let dependency_card = gtk::Box::new(gtk::Orientation::Vertical, 10);
        dependency_card.append(&setting_title("Dependencies"));
        dependency_card.append(&setting_subtitle(
            "RadialDeck shells out to programs already on your system. A missing program only disables the tools that need it.",
        ));
        dependency_card.append(&dependency_summary);
        dependency_card.append(&dependencies);
        let recheck = gtk::Button::with_label("Recheck");
        recheck.set_halign(gtk::Align::End);
        dependency_card.append(&recheck);

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
        content.append(&card(live_row));
        content.append(&card(close_row));
        content.append(&card(autostart_row));
        content.append(&card(terminal_row));
        content.append(&card(theme_row));
        content.append(&card(output_box));

        // Tool wheel customisation: one row per category with up/down buttons
        // and a switch that hides the tool. The saved order is the source of
        // truth, so a hidden tool simply leaves the list.
        let tool_section = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let tool_heading = setting_title("Tool wheel");
        let tool_hint = setting_subtitle(
            "Choose which tools appear for each file type and in what order.              Digit shortcuts follow the order shown here.",
        );
        tool_section.append(&tool_heading);
        tool_section.append(&tool_hint);
        let tool_lists = gtk::Box::new(gtk::Orientation::Vertical, 10);
        tool_lists.add_css_class("rd-tool-editor");
        let tool_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(220)
            .child(&tool_lists)
            .build();
        tool_section.append(&tool_scroller);
        content.append(&card(tool_section));
        content.append(&card(dependency_card));
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
            live_modifier_switch,
            autostart_switch,
            terminal,
            terminal_options: Rc::new(RefCell::new(Vec::new())),
            theme: theme_dropdown,
            output_mode,
            output_subfolder,
            output_folder,
            choose_output_folder,
            tool_lists,
            config_path,
            log_path,
            dependencies,
            dependency_summary,
            controller: controller.clone(),
            suppress: Rc::new(Cell::new(false)),
        };

        settings.connect_signals(&change, &manage_button, &open_folder, &open_logs, &recheck);
        settings.refresh();
        settings
    }

    fn connect_signals(
        &self,
        change: &gtk::Button,
        manage: &gtk::Button,
        open_folder: &gtk::Button,
        open_logs: &gtk::Button,
        recheck: &gtk::Button,
    ) {
        {
            let dependencies = self.dependencies.clone();
            let summary = self.dependency_summary.clone();
            let window = self.window.clone();
            recheck.connect_clicked(move |_| {
                populate_dependencies(&dependencies, &summary, &window);
            });
        }
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
            self.live_modifier_switch
                .connect_active_notify(move |witch| {
                    if suppress.get() {
                        return;
                    }
                    if let Some(controller) = controller.upgrade() {
                        controller.set_live_modifier(witch.is_active());
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
            let window = self.window.clone();
            self.output_mode.connect_selected_notify(move |dropdown| {
                if suppress.get() {
                    return;
                }
                let Some(controller) = controller.upgrade() else {
                    return;
                };
                let current = controller.config().preferences.output_location.clone();
                match dropdown.selected() {
                    1 => controller.set_output_location(OutputLocation::Subfolder(
                        controller.config().preferences.output_subfolder.clone(),
                    )),
                    2 => {
                        let dialog = gtk::FileDialog::builder()
                            .title("Choose an output folder")
                            .modal(true)
                            .build();
                        let controller = controller.clone();
                        let window = window.clone();
                        dialog.select_folder(
                            Some(&window),
                            gtk::gio::Cancellable::NONE,
                            move |result| {
                                if let Ok(folder) = result {
                                    if let Some(path) = folder.path() {
                                        controller
                                            .set_output_location(OutputLocation::Folder(path));
                                        return;
                                    }
                                }
                                // Cancelled: restore the previous mode so the UI
                                // never claims a folder that was not chosen.
                                let _ = current;
                                controller.set_output_location(OutputLocation::Alongside);
                            },
                        );
                    }
                    _ => controller.set_output_location(OutputLocation::Alongside),
                }
            });
        }
        {
            // Typing a folder name should not rewrite the config file on every
            // keystroke, so persistence waits for a short pause.
            let controller = self.controller.clone();
            let pending: Rc<Cell<Option<glib::SourceId>>> = Rc::new(Cell::new(None));
            self.output_subfolder.connect_changed(move |entry| {
                let controller = controller.clone();
                let text = entry.text().to_string();
                if let Some(source) = pending.take() {
                    source.remove();
                }
                let slot = Rc::clone(&pending);
                let source = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(500),
                    move || {
                        slot.set(None);
                        if let Some(controller) = controller.upgrade() {
                            controller.set_output_subfolder(&text);
                        }
                    },
                );
                pending.set(Some(source));
            });
        }
        {
            let controller = self.controller.clone();
            let window = self.window.clone();
            self.choose_output_folder.connect_clicked(move |_| {
                let Some(controller) = controller.upgrade() else {
                    return;
                };
                let dialog = gtk::FileDialog::builder()
                    .title("Choose an output folder")
                    .modal(true)
                    .build();
                let controller = controller.clone();
                let window = window.clone();
                dialog.select_folder(Some(&window), gtk::gio::Cancellable::NONE, move |result| {
                    if let Ok(folder) = result {
                        if let Some(path) = folder.path() {
                            controller.set_output_location(OutputLocation::Folder(path));
                        }
                    }
                });
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
        let (
            close_after,
            live_modifier,
            autostart,
            preferred_terminal,
            theme_pref,
            output_location,
            output_subfolder,
        ) = {
            let config = controller.config();
            (
                config.preferences.close_after_launch,
                config.preferences.live_modifier,
                config.preferences.launch_at_login,
                config.preferences.terminal.clone(),
                config.preferences.theme,
                config.preferences.output_location.clone(),
                config.preferences.output_subfolder.clone(),
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
        self.live_modifier_switch.set_active(live_modifier);
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
        self.output_mode.set_selected(match output_location {
            OutputLocation::Alongside => 0,
            OutputLocation::Subfolder(_) => 1,
            OutputLocation::Folder(_) => 2,
        });
        self.output_subfolder.set_text(&output_subfolder);
        if let OutputLocation::Folder(path) = &output_location {
            self.output_folder.set_text(&path.to_string_lossy());
        } else {
            self.output_folder.set_text("Not selected");
        }
        self.output_subfolder
            .set_visible(matches!(output_location, OutputLocation::Subfolder(_)));
        self.output_folder
            .set_visible(matches!(output_location, OutputLocation::Folder(_)));
        self.choose_output_folder
            .set_visible(matches!(output_location, OutputLocation::Folder(_)));

        self.rebuild_tool_lists(&controller);

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

        populate_dependencies(&self.dependencies, &self.dependency_summary, &self.window);
        theme::apply_theme(theme_pref);
        self.suppress.set(false);
    }
}

/// Rebuilds the dependency rows. Detection is cheap (a few `PATH` lookups),
/// but it only runs when the window is refreshed or the user asks for it.
fn populate_dependencies(rows: &gtk::Box, summary: &gtk::Label, window: &gtk::ApplicationWindow) {
    while let Some(child) = rows.first_child() {
        rows.remove(&child);
    }

    let statuses = deck_core::dependencies::statuses();
    let missing = deck_core::dependencies::missing(&statuses);
    if missing.is_empty() {
        summary.set_text("Every optional program is installed.");
    } else {
        summary.set_text(&format!(
            "{} of {} programs are missing; affected tools are disabled.",
            missing.len(),
            statuses.len()
        ));
    }

    for status in &statuses {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        labels.append(&setting_title(status.dependency.label));
        labels.append(&setting_subtitle(status.dependency.purpose));
        row.append(&labels);
        let badge = gtk::Label::new(Some(if status.available {
            "Installed"
        } else {
            "Missing"
        }));
        badge.add_css_class(if status.available {
            "rd-badge"
        } else {
            "rd-badge-missing"
        });
        row.append(&badge);
        rows.append(&row);
    }

    if let Some(command) = PackageManager::detect().install_command(&missing) {
        let install = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let entry = gtk::Entry::new();
        entry.set_text(&command);
        entry.set_editable(false);
        entry.set_hexpand(true);
        entry.add_css_class("rd-mono");
        let copy = gtk::Button::with_label("Copy");
        let window = window.clone();
        let text = command.clone();
        copy.connect_clicked(move |_| {
            window.clipboard().set_text(&text);
        });
        install.append(&entry);
        install.append(&copy);
        rows.append(&install);
        rows.append(&setting_subtitle(
            "Run this in a terminal, then press Recheck. RadialDeck never installs packages itself.",
        ));
    }

    let hint = gtk::Label::new(Some(
        "Optional: Ghostscript for PDF compression, 7-Zip for extra archive formats, KDE Connect for sending files.",
    ));
    hint.add_css_class("rd-subtle");
    hint.set_wrap(true);
    hint.set_halign(gtk::Align::Start);
    rows.append(&hint);
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

impl SettingsWindow {
    pub fn rebuild_tools(&self, controller: &Rc<Controller>) {
        self.rebuild_tool_lists(controller);
    }

    fn rebuild_tool_lists(&self, controller: &Rc<Controller>) {
        while let Some(child) = self.tool_lists.first_child() {
            self.tool_lists.remove(&child);
        }
        for category in deck_core::convert::registry::Category::ALL {
            let ordered = controller.wheel_tools(category);
            // Hidden tools stay listed so they can be switched back on, but
            // they come after the visible ones: the editor has to mirror the
            // wheel order, otherwise "move earlier" looks like a no-op.
            let mut rows: Vec<ToolId> = ordered.clone();
            for tool in deck_core::tools::ToolId::available_for(category) {
                if !rows.contains(tool) {
                    rows.push(*tool);
                }
            }

            let group = gtk::Box::new(gtk::Orientation::Vertical, 4);
            group.add_css_class("rd-tool-group");
            let title = gtk::Label::new(Some(category.label()));
            title.add_css_class("rd-tool-group-title");
            title.set_halign(gtk::Align::Start);
            group.append(&title);
            if ordered.is_empty() {
                let empty = gtk::Label::new(Some("No tools shown for this file type"));
                empty.add_css_class("rd-subtle");
                empty.set_halign(gtk::Align::Start);
                group.append(&empty);
            }

            for tool in rows {
                let visible = ordered.contains(&tool);
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                row.add_css_class("rd-tool-row");

                let toggle = gtk::Switch::new();
                toggle.set_active(visible);
                toggle.set_valign(gtk::Align::Center);
                toggle.set_tooltip_text(Some("Show this tool in the wheel"));

                let label = gtk::Label::new(Some(tool.short_label()));
                label.set_halign(gtk::Align::Start);
                label.set_hexpand(true);
                if !visible {
                    label.add_css_class("rd-subtle");
                }

                let up = gtk::Button::from_icon_name("go-up-symbolic");
                up.add_css_class("flat");
                up.set_tooltip_text(Some("Move earlier"));
                up.set_sensitive(visible && ordered.first() != Some(&tool));
                let down = gtk::Button::from_icon_name("go-down-symbolic");
                down.add_css_class("flat");
                down.set_tooltip_text(Some("Move later"));
                down.set_sensitive(visible && ordered.last() != Some(&tool));

                row.append(&toggle);
                row.append(&label);
                row.append(&up);
                row.append(&down);
                group.append(&row);

                let weak = Rc::downgrade(controller);
                toggle.connect_state_set(move |_, state| {
                    if let Some(controller) = weak.upgrade() {
                        controller.set_tool_visible(category, tool, state);
                    }
                    gtk::glib::Propagation::Proceed
                });

                let weak = Rc::downgrade(controller);
                up.connect_clicked(move |_| {
                    if let Some(controller) = weak.upgrade() {
                        controller.move_tool(category, tool, -1);
                    }
                });

                let weak = Rc::downgrade(controller);
                down.connect_clicked(move |_| {
                    if let Some(controller) = weak.upgrade() {
                        controller.move_tool(category, tool, 1);
                    }
                });
            }
            self.tool_lists.append(&group);
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
