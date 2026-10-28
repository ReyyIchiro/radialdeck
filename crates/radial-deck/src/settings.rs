use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use deck_core::convert::registry::Category;
use deck_core::tools::ToolId;
use deck_core::{OutputLocation, PackageManager, Theme};
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;

use crate::actions;
use crate::controller::Controller;
use crate::theme;

/// Page order and identifiers. The sidebar rows and the stack pages are built
/// from the same list, so adding a page means touching one place only.
const PAGE_IDS: [&str; 6] = [
    "general",
    "shortcuts",
    "projects",
    "tools",
    "output",
    "advanced",
];

const SIDEBAR: [(&str, &str); 6] = [
    ("emblem-system-symbolic", "General"),
    ("input-keyboard-symbolic", "Shortcuts"),
    ("folder-symbolic", "Projects"),
    ("applications-utilities-symbolic", "Tool wheel"),
    ("folder-download-symbolic", "Output"),
    ("applications-system-symbolic", "Advanced"),
];

pub struct SettingsWindow {
    window: gtk::ApplicationWindow,
    nav: gtk::ListBox,
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
    output_subfolder_row: gtk::Box,
    output_folder: gtk::Label,
    output_folder_row: gtk::Box,
    tool_category: gtk::DropDown,
    tool_summary: gtk::Label,
    tool_lists: gtk::Box,
    project_summary: gtk::Label,
    project_list: gtk::Box,
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
            .default_width(900)
            .default_height(660)
            .build();
        window.add_css_class("rd-dialog");

        let quick_shortcut = gtk::Label::new(None);
        quick_shortcut.add_css_class("rd-badge");
        let projects_shortcut = gtk::Label::new(None);
        projects_shortcut.add_css_class("rd-badge");

        let quick_error = shortcut_error_label();
        let projects_error = shortcut_error_label();

        // --- Shortcuts page -------------------------------------------------
        let shortcut_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        shortcut_box.append(&setting_title("Global shortcuts"));
        shortcut_box.append(&setting_subtitle(
            "Two independent shortcuts, registered through the desktop portal. The system may ask you to confirm them once.",
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

        // --- Projects page --------------------------------------------------
        let project_summary = setting_subtitle("");
        let project_labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        project_labels.set_hexpand(true);
        project_labels.append(&setting_title("Projects & folders"));
        project_labels.append(&project_summary);
        let open_wheel = gtk::Button::with_label("Open Wheel");
        let manage_button = gtk::Button::with_label("Manage…");
        let manage_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        manage_buttons.append(&open_wheel);
        manage_buttons.append(&manage_button);
        let manage_row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        manage_row.append(&project_labels);
        manage_row.append(&manage_buttons);
        // Read-only overview: the Manage window owns editing, but seeing the
        // folders here answers "what is on my wheel?" without a detour.
        let project_list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let projects_card = gtk::Box::new(gtk::Orientation::Vertical, 10);
        projects_card.append(&manage_row);
        projects_card.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        projects_card.append(&project_list);

        // --- General page ---------------------------------------------------
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

        let behavior_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        behavior_box.append(&close_row);
        behavior_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        behavior_box.append(&live_row);
        behavior_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        behavior_box.append(&autostart_row);

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

        // --- Output page ----------------------------------------------------
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

        // --- Tool wheel page ------------------------------------------------
        let category_labels: Vec<&str> = Category::ALL.iter().map(|item| item.label()).collect();
        let tool_category = gtk::DropDown::from_strings(&category_labels);
        tool_category.set_valign(gtk::Align::Center);
        let tool_summary = setting_subtitle("");
        let tool_lists = gtk::Box::new(gtk::Orientation::Vertical, 2);
        tool_lists.add_css_class("rd-tool-editor");
        let reset_tools = gtk::Button::with_label("Reset");
        reset_tools.set_valign(gtk::Align::Center);
        reset_tools.set_tooltip_text(Some("Restore the built-in order for this file type"));

        let tool_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let type_label = setting_title("File type");
        type_label.set_valign(gtk::Align::Center);
        tool_header.append(&type_label);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        tool_header.append(&spacer);
        tool_header.append(&tool_category);
        tool_header.append(&reset_tools);

        let tool_section = gtk::Box::new(gtk::Orientation::Vertical, 10);
        tool_section.append(&tool_header);
        tool_section.append(&tool_summary);
        tool_section.append(&tool_lists);
        let tool_hint = setting_subtitle(
            "Hidden tools stay in the list after the visible ones. Digit shortcuts follow this order.",
        );
        tool_section.append(&tool_hint);

        // --- Advanced page --------------------------------------------------
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

        // --- Pages and sidebar ----------------------------------------------
        let general = page_content(
            "General",
            "Appearance, launch behavior, and the terminal used by actions.",
        );
        general.append(&card(theme_row));
        general.append(&card(terminal_row));
        general.append(&card(behavior_box));

        let shortcuts = page_content(
            "Shortcuts",
            "Global hotkeys that open the two wheels from anywhere.",
        );
        shortcuts.append(&card(shortcut_box));

        let projects = page_content(
            "Projects",
            "Each project is a wheel of folders you can launch in one gesture.",
        );
        projects.append(&card(projects_card));

        let tools = page_content(
            "Tool wheel",
            "Choose which tools appear for each file type and in what order.",
        );
        tools.append(&card(tool_section));

        let output = page_content(
            "Output",
            "Where converted files and tool results are written. Originals are never overwritten.",
        );
        output.append(&card(output_box));

        let advanced = page_content(
            "Advanced",
            "Optional programs, configuration files, and logs.",
        );
        advanced.append(&card(dependency_card));
        advanced.append(&card(config_row));
        advanced.append(&card(log_row));
        let footer = gtk::Label::new(Some(&format!(
            "RadialDeck {} · MIT · folders stay on this device",
            env!("CARGO_PKG_VERSION")
        )));
        footer.add_css_class("rd-footer");
        footer.set_margin_top(4);
        advanced.append(&footer);

        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_transition_duration(120);
        for (id, content) in PAGE_IDS
            .iter()
            .zip([&general, &shortcuts, &projects, &tools, &output, &advanced])
        {
            stack.add_named(&page_scroller(content), Some(*id));
        }

        let nav = gtk::ListBox::new();
        nav.add_css_class("rd-sidebar");
        nav.set_selection_mode(gtk::SelectionMode::Single);
        nav.set_activate_on_single_click(true);
        nav.set_margin_start(8);
        nav.set_margin_end(8);
        nav.set_vexpand(true);
        for (icon_name, label) in SIDEBAR {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            content.set_margin_top(6);
            content.set_margin_bottom(6);
            content.set_margin_start(10);
            content.set_margin_end(10);
            let icon = gtk::Image::from_icon_name(icon_name);
            icon.add_css_class("rd-sidebar-icon");
            let text = gtk::Label::new(Some(label));
            text.set_halign(gtk::Align::Start);
            text.set_hexpand(true);
            content.append(&icon);
            content.append(&text);
            row.set_child(Some(&content));
            nav.append(&row);
        }

        let sidebar_header = gtk::Box::new(gtk::Orientation::Vertical, 1);
        sidebar_header.set_margin_top(16);
        sidebar_header.set_margin_bottom(10);
        sidebar_header.set_margin_start(18);
        sidebar_header.set_margin_end(18);
        let app_name = gtk::Label::new(Some("RadialDeck"));
        app_name.add_css_class("rd-sidebar-app");
        app_name.set_halign(gtk::Align::Start);
        let app_sub = gtk::Label::new(Some("Settings"));
        app_sub.add_css_class("rd-sidebar-sub");
        app_sub.set_halign(gtk::Align::Start);
        sidebar_header.append(&app_name);
        sidebar_header.append(&app_sub);

        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar.add_css_class("rd-sidebar-pane");
        sidebar.set_size_request(216, -1);
        sidebar.append(&sidebar_header);
        sidebar.append(&nav);
        let version = gtk::Label::new(Some(&format!("v{}", env!("CARGO_PKG_VERSION"))));
        version.add_css_class("rd-sidebar-version");
        version.set_halign(gtk::Align::Start);
        version.set_margin_start(18);
        version.set_margin_bottom(14);
        sidebar.append(&version);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.append(&sidebar);
        body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        body.append(&stack);
        window.set_child(Some(&body));

        {
            let stack = stack.clone();
            nav.connect_row_selected(move |_, row| {
                if let Some(row) = row {
                    if let Some(id) = PAGE_IDS.get(row.index() as usize) {
                        stack.set_visible_child_name(id);
                    }
                }
            });
        }
        nav.select_row(nav.row_at_index(0).as_ref());

        let settings = Self {
            window,
            nav,
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
            output_subfolder_row,
            output_folder,
            output_folder_row,
            tool_category,
            tool_summary,
            tool_lists,
            project_summary,
            project_list,
            config_path,
            log_path,
            dependencies,
            dependency_summary,
            controller: controller.clone(),
            suppress: Rc::new(Cell::new(false)),
        };

        settings.connect_signals(
            &change,
            &manage_button,
            &open_wheel,
            &open_folder,
            &open_logs,
            &choose_output_folder,
            &recheck,
            &reset_tools,
        );
        settings.refresh();
        settings
    }

    #[allow(clippy::too_many_arguments)]
    fn connect_signals(
        &self,
        change: &gtk::Button,
        manage: &gtk::Button,
        open_wheel: &gtk::Button,
        open_folder: &gtk::Button,
        open_logs: &gtk::Button,
        choose_output_folder: &gtk::Button,
        recheck: &gtk::Button,
        reset_tools: &gtk::Button,
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
            open_wheel.connect_clicked(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.show_launcher();
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
            self.tool_category.connect_selected_notify(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.refresh_tool_editor();
                }
            });
        }
        {
            let controller = self.controller.clone();
            let dropdown = self.tool_category.clone();
            reset_tools.connect_clicked(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.reset_wheel_tools(selected_category(&dropdown));
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
            choose_output_folder.connect_clicked(move |_| {
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
        // Focus the page list so Up/Down switch pages without the pointer.
        let nav = self.nav.clone();
        glib::idle_add_local_once(move || {
            nav.grab_focus();
        });
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
            project_count,
            folder_count,
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
                config.projects.len(),
                config
                    .projects
                    .iter()
                    .map(|project| project.folders.len())
                    .sum::<usize>(),
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
        self.output_subfolder_row
            .set_visible(matches!(output_location, OutputLocation::Subfolder(_)));
        self.output_folder_row
            .set_visible(matches!(output_location, OutputLocation::Folder(_)));

        self.rebuild_project_list(&controller);
        if project_count == 0 {
            self.project_summary
                .set_text("No projects yet — create one to fill the project wheel.");
        } else {
            let projects = if project_count == 1 {
                "project"
            } else {
                "projects"
            };
            let folders = if folder_count == 1 {
                "folder"
            } else {
                "folders"
            };
            self.project_summary.set_text(&format!(
                "{project_count} {projects} · {folder_count} {folders}"
            ));
        }

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

    /// Lists every project with its folders, marking the active one. Long
    /// folder paths are ellipsized in the middle so the tail (the part that
    /// tells sibling folders apart) stays readable.
    fn rebuild_project_list(&self, controller: &Rc<Controller>) {
        while let Some(child) = self.project_list.first_child() {
            self.project_list.remove(&child);
        }
        let config = controller.config();
        if config.projects.is_empty() {
            let empty = gtk::Label::new(Some("No projects yet."));
            empty.add_css_class("rd-subtle");
            empty.set_halign(gtk::Align::Start);
            self.project_list.append(&empty);
            return;
        }
        for project in &config.projects {
            let active = config.active_project == project.id;
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("rd-project-row");
            row.append(&crate::widgets::color_dot(&project.color, 12));
            let name = gtk::Label::new(Some(&project.name));
            name.add_css_class("rd-title");
            name.set_halign(gtk::Align::Start);
            row.append(&name);
            if active {
                let badge = gtk::Label::new(Some("Active"));
                badge.add_css_class("rd-badge");
                row.append(&badge);
            }
            let count = project.folders.len();
            let label = if count == 1 {
                "1 folder".to_string()
            } else {
                format!("{count} folders")
            };
            let badge = gtk::Label::new(Some(&label));
            badge.add_css_class("rd-subtle");
            badge.set_hexpand(true);
            badge.set_halign(gtk::Align::End);
            row.append(&badge);
            self.project_list.append(&row);

            for folder in &project.folders {
                let item = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                item.set_margin_start(22);
                let name = gtk::Label::new(Some(&folder.label));
                name.set_halign(gtk::Align::Start);
                item.append(&name);
                let path = gtk::Label::new(Some(&folder.path.to_string_lossy()));
                path.add_css_class("rd-subtle");
                path.set_ellipsize(pango::EllipsizeMode::Middle);
                path.set_hexpand(true);
                path.set_halign(gtk::Align::Start);
                path.set_tooltip_text(Some(&folder.path.to_string_lossy()));
                item.append(&path);
                self.project_list.append(&item);
            }
        }
    }

    fn rebuild_tool_lists(&self, controller: &Rc<Controller>) {
        while let Some(child) = self.tool_lists.first_child() {
            self.tool_lists.remove(&child);
        }
        let category = selected_category(&self.tool_category);
        let ordered = controller.wheel_tools(category);
        let available = ToolId::available_for(category);
        let hidden = available
            .iter()
            .filter(|tool| !ordered.contains(tool))
            .count();
        self.tool_summary.set_text(&format!(
            "{} of {} tools shown · {} hidden",
            ordered.len(),
            available.len(),
            hidden
        ));

        // Hidden tools stay listed so they can be switched back on, but they
        // come after the visible ones: the editor has to mirror the wheel
        // order, otherwise "move earlier" looks like a no-op.
        let mut rows: Vec<ToolId> = ordered.clone();
        for tool in available {
            if !rows.contains(tool) {
                rows.push(*tool);
            }
        }

        if ordered.is_empty() {
            let empty = gtk::Label::new(Some("No tools shown for this file type"));
            empty.add_css_class("rd-subtle");
            empty.set_halign(gtk::Align::Start);
            empty.set_margin_top(6);
            empty.set_margin_bottom(6);
            self.tool_lists.append(&empty);
        }

        for tool in rows {
            let visible = ordered.contains(&tool);
            let index = ordered.iter().position(|item| *item == tool);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.add_css_class("rd-tool-row");

            let toggle = gtk::Switch::new();
            toggle.set_active(visible);
            toggle.set_valign(gtk::Align::Center);
            toggle.set_tooltip_text(Some("Show this tool in the wheel"));

            let position = gtk::Label::new(Some(&match index {
                Some(index) => (index + 1).to_string(),
                None => String::new(),
            }));
            position.add_css_class("rd-tool-index");

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
            row.append(&position);
            row.append(&label);
            row.append(&up);
            row.append(&down);
            self.tool_lists.append(&row);

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
    }
}

fn selected_category(dropdown: &gtk::DropDown) -> Category {
    Category::ALL
        .get(dropdown.selected() as usize)
        .copied()
        .unwrap_or(Category::Images)
}

/// A page body: fixed reading width, generous margins, and a heading. Pages
/// live inside their own scroller so long lists never push the sidebar.
fn page_content(title: &str, subtitle: &str) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(28);
    content.set_margin_end(28);
    content.set_halign(gtk::Align::Center);
    content.set_width_request(560);

    let heading = gtk::Box::new(gtk::Orientation::Vertical, 3);
    let title = gtk::Label::new(Some(title));
    title.add_css_class("rd-page-title");
    title.set_halign(gtk::Align::Start);
    let subtitle = gtk::Label::new(Some(subtitle));
    subtitle.add_css_class("rd-page-subtitle");
    subtitle.set_halign(gtk::Align::Start);
    subtitle.set_wrap(true);
    subtitle.set_xalign(0.0);
    heading.append(&title);
    heading.append(&subtitle);
    heading.set_margin_bottom(4);
    content.append(&heading);
    content
}

fn page_scroller(content: &gtk::Box) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(content)
        .build()
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
