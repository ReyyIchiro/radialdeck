//! A small persistent shelf for files being compared, edited, or dragged on.
//!
//! Float is the Linux counterpart of Tangerine's floating shelf. It is a
//! non-modal layer-shell surface on Wayland (a regular window on X11) and
//! every row is a real drag source, so files can be dragged back into Dolphin
//! or another application.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::glib::value::ToValue;
use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

pub struct FloatShelf {
    window: gtk::ApplicationWindow,
    list: gtk::Box,
    paths: RefCell<Vec<PathBuf>>,
    destroyed: Cell<bool>,
}

impl FloatShelf {
    pub fn new(app: &gtk::Application) -> Rc<Self> {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Float")
            .decorated(false)
            .build();
        window.add_css_class("rd-float-window");
        window.set_default_size(340, 420);

        if crate::surface::layer_shell_available() {
            window.init_layer_shell();
            window.set_namespace(Some("radial-deck-float"));
            window.set_layer(gtk4_layer_shell::Layer::Overlay);
            window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);
            window.set_anchor(gtk4_layer_shell::Edge::Top, true);
            window.set_anchor(gtk4_layer_shell::Edge::Right, true);
            window.set_margin(gtk4_layer_shell::Edge::Top, 24);
            window.set_margin(gtk4_layer_shell::Edge::Right, 24);
            window.set_exclusive_zone(0);
        }

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("rd-float-panel");

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.add_css_class("rd-float-header");
        let title = gtk::Label::new(Some("Float"));
        title.add_css_class("rd-float-title");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        header.append(&title);
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("rd-icon-button");
        close.set_tooltip_text(Some("Close shelf"));
        header.append(&close);
        root.append(&header);

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_vexpand(true);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.add_css_class("rd-float-list");
        list.set_margin_start(12);
        list.set_margin_end(12);
        list.set_margin_top(4);
        list.set_margin_bottom(12);
        scroller.set_child(Some(&list));
        root.append(&scroller);

        let hint = gtk::Label::new(Some("Drop files here · drag a row back out"));
        hint.add_css_class("rd-footer");
        hint.set_margin_bottom(10);
        root.append(&hint);
        window.set_child(Some(&root));

        let shelf = Rc::new(Self {
            window,
            list,
            paths: RefCell::new(Vec::new()),
            destroyed: Cell::new(false),
        });

        {
            let window = shelf.window.clone();
            close.connect_clicked(move |_| window.set_visible(false));
        }
        {
            let shelf = Rc::downgrade(&shelf);
            shelf
                .upgrade()
                .expect("shelf alive")
                .window
                .connect_destroy(move |_| {
                    if let Some(shelf) = shelf.upgrade() {
                        shelf.destroyed.set(true);
                    }
                });
        }
        shelf.install_drop_target(&root);
        shelf.render();
        shelf
    }

    pub fn add(self: &Rc<Self>, files: impl IntoIterator<Item = PathBuf>) {
        let mut changed = false;
        {
            let mut paths = self.paths.borrow_mut();
            for file in files {
                if !paths.contains(&file) {
                    paths.push(file);
                    changed = true;
                }
            }
        }
        if changed {
            self.render();
        }
    }

    pub fn present(&self) {
        if crate::surface::layer_shell_available() {
            crate::surface::anchor_to_pointer_monitor(&self.window);
        }
        self.window.set_visible(true);
        self.window.present();
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    fn remove(self: &Rc<Self>, path: &std::path::Path) {
        self.paths.borrow_mut().retain(|item| item != path);
        self.render();
    }

    fn render(self: &Rc<Self>) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let paths = self.paths.borrow().clone();
        if paths.is_empty() {
            let empty = gtk::Label::new(Some("Drop files to keep them close"));
            empty.add_css_class("rd-float-empty");
            empty.set_margin_top(28);
            self.list.append(&empty);
            return;
        }
        for path in paths {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("rd-float-row");
            row.set_tooltip_text(Some(&path.to_string_lossy()));

            let icon = gtk::Image::from_icon_name(if path.is_dir() {
                "folder-symbolic"
            } else {
                "text-x-generic-symbolic"
            });
            icon.set_pixel_size(18);
            row.append(&icon);

            let name = gtk::Label::new(Some(
                &path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
            ));
            name.set_hexpand(true);
            name.set_xalign(0.0);
            name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            row.append(&name);

            let remove = gtk::Button::from_icon_name("window-close-symbolic");
            remove.add_css_class("rd-icon-button");
            remove.set_tooltip_text(Some("Remove from Float"));
            {
                let shelf: Weak<Self> = Rc::downgrade(self);
                let path = path.clone();
                remove.connect_clicked(move |_| {
                    if let Some(shelf) = shelf.upgrade() {
                        shelf.remove(&path);
                    }
                });
            }
            row.append(&remove);

            let drag = gtk::DragSource::new();
            drag.set_actions(gdk::DragAction::COPY);
            {
                let path = path.clone();
                drag.connect_prepare(move |_, _, _| {
                    let files = [gio::File::for_path(&path)];
                    let list = gdk::FileList::from_array(&files);
                    Some(gdk::ContentProvider::for_value(&list.to_value()))
                });
            }
            row.add_controller(drag);
            self.list.append(&row);
        }
    }

    fn install_drop_target(self: &Rc<Self>, widget: &impl IsA<gtk::Widget>) {
        let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        {
            let shelf = Rc::downgrade(self);
            target.connect_drop(move |_, value, _, _| {
                let Some(shelf) = shelf.upgrade() else {
                    return false;
                };
                let Ok(list) = value.get::<gdk::FileList>() else {
                    return false;
                };
                let files: Vec<PathBuf> = list
                    .files()
                    .into_iter()
                    .filter_map(|file| file.path())
                    .collect();
                if files.is_empty() {
                    return false;
                }
                shelf.add(files);
                true
            });
        }
        widget.add_controller(target);

        let uri_target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);
        {
            let shelf = Rc::downgrade(self);
            uri_target.connect_drop(move |_, value, _, _| {
                let Some(shelf) = shelf.upgrade() else {
                    return false;
                };
                let Ok(text) = value.get::<String>() else {
                    return false;
                };
                let files: Vec<PathBuf> = text
                    .lines()
                    .filter_map(|line| glib::filename_from_uri(line.trim()).ok())
                    .map(|(path, _)| path)
                    .collect();
                if files.is_empty() {
                    return false;
                }
                shelf.add(files);
                true
            });
        }
        widget.add_controller(uri_target);
    }
}
