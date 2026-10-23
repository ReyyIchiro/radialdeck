use std::cell::Cell;
use std::rc::{Rc, Weak};

use deck_core::{Availability, Folder};
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;

use crate::actions;
use crate::controller::Controller;
use crate::theme;
use crate::wheel::Intent;

struct ManageButtons<'a> {
    settings: &'a gtk::Button,
    add_folder: &'a gtk::Button,
    open_wheel: &'a gtk::Button,
    add_project: &'a gtk::Button,
    rename_project: &'a gtk::Button,
    delete_project: &'a gtk::Button,
    project_color: &'a gtk::Button,
}

struct FolderRowContext<'a> {
    window: &'a gtk::ApplicationWindow,
    controller: &'a Rc<Controller>,
    project_id: uuid::Uuid,
    project_color: &'a str,
    index: usize,
    count: usize,
    status: &'a gtk::Label,
}

pub struct ManageWindow {
    window: gtk::ApplicationWindow,
    projects: gtk::ListBox,
    folders: gtk::ListBox,
    heading: gtk::Label,
    subtitle: gtk::Label,
    status: gtk::Label,
    project_color_button: gtk::Button,
    delete_project: gtk::Button,
    controller: Weak<Controller>,
    refreshing: Cell<bool>,
}

impl ManageWindow {
    pub fn new(app: &gtk::Application, controller: Weak<Controller>) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("RadialDeck — Folders")
            .default_width(900)
            .default_height(640)
            .build();
        window.add_css_class("rd-dialog");

        let header = gtk::HeaderBar::new();
        let title = gtk::Label::new(Some("Folders"));
        title.add_css_class("rd-title");
        header.set_title_widget(Some(&title));

        let settings_button = gtk::Button::with_label("Settings");
        header.pack_end(&settings_button);
        let add_button = gtk::Button::with_label("Add Folder");
        add_button.add_css_class("suggested-action");
        header.pack_end(&add_button);
        let open_wheel = gtk::Button::with_label("Open Wheel");
        open_wheel.set_tooltip_text(Some("Show the project-folder wheel"));
        header.pack_end(&open_wheel);

        let projects = gtk::ListBox::new();
        projects.set_selection_mode(gtk::SelectionMode::Single);
        projects.add_css_class("navigation-sidebar");
        let projects_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&projects)
            .build();

        let folders = gtk::ListBox::new();
        folders.set_selection_mode(gtk::SelectionMode::None);
        folders.set_activate_on_single_click(false);
        folders.add_css_class("boxed-list");
        let folders_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&folders)
            .build();

        let heading = gtk::Label::new(None);
        heading.add_css_class("rd-title");
        heading.set_halign(gtk::Align::Start);
        heading.set_hexpand(true);
        let project_color_button = gtk::Button::new();
        project_color_button.add_css_class("flat");
        project_color_button.set_child(Some(&color_dot("#3b82f6", 18)));
        project_color_button.set_tooltip_text(Some("Choose the project colour"));
        let heading_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        heading_row.append(&heading);
        heading_row.append(&project_color_button);
        let subtitle = gtk::Label::new(Some("Drag folders here to add them"));
        subtitle.add_css_class("rd-subtle");
        subtitle.set_halign(gtk::Align::Start);

        let status = gtk::Label::new(None);
        status.add_css_class("rd-footer");
        status.set_halign(gtk::Align::Start);

        let projects_label = gtk::Label::new(Some("Projects"));
        projects_label.add_css_class("rd-subtle");
        projects_label.set_halign(gtk::Align::Start);

        let project_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let add_project = icon_button("list-add-symbolic", "Add project");
        let rename_project = icon_button("document-edit-symbolic", "Rename project");
        let delete_project = icon_button("user-trash-symbolic", "Delete project");
        project_buttons.append(&add_project);
        project_buttons.append(&rename_project);
        project_buttons.append(&delete_project);

        let left = gtk::Box::new(gtk::Orientation::Vertical, 8);
        left.set_width_request(240);
        left.append(&projects_label);
        left.append(&projects_scroll);
        left.append(&project_buttons);

        let right = gtk::Box::new(gtk::Orientation::Vertical, 4);
        right.set_hexpand(true);
        right.append(&heading_row);
        right.append(&subtitle);
        let folder_spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        folder_spacer.set_height_request(6);
        right.append(&folder_spacer);
        right.append(&folders_scroll);
        right.append(&status);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        body.set_margin_top(18);
        body.set_margin_bottom(18);
        body.set_margin_start(18);
        body.set_margin_end(18);
        body.set_vexpand(true);
        body.append(&left);
        body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        body.append(&right);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&body);
        window.set_child(Some(&root));

        let manage = Self {
            window,
            projects,
            folders,
            heading,
            subtitle,
            status,
            project_color_button,
            delete_project,
            controller: controller.clone(),
            refreshing: Cell::new(false),
        };

        manage.connect_signals(ManageButtons {
            settings: &settings_button,
            add_folder: &add_button,
            open_wheel: &open_wheel,
            add_project: &add_project,
            rename_project: &rename_project,
            delete_project: &manage.delete_project,
            project_color: &manage.project_color_button,
        });
        manage.connect_drop_target();
        connect_folder_activation(&manage.folders, controller);
        manage.refresh();
        manage
    }

    fn connect_signals(&self, buttons: ManageButtons<'_>) {
        let controller = self.controller.clone();
        buttons.settings.connect_clicked(move |_| {
            if let Some(controller) = controller.upgrade() {
                controller.open_settings();
            }
        });

        let controller = self.controller.clone();
        buttons.open_wheel.connect_clicked(move |_| {
            if let Some(controller) = controller.upgrade() {
                controller.show_launcher();
            }
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        let status = self.status.clone();
        buttons.add_folder.connect_clicked(move |_| {
            choose_folder(&window, controller.clone(), status.clone());
        });

        let controller = self.controller.clone();
        buttons.project_color.connect_clicked(move |button| {
            let Some(controller) = controller.upgrade() else {
                return;
            };
            let project = controller.config().active_project().clone();
            let project_id = project.id;
            let controller = Rc::downgrade(&controller);
            show_color_popover(button, Some(&project.color), false, move |color| {
                if let (Some(color), Some(controller)) = (color, controller.upgrade()) {
                    controller.set_project_color(project_id, color);
                }
            });
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        let status = self.status.clone();
        buttons.add_project.connect_clicked(move |_| {
            let controller = controller.clone();
            let status = status.clone();
            prompt_text(&window, "New project", "New Project", move |name| {
                if let Some(controller) = controller.upgrade() {
                    controller.add_project(&name);
                    show_status(&status, "Project added");
                }
            });
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        let status = self.status.clone();
        buttons.rename_project.connect_clicked(move |_| {
            let Some(controller) = controller.upgrade() else {
                return;
            };
            let project = controller.config().active_project().clone();
            let controller = Rc::downgrade(&controller);
            let status = status.clone();
            prompt_text(&window, "Rename project", &project.name, move |name| {
                if let Some(controller) = controller.upgrade() {
                    controller.rename_project(project.id, &name);
                    show_status(&status, "Project renamed");
                }
            });
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        buttons.delete_project.connect_clicked(move |_| {
            let Some(controller) = controller.upgrade() else {
                return;
            };
            let project = controller.config().active_project().clone();
            let dialog = gtk::AlertDialog::builder()
                .modal(true)
                .message(format!("Delete “{}”?", project.name))
                .detail("The folder shortcuts in this project will be removed. Files on disk are not touched.")
                .buttons(["Cancel", "Delete"])
                .cancel_button(0)
                .default_button(0)
                .build();
            let controller = Rc::downgrade(&controller);
            dialog.choose(
                Some(&window),
                None::<&gio::Cancellable>,
                move |result| {
                    if result == Ok(1) {
                        if let Some(controller) = controller.upgrade() {
                            controller.delete_project(project.id);
                        }
                    }
                },
            );
        });

        let controller = self.controller.clone();
        self.projects.connect_row_selected(move |_, row| {
            let Some(row) = row else {
                return;
            };
            let Some(controller) = controller.upgrade() else {
                return;
            };
            let index = row.index() as usize;
            let Some(project_id) = controller
                .config()
                .projects
                .get(index)
                .map(|project| project.id)
            else {
                return;
            };
            if controller.config().active_project != project_id {
                controller.set_active_project(project_id);
            }
        });
    }

    fn connect_drop_target(&self) {
        let controller = self.controller.clone();
        let status = self.status.clone();
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        drop.connect_drop(move |_target, value, _x, _y| {
            let Ok(files) = value.get::<gdk::FileList>() else {
                return false;
            };
            let Some(controller) = controller.upgrade() else {
                return false;
            };
            let mut added = 0_usize;
            let mut last_error = None;
            for file in files.files() {
                let Some(path) = file.path() else {
                    continue;
                };
                if !path.is_dir() {
                    last_error = Some("Only folders can be added".to_string());
                    continue;
                }
                match controller.add_folder(path) {
                    Ok(()) => added += 1,
                    Err(error) => last_error = Some(error.to_string()),
                }
            }
            if added > 0 {
                show_status(
                    &status,
                    &format!("Added {added} folder{}", if added == 1 { "" } else { "s" }),
                );
            } else if let Some(error) = last_error {
                show_status(&status, &error);
            }
            added > 0
        });
        self.folders.add_controller(drop);
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn window(&self) -> &gtk::ApplicationWindow {
        &self.window
    }

    pub fn refresh(&self) {
        if self.refreshing.replace(true) {
            return;
        }
        let Some(controller) = self.controller.upgrade() else {
            self.refreshing.set(false);
            return;
        };
        let config = controller.config();
        let active_id = config.active_project;
        let active_project = config.active_project().clone();
        let project_count = config.projects.len();

        self.projects.remove_all();
        for project in &config.projects {
            let row = gtk::ListBoxRow::new();
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            content.set_margin_top(8);
            content.set_margin_bottom(8);
            content.set_margin_start(8);
            content.set_margin_end(8);
            content.append(&color_dot(&project.color, 14));
            let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
            let name = gtk::Label::new(Some(&project.name));
            name.set_halign(gtk::Align::Start);
            name.set_ellipsize(pango::EllipsizeMode::End);
            let count = gtk::Label::new(Some(&format!(
                "{} folder{}",
                project.folders.len(),
                if project.folders.len() == 1 { "" } else { "s" }
            )));
            count.add_css_class("rd-subtle");
            count.set_halign(gtk::Align::Start);
            labels.append(&name);
            labels.append(&count);
            content.append(&labels);
            row.set_child(Some(&content));
            self.projects.append(&row);
            if project.id == active_id {
                self.projects.select_row(Some(&row));
            }
        }

        self.heading.set_text(&active_project.name);
        self.delete_project.set_sensitive(project_count > 1);
        self.project_color_button
            .set_child(Some(&color_dot(&active_project.color, 18)));
        self.project_color_button
            .set_tooltip_text(Some(&format!("Project colour: {}", active_project.color)));
        self.subtitle.set_text(&format!(
            "{} folder{} · {} project{} · drag folders here",
            active_project.folders.len(),
            if active_project.folders.len() == 1 {
                ""
            } else {
                "s"
            },
            project_count,
            if project_count == 1 { "" } else { "s" }
        ));

        self.folders.remove_all();
        let project_color = active_project.color.clone();
        let folder_count = active_project.folders.len();
        for (index, folder) in active_project.folders.iter().enumerate() {
            let availability = config.availability_of(folder);
            let row = build_folder_row(
                FolderRowContext {
                    window: &self.window,
                    controller: &controller,
                    project_id: active_project.id,
                    project_color: &project_color,
                    index,
                    count: folder_count,
                    status: &self.status,
                },
                folder,
                availability,
            );
            self.folders.append(&row);
        }

        if active_project.folders.is_empty() {
            let empty = gtk::Box::new(gtk::Orientation::Vertical, 6);
            empty.set_valign(gtk::Align::Center);
            empty.set_halign(gtk::Align::Center);
            empty.set_margin_top(36);
            empty.set_margin_bottom(36);
            let icon = gtk::Image::from_icon_name("folder-open-symbolic");
            icon.set_pixel_size(36);
            icon.set_opacity(0.45);
            let title = gtk::Label::new(Some("No folders in this project"));
            title.add_css_class("rd-title");
            let hint = gtk::Label::new(Some(
                "Add a folder or drag one here. The wheel shows this project's folders.",
            ));
            hint.add_css_class("rd-subtle");
            let add = gtk::Button::with_label("Add Folder");
            add.add_css_class("suggested-action");
            add.set_halign(gtk::Align::Center);
            let controller = self.controller.clone();
            let window = self.window.clone();
            let status = self.status.clone();
            add.connect_clicked(move |_| {
                choose_folder(&window, controller.clone(), status.clone());
            });
            empty.append(&icon);
            empty.append(&title);
            empty.append(&hint);
            empty.append(&add);
            self.folders.append(&empty);
        }

        drop(config);
        self.refreshing.set(false);
    }
}

fn build_folder_row(
    context: FolderRowContext<'_>,
    folder: &Folder,
    availability: Availability,
) -> gtk::ListBoxRow {
    let FolderRowContext {
        window,
        controller,
        project_id,
        project_color,
        index,
        count,
        status,
    } = context;
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);
    row.set_tooltip_text(Some(&format!(
        "{}\nDouble-click to open",
        folder.path.display()
    )));

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.set_margin_top(6);
    content.set_margin_bottom(6);
    content.set_margin_start(8);
    content.set_margin_end(8);

    let color = folder.color.as_deref().unwrap_or(project_color).to_owned();
    let color_button = gtk::Button::new();
    color_button.add_css_class("flat");
    color_button.set_child(Some(&color_dot(&color, 14)));
    color_button.set_tooltip_text(Some("Folder colour"));
    {
        let controller = Rc::downgrade(controller);
        let folder_id = folder.id;
        let current = color.clone();
        color_button.connect_clicked(move |button| {
            let controller = controller.clone();
            let current = current.clone();
            show_color_popover(button, Some(&current), true, move |picked| {
                if let Some(controller) = controller.upgrade() {
                    controller.set_folder_color(project_id, folder_id, picked);
                }
            });
        });
    }
    content.append(&color_button);

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
    labels.set_hexpand(true);
    let label = gtk::Label::new(Some(&folder.label));
    label.set_halign(gtk::Align::Start);
    label.set_ellipsize(pango::EllipsizeMode::End);
    let path = gtk::Label::new(Some(&folder.path.to_string_lossy()));
    path.add_css_class("rd-subtle");
    path.set_halign(gtk::Align::Start);
    path.set_ellipsize(pango::EllipsizeMode::Middle);
    labels.append(&label);
    labels.append(&path);
    content.append(&labels);

    match availability {
        Availability::Available => {}
        Availability::Missing => {
            let badge = gtk::Label::new(Some("missing"));
            badge.add_css_class("rd-badge");
            badge.add_css_class("rd-badge-error");
            badge.set_valign(gtk::Align::Center);
            content.append(&badge);
        }
        Availability::Offline => {
            let badge = gtk::Label::new(Some("drive offline"));
            badge.add_css_class("rd-badge");
            badge.add_css_class("rd-badge-error");
            badge.set_valign(gtk::Align::Center);
            content.append(&badge);
        }
    }

    let up = icon_button("go-up-symbolic", "Move up");
    up.set_sensitive(index > 0);
    let down = icon_button("go-down-symbolic", "Move down");
    down.set_sensitive(index + 1 < count);
    let more = icon_button("view-more-symbolic", "More actions");

    {
        let controller = Rc::downgrade(controller);
        let folder_id = folder.id;
        up.connect_clicked(move |_| {
            if let Some(controller) = controller.upgrade() {
                controller.move_folder(project_id, folder_id, -1);
            }
        });
    }
    {
        let controller = Rc::downgrade(controller);
        let folder_id = folder.id;
        down.connect_clicked(move |_| {
            if let Some(controller) = controller.upgrade() {
                controller.move_folder(project_id, folder_id, 1);
            }
        });
    }

    {
        let controller = Rc::downgrade(controller);
        let window = window.clone();
        let status = status.clone();
        let more = more.clone();
        let folder_id = folder.id;
        let label = folder.label.clone();
        let path = folder.path.clone();
        let current_color = color.clone();
        more.connect_clicked(move |button| {
            let popover = gtk::Popover::new();
            popover.set_parent(button);
            popover.set_has_arrow(false);
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
            menu.set_margin_top(6);
            menu.set_margin_bottom(6);
            menu.set_margin_start(6);
            menu.set_margin_end(6);

            let open = menu_button("Open");
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                open.connect_clicked(move |_| {
                    popdown(&popover);
                    if let Some(controller) = controller.upgrade() {
                        controller.launch(folder_id, Intent::Open);
                    }
                });
            }
            menu.append(&open);

            let terminal = menu_button("Open in Terminal");
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                terminal.connect_clicked(move |_| {
                    popdown(&popover);
                    if let Some(controller) = controller.upgrade() {
                        controller.launch(folder_id, Intent::Terminal);
                    }
                });
            }
            menu.append(&terminal);

            let copy = menu_button("Copy Path");
            {
                let popover = popover.downgrade();
                let status = status.clone();
                let path = path.clone();
                copy.connect_clicked(move |_| {
                    popdown(&popover);
                    actions::copy_path_to_clipboard(&path);
                    show_status(&status, "Path copied to the clipboard");
                });
            }
            menu.append(&copy);

            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

            let rename = menu_button("Rename…");
            {
                let controller = controller.clone();
                let window = window.clone();
                let status = status.clone();
                let popover = popover.downgrade();
                let label = label.clone();
                rename.connect_clicked(move |_| {
                    popdown(&popover);
                    let controller = controller.clone();
                    let status = status.clone();
                    let initial = label.clone();
                    prompt_text(&window, "Rename folder", &initial, move |name| {
                        if let Some(controller) = controller.upgrade() {
                            controller.rename_folder(project_id, folder_id, &name);
                            show_status(&status, "Folder renamed");
                        }
                    });
                });
            }
            menu.append(&rename);

            let color = menu_button("Folder colour…");
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                let button = button.clone();
                let current_color = current_color.clone();
                color.connect_clicked(move |_| {
                    popdown(&popover);
                    let controller = controller.clone();
                    let current_color = current_color.clone();
                    show_color_popover(&button, Some(&current_color), true, move |picked| {
                        if let Some(controller) = controller.upgrade() {
                            controller.set_folder_color(project_id, folder_id, picked);
                        }
                    });
                });
            }
            menu.append(&color);

            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

            let remove = menu_button("Remove from wheel…");
            remove.add_css_class("destructive-action");
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                let window = window.clone();
                let status = status.clone();
                let label = label.clone();
                remove.connect_clicked(move |_| {
                    popdown(&popover);
                    let dialog = gtk::AlertDialog::builder()
                        .modal(true)
                        .message(format!("Remove “{label}” from the wheel?"))
                        .detail("The folder itself stays on disk.")
                        .buttons(["Cancel", "Remove"])
                        .cancel_button(0)
                        .default_button(0)
                        .build();
                    let controller = controller.clone();
                    let status = status.clone();
                    dialog.choose(Some(&window), None::<&gio::Cancellable>, move |result| {
                        if result == Ok(1) {
                            if let Some(controller) = controller.upgrade() {
                                controller.remove_folder(project_id, folder_id);
                                show_status(&status, "Folder removed from the wheel");
                            }
                        }
                    });
                });
            }
            menu.append(&remove);

            popover.set_child(Some(&menu));
            popover.connect_closed(|popover| popover.unparent());
            popover.popup();
        });
    }

    content.append(&up);
    content.append(&down);
    content.append(&more);
    row.set_child(Some(&content));
    row
}

fn menu_button(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("flat");
    button.set_halign(gtk::Align::Fill);
    button
}

fn popdown(popover: &glib::WeakRef<gtk::Popover>) {
    if let Some(popover) = popover.upgrade() {
        popover.popdown();
    }
}

const PALETTE: [&str; 12] = [
    "#3b82f6", "#06b6d4", "#10b981", "#84cc16", "#eab308", "#f97316", "#ef4444", "#ec4899",
    "#a855f7", "#6366f1", "#64748b", "#111827",
];

fn show_color_popover(
    anchor: &impl IsA<gtk::Widget>,
    current: Option<&str>,
    allow_default: bool,
    on_pick: impl Fn(Option<&str>) + 'static,
) {
    let popover = gtk::Popover::new();
    popover.set_parent(anchor);
    popover.set_has_arrow(false);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);

    let grid = gtk::Grid::builder()
        .row_spacing(6)
        .column_spacing(6)
        .build();
    let on_pick = Rc::new(on_pick);
    for (index, color) in PALETTE.iter().enumerate() {
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.set_tooltip_text(Some(color));
        button.set_child(Some(&color_dot(color, 20)));
        if current.is_some_and(|value| value.eq_ignore_ascii_case(color)) {
            button.add_css_class("rd-color-selected");
        }
        let popover_weak = popover.downgrade();
        let on_pick = on_pick.clone();
        let color = *color;
        button.connect_clicked(move |_| {
            popdown(&popover_weak);
            on_pick(Some(color));
        });
        grid.attach(&button, index as i32 % 6, index as i32 / 6, 1, 1);
    }
    content.append(&grid);

    if allow_default {
        let default = gtk::Button::with_label("Use project colour");
        let popover_weak = popover.downgrade();
        let on_pick = on_pick.clone();
        default.connect_clicked(move |_| {
            popdown(&popover_weak);
            on_pick(None);
        });
        content.append(&default);
    }

    popover.set_child(Some(&content));
    popover.connect_closed(|popover| popover.unparent());
    popover.popup();
}

fn choose_folder(
    parent: &gtk::ApplicationWindow,
    controller: Weak<Controller>,
    status: gtk::Label,
) {
    let dialog = gtk::FileDialog::builder()
        .title("Add folder")
        .modal(true)
        .build();
    let parent = parent.clone();
    dialog.select_folder(
        Some(&parent),
        None::<&gio::Cancellable>,
        move |result| match result {
            Ok(file) => {
                if let Some(path) = file.path() {
                    if let Some(controller) = controller.upgrade() {
                        match controller.add_folder(path) {
                            Ok(()) => show_status(&status, "Folder added"),
                            Err(error) => show_status(&status, &error.to_string()),
                        }
                    }
                }
            }
            Err(error) if !error.matches(gtk::DialogError::Dismissed) => {
                show_status(&status, &error.to_string());
            }
            Err(_) => {}
        },
    );
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.add_css_class("flat");
    button.set_tooltip_text(Some(tooltip));
    button.set_valign(gtk::Align::Center);
    button
}

fn color_dot(color: &str, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_valign(gtk::Align::Center);
    let color = color.to_owned();
    area.set_draw_func(move |_area, cr, width, height| {
        let (r, g, b) = theme::parse_hex(&color);
        let radius = width.min(height) as f64 / 2.0 - 1.0;
        cr.arc(
            width as f64 / 2.0,
            height as f64 / 2.0,
            radius,
            0.0,
            std::f64::consts::TAU,
        );
        cr.set_source_rgb(r, g, b);
        let _ = cr.fill();
    });
    area
}

fn show_status(label: &gtk::Label, message: &str) {
    label.set_text(message);
    let label = label.clone();
    let message = message.to_owned();
    glib::timeout_add_seconds_local_once(5, move || {
        if label.text().as_str() == message {
            label.set_text("");
        }
    });
}

fn prompt_text(
    parent: &gtk::ApplicationWindow,
    title: &str,
    initial: &str,
    on_accept: impl Fn(String) + 'static,
) {
    let dialog = gtk::Window::builder()
        .title(title)
        .modal(true)
        .transient_for(parent)
        .default_width(400)
        .resizable(false)
        .build();
    dialog.add_css_class("rd-dialog");

    let header = gtk::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancel");
    let accept = gtk::Button::with_label("Save");
    accept.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&accept);
    dialog.set_titlebar(Some(&header));

    let entry = gtk::Entry::new();
    entry.set_text(initial);
    entry.set_margin_top(18);
    entry.set_margin_bottom(18);
    entry.set_margin_start(18);
    entry.set_margin_end(18);
    entry.select_region(0, -1);
    dialog.set_child(Some(&entry));

    let on_accept = Rc::new(on_accept);
    let submit: Rc<dyn Fn()> = {
        let entry = entry.clone();
        let weak = dialog.downgrade();
        let on_accept = on_accept.clone();
        Rc::new(move || {
            let value = entry.text().to_string();
            if value.trim().is_empty() {
                return;
            }
            on_accept(value);
            if let Some(dialog) = weak.upgrade() {
                dialog.close();
            }
        })
    };

    {
        let submit = submit.clone();
        accept.connect_clicked(move |_| submit());
    }
    {
        let submit = submit.clone();
        entry.connect_activate(move |_| submit());
    }
    {
        let weak = dialog.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.close();
            }
        });
    }

    dialog.present();
    entry.grab_focus();
}

/// Double-clicking a row opens that folder.
pub fn connect_folder_activation(folders: &gtk::ListBox, controller: Weak<Controller>) {
    folders.connect_row_activated(move |_, row| {
        let Some(controller) = controller.upgrade() else {
            return;
        };
        let index = row.index() as usize;
        let folder_id = controller
            .config()
            .active_project()
            .folders
            .get(index)
            .map(|folder| folder.id);
        if let Some(folder_id) = folder_id {
            controller.launch(folder_id, Intent::Open);
        }
    });
}
