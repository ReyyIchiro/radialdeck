use std::cell::Cell;
use std::rc::{Rc, Weak};

use deck_core::{Availability, Folder};
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;

use crate::controller::Controller;
use crate::theme;
use crate::wheel::Intent;

pub struct ManageWindow {
    window: gtk::ApplicationWindow,
    projects: gtk::ListBox,
    folders: gtk::ListBox,
    heading: gtk::Label,
    subtitle: gtk::Label,
    status: gtk::Label,
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
        right.append(&heading);
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
            controller: controller.clone(),
            refreshing: Cell::new(false),
        };

        manage.connect_signals(
            &settings_button,
            &add_button,
            &add_project,
            &rename_project,
            &delete_project,
        );
        manage.connect_drop_target();
        connect_folder_activation(&manage.folders, controller);
        manage.refresh();
        manage
    }

    fn connect_signals(
        &self,
        settings_button: &gtk::Button,
        add_button: &gtk::Button,
        add_project: &gtk::Button,
        rename_project: &gtk::Button,
        delete_project: &gtk::Button,
    ) {
        let controller = self.controller.clone();
        settings_button.connect_clicked(move |_| {
            if let Some(controller) = controller.upgrade() {
                controller.open_settings();
            }
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        let status = self.status.clone();
        add_button.connect_clicked(move |_| {
            let controller = controller.clone();
            let status = status.clone();
            let dialog = gtk::FileDialog::builder()
                .title("Add folder")
                .modal(true)
                .build();
            let parent = window.clone();
            dialog.select_folder(Some(&parent), None::<&gio::Cancellable>, move |result| {
                match result {
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
                }
            });
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        let status = self.status.clone();
        add_project.connect_clicked(move |_| {
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
        rename_project.connect_clicked(move |_| {
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
        delete_project.connect_clicked(move |_| {
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
                &controller,
                active_project.id,
                &project_color,
                folder,
                availability,
                index,
                folder_count,
            );
            self.folders.append(&row);
        }

        if active_project.folders.is_empty() {
            let empty = gtk::Label::new(Some(
                "No folders yet — use “Add Folder” or drop folders here.",
            ));
            empty.add_css_class("rd-subtle");
            empty.set_margin_top(24);
            empty.set_margin_bottom(24);
            self.folders.append(&empty);
        }

        drop(config);
        self.refreshing.set(false);
    }
}

fn build_folder_row(
    controller: &Rc<Controller>,
    project_id: uuid::Uuid,
    project_color: &str,
    folder: &Folder,
    availability: Availability,
    index: usize,
    count: usize,
) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(true);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(10);
    content.set_margin_end(10);

    content.append(&color_dot(
        folder.color.as_deref().unwrap_or(project_color),
        12,
    ));

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 1);
    labels.set_hexpand(true);
    let label = gtk::Label::new(Some(&folder.label));
    label.set_halign(gtk::Align::Start);
    label.set_ellipsize(pango::EllipsizeMode::End);
    let path = gtk::Label::new(Some(&folder.path.to_string_lossy()));
    path.add_css_class("rd-subtle");
    path.set_halign(gtk::Align::Start);
    path.set_ellipsize(pango::EllipsizeMode::Middle);
    path.set_tooltip_text(Some(&folder.path.to_string_lossy()));
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
    let delete = icon_button("user-trash-symbolic", "Remove folder");

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
        let folder_id = folder.id;
        delete.connect_clicked(move |_| {
            if let Some(controller) = controller.upgrade() {
                controller.remove_folder(project_id, folder_id);
            }
        });
    }

    content.append(&up);
    content.append(&down);
    content.append(&delete);
    row.set_child(Some(&content));
    row
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
