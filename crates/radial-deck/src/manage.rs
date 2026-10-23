use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::actions;
use crate::controller::Controller;
use crate::project_tabs::ProjectTabs;
use crate::wheel::{Intent, ProjectTab};
use crate::widgets::color_dot;
use deck_core::{Availability, Folder};
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;

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
    tabs: ProjectTabs,
    folders: gtk::ListBox,
    heading: gtk::Label,
    subtitle: gtk::Label,
    status: gtk::Label,
    project_color_button: gtk::Button,
    project_menu_button: gtk::MenuButton,
    delete_project: gtk::Button,
    controller: Weak<Controller>,
    refreshing: Cell<bool>,
}

impl ManageWindow {
    pub fn new(app: &gtk::Application, controller: Weak<Controller>) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("RadialDeck: Project Folders")
            .default_width(900)
            .default_height(640)
            .build();
        window.add_css_class("rd-dialog");
        window.add_css_class("rd-manage-window");

        let header = gtk::HeaderBar::new();
        let title = gtk::Label::new(Some("Project Folders"));
        title.add_css_class("rd-title");
        header.set_title_widget(Some(&title));

        let settings_button = gtk::Button::with_label("Settings");
        settings_button.add_css_class("flat");
        header.pack_end(&settings_button);
        let open_wheel = gtk::Button::with_label("Open Wheel");
        open_wheel.add_css_class("flat");
        open_wheel.set_tooltip_text(Some("Show the project-folder wheel"));
        header.pack_end(&open_wheel);

        let tabs = ProjectTabs::new();
        {
            let target = controller.clone();
            tabs.set_handler(Rc::new(move |id| {
                if let Some(controller) = target.upgrade() {
                    controller.set_active_project(id);
                }
            }));
        }
        let add_project = icon_button("list-add-symbolic", "New project");
        add_project.add_css_class("rd-tab-add");
        let rail = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        rail.set_halign(gtk::Align::Start);
        rail.append(&tabs.container);
        rail.append(&add_project);
        let rail_row = gtk::Box::new(gtk::Orientation::Vertical, 0);
        rail_row.set_margin_top(18);
        rail_row.set_margin_bottom(6);
        rail_row.set_margin_start(20);
        rail_row.set_margin_end(20);
        rail_row.append(&rail);

        let heading = gtk::Label::new(None);
        heading.add_css_class("rd-manage-heading");
        heading.set_halign(gtk::Align::Start);
        heading.set_hexpand(true);
        heading.set_ellipsize(pango::EllipsizeMode::End);
        heading.set_max_width_chars(42);
        let project_color_button = gtk::Button::new();
        project_color_button.add_css_class("flat");
        project_color_button.add_css_class("rd-icon-button");
        project_color_button.set_child(Some(&color_dot("#3b82f6", 18)));
        project_color_button.set_tooltip_text(Some("Choose the project colour"));

        let project_menu_button = gtk::MenuButton::new();
        project_menu_button.add_css_class("flat");
        project_menu_button.add_css_class("rd-icon-button");
        project_menu_button.set_tooltip_text(Some("Project actions"));
        let menu_icon = gtk::Image::from_icon_name("view-more-symbolic");
        menu_icon.set_pixel_size(18);
        project_menu_button.set_child(Some(&menu_icon));
        let project_popover = gtk::Popover::new();
        project_popover.add_css_class("rd-menu-popover");
        let project_menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
        project_menu.add_css_class("rd-menu");
        project_menu.set_margin_top(6);
        project_menu.set_margin_bottom(6);
        project_menu.set_margin_start(6);
        project_menu.set_margin_end(6);
        let rename_project = menu_item("document-save-as-symbolic", "Rename project…");
        let delete_project = menu_item("user-trash-symbolic", "Delete project…");
        delete_project.add_css_class("destructive-action");
        project_menu.append(&rename_project);
        project_menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        project_menu.append(&delete_project);
        project_popover.set_child(Some(&project_menu));
        project_menu_button.set_popover(Some(&project_popover));
        project_popover.connect_closed(|popover| popover.unparent());

        let add_folder = gtk::Button::with_label("Add Folder");
        add_folder.add_css_class("suggested-action");
        add_folder.set_tooltip_text(Some("Choose a folder to add to this project"));

        let heading_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        heading_row.set_margin_start(20);
        heading_row.set_margin_end(20);
        heading_row.set_margin_top(10);
        heading_row.append(&project_color_button);
        heading_row.append(&heading);
        heading_row.append(&project_menu_button);
        heading_row.append(&add_folder);

        let subtitle = gtk::Label::new(None);
        subtitle.add_css_class("rd-manage-subtitle");
        subtitle.set_halign(gtk::Align::Start);
        subtitle.set_margin_start(46);
        subtitle.set_margin_end(20);
        subtitle.set_margin_bottom(4);

        let folders = gtk::ListBox::new();
        folders.set_selection_mode(gtk::SelectionMode::None);
        folders.set_activate_on_single_click(false);
        folders.add_css_class("rd-folder-list");
        let folders_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .margin_start(20)
            .margin_end(20)
            .margin_top(6)
            .child(&folders)
            .build();

        let status = gtk::Label::new(None);
        status.add_css_class("rd-status");
        status.set_halign(gtk::Align::Start);
        status.set_margin_start(20);
        status.set_margin_end(20);
        status.set_margin_bottom(8);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_vexpand(true);
        body.append(&rail_row);
        body.append(&heading_row);
        body.append(&subtitle);
        body.append(&folders_scroll);
        body.append(&status);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&body);
        window.set_child(Some(&root));

        let manage = Self {
            window,
            tabs,
            folders,
            heading,
            subtitle,
            status,
            project_color_button,
            project_menu_button,
            delete_project,
            controller: controller.clone(),
            refreshing: Cell::new(false),
        };

        manage.connect_signals(
            &settings_button,
            &open_wheel,
            &add_folder,
            &add_project,
            &rename_project,
        );
        manage.connect_drop_target();
        connect_folder_activation(&manage.folders, controller);
        manage.refresh();
        manage
    }

    fn connect_signals(
        &self,
        settings: &gtk::Button,
        open_wheel: &gtk::Button,
        add_folder: &gtk::Button,
        add_project: &gtk::Button,
        rename_project: &gtk::Button,
    ) {
        let controller = self.controller.clone();
        let window = self.window.clone();
        settings.connect_clicked(move |_| {
            window.close();
            if let Some(controller) = controller.upgrade() {
                controller.open_settings();
            }
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        open_wheel.connect_clicked(move |_| {
            window.close();
            if let Some(controller) = controller.upgrade() {
                controller.show_launcher();
            }
        });

        let controller = self.controller.clone();
        let window = self.window.clone();
        let status = self.status.clone();
        add_folder.connect_clicked(move |_| {
            choose_folder(&window, controller.clone(), status.clone());
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
        let menu = self.project_menu_button.clone();
        rename_project.connect_clicked(move |_| {
            menu.popdown();
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
        let menu = self.project_menu_button.clone();
        self.delete_project.connect_clicked(move |_| {
            menu.popdown();
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
        self.project_color_button.connect_clicked(move |button| {
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

        let projects: Vec<ProjectTab> = config
            .projects
            .iter()
            .map(|project| ProjectTab {
                id: project.id,
                name: project.name.clone(),
                color: project.color.clone(),
                folder_count: project.folders.len(),
            })
            .collect();
        self.tabs.render(&projects, active_id);

        self.heading.set_text(&active_project.name);
        self.delete_project.set_sensitive(project_count > 1);
        self.project_color_button
            .set_child(Some(&color_dot(&active_project.color, 18)));
        self.project_color_button
            .set_tooltip_text(Some(&format!("Project colour: {}", active_project.color)));
        self.subtitle.set_text(&format!(
            "{} folder{} in {} project{} · drag folders onto the list",
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
            let icon = gtk::Image::from_icon_name("folder-symbolic");
            icon.set_pixel_size(36);
            icon.set_opacity(0.5);
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
            let empty_row = gtk::ListBoxRow::new();
            empty_row.set_activatable(false);
            empty_row.set_selectable(false);
            empty_row.set_child(Some(&empty));
            self.folders.append(&empty_row);
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
    let window = window.clone();
    let controller = Rc::clone(controller);
    let status = status.clone();
    let project_color = project_color.to_owned();
    let row = gtk::ListBoxRow::new();
    row.add_css_class("rd-folder-row");
    row.set_activatable(true);
    row.set_tooltip_text(Some(&format!(
        "{}\nDouble-click to open",
        folder.path.display()
    )));

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    content.set_margin_top(9);
    content.set_margin_bottom(9);
    content.set_margin_start(12);
    content.set_margin_end(10);

    let color = folder.color.as_deref().unwrap_or(&project_color).to_owned();
    content.append(&color_dot(&color, 12));

    let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
    labels.set_hexpand(true);
    let label = gtk::Label::new(Some(&folder.label));
    label.add_css_class("rd-folder-name");
    label.set_halign(gtk::Align::Start);
    label.set_ellipsize(pango::EllipsizeMode::End);
    labels.append(&label);

    let path_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    path_row.set_halign(gtk::Align::Fill);
    let path = gtk::Label::new(Some(&folder.path.to_string_lossy()));
    path.add_css_class("rd-folder-path");
    path.set_halign(gtk::Align::Start);
    path.set_ellipsize(pango::EllipsizeMode::Middle);
    path_row.append(&path);
    match availability {
        Availability::Available => {}
        Availability::Missing => {
            let badge = gtk::Label::new(Some("missing"));
            badge.add_css_class("rd-badge");
            badge.add_css_class("rd-badge-error");
            badge.set_valign(gtk::Align::Center);
            path_row.append(&badge);
        }
        Availability::Offline => {
            let badge = gtk::Label::new(Some("drive offline"));
            badge.add_css_class("rd-badge");
            badge.add_css_class("rd-badge-error");
            badge.set_valign(gtk::Align::Center);
            path_row.append(&badge);
        }
    }
    labels.append(&path_row);
    content.append(&labels);

    let open = gtk::Button::with_label("Open");
    open.add_css_class("rd-row-action");
    open.set_tooltip_text(Some("Open this folder"));
    {
        let controller = Rc::downgrade(&controller);
        let folder_id = folder.id;
        open.connect_clicked(move |_| {
            if let Some(controller) = controller.upgrade() {
                controller.launch(folder_id, Intent::Open);
            }
        });
    }
    content.append(&open);

    let more = gtk::Button::from_icon_name("view-more-symbolic");
    more.add_css_class("flat");
    more.add_css_class("rd-icon-button");
    more.set_tooltip_text(Some("Folder actions"));
    more.set_valign(gtk::Align::Center);

    {
        let controller = Rc::downgrade(&controller);
        let folder_id = folder.id;
        let label = folder.label.clone();
        let path = folder.path.clone();
        let current_color = color.clone();
        more.connect_clicked(move |button| {
            let popover = gtk::Popover::new();
            popover.add_css_class("rd-menu-popover");
            popover.set_parent(button);
            popover.set_has_arrow(false);
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
            menu.add_css_class("rd-menu");
            menu.set_margin_top(6);
            menu.set_margin_bottom(6);
            menu.set_margin_start(6);
            menu.set_margin_end(6);

            let open_item = menu_item("document-open-symbolic", "Open");
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                open_item.connect_clicked(move |_| {
                    popdown(&popover);
                    if let Some(controller) = controller.upgrade() {
                        controller.launch(folder_id, Intent::Open);
                    }
                });
            }
            menu.append(&open_item);

            let terminal = menu_item("system-run-symbolic", "Open in Terminal");
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

            let copy = menu_item("edit-copy-symbolic", "Copy Path");
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

            let rename = menu_item("document-save-as-symbolic", "Rename…");
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
                    prompt_text(&window, "Rename folder", &label, move |name| {
                        if let Some(controller) = controller.upgrade() {
                            controller.rename_folder(project_id, folder_id, &name);
                            show_status(&status, "Folder renamed");
                        }
                    });
                });
            }
            menu.append(&rename);

            let color_item = menu_item("color-select-symbolic", "Folder colour…");
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                let button: gtk::Button = button.clone();
                let current_color = current_color.clone();
                color_item.connect_clicked(move |_| {
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
            menu.append(&color_item);

            let up = menu_item("go-up-symbolic", "Move up");
            up.set_sensitive(index > 0);
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                up.connect_clicked(move |_| {
                    popdown(&popover);
                    if let Some(controller) = controller.upgrade() {
                        controller.move_folder(project_id, folder_id, -1);
                    }
                });
            }
            menu.append(&up);

            let down = menu_item("go-down-symbolic", "Move down");
            down.set_sensitive(index + 1 < count);
            {
                let controller = controller.clone();
                let popover = popover.downgrade();
                down.connect_clicked(move |_| {
                    popdown(&popover);
                    if let Some(controller) = controller.upgrade() {
                        controller.move_folder(project_id, folder_id, 1);
                    }
                });
            }
            menu.append(&down);

            menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

            let remove = menu_item("user-trash-symbolic", "Remove from wheel…");
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

    content.append(&more);
    row.set_child(Some(&content));
    row
}

fn menu_item(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.add_css_class("rd-menu-item");
    button.set_halign(gtk::Align::Fill);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(16);
    content.append(&image);
    let text = gtk::Label::new(Some(label));
    text.set_halign(gtk::Align::Start);
    content.append(&text);
    button.set_child(Some(&content));
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
    popover.add_css_class("rd-menu-popover");
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
    let selected: Rc<RefCell<Option<gtk::Button>>> = Rc::new(RefCell::new(None));
    for (index, color) in PALETTE.iter().enumerate() {
        let button = gtk::Button::new();
        button.add_css_class("flat");
        button.set_tooltip_text(Some(color));
        button.set_child(Some(&color_dot(color, 20)));
        if current.is_some_and(|value| value.eq_ignore_ascii_case(color)) {
            button.add_css_class("rd-color-selected");
            *selected.borrow_mut() = Some(button.clone());
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
    let focus_target = selected.borrow().clone();
    if let Some(button) = focus_target {
        glib::idle_add_local_once(move || {
            if button.is_mapped() {
                button.grab_focus();
            }
        });
    }
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
