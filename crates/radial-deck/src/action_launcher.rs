//! Full-screen overlay that hosts the radial action wheel.

use std::path::PathBuf;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::action_wheel::{ActionWheel, WheelEvent, WheelTarget};
use crate::theme;
use deck_core::Category;

#[derive(Debug, Clone)]
pub enum ActionRequest {
    Category(Category),
    Run(String),
    Unavailable(String),
    Dismiss,
    Dropped(Vec<PathBuf>),
}

type RequestHandler = Rc<dyn Fn(ActionRequest)>;

pub struct ActionView {
    pub category: Category,
    pub title: String,
    pub source_label: String,
    pub targets: Vec<WheelTarget>,
    pub shortcut: String,
    pub status: Option<String>,
    pub empty_hint: String,
}

pub struct ActionLauncher {
    window: gtk::ApplicationWindow,
    panel: gtk::Box,
    tabs: Vec<(Category, gtk::Button)>,
    title: gtk::Label,
    shortcut: gtk::Label,
    wheel: Rc<ActionWheel>,
    status: gtk::Label,
    footer: gtk::Label,
}

impl ActionLauncher {
    pub fn new(app: &gtk::Application, handler: RequestHandler) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("RadialDeck")
            .decorated(false)
            .build();
        window.add_css_class("rd-action-overlay");
        window.set_can_focus(true);

        let layer_shell = gtk4_layer_shell::is_supported();
        if layer_shell {
            window.init_layer_shell();
            window.set_namespace(Some("radial-deck-actions"));
            window.set_layer(gtk4_layer_shell::Layer::Overlay);
            window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::Exclusive);
            for edge in [
                gtk4_layer_shell::Edge::Top,
                gtk4_layer_shell::Edge::Bottom,
                gtk4_layer_shell::Edge::Left,
                gtk4_layer_shell::Edge::Right,
            ] {
                window.set_anchor(edge, true);
            }
            window.set_exclusive_zone(-1);
        } else {
            window.set_default_size(1040, 820);
        }

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("rd-panel");
        panel.set_halign(gtk::Align::Center);
        panel.set_valign(gtk::Align::Center);
        panel.set_width_request(980);
        panel.set_margin_top(24);
        panel.set_margin_bottom(24);

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        tabs.add_css_class("rd-tabs");
        tabs.set_halign(gtk::Align::Center);
        tabs.set_margin_bottom(4);

        let mut tab_buttons = Vec::new();
        for category in Category::ALL {
            let button = gtk::Button::new();
            button.add_css_class("rd-tab");
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
            let icon = gtk::Image::from_icon_name(category.icon());
            icon.set_pixel_size(16);
            content.append(&icon);
            let label = gtk::Label::new(Some(category.label()));
            label.add_css_class("rd-tab-label");
            content.append(&label);
            button.set_child(Some(&content));
            button.set_tooltip_text(Some(category.heading()));
            let handler = handler.clone();
            button.connect_clicked(move |_| handler(ActionRequest::Category(category)));
            tabs.append(&button);
            tab_buttons.push((category, button));
        }
        panel.append(&tabs);

        let title = gtk::Label::new(Some("Image conversions"));
        title.add_css_class("rd-heading");
        title.set_halign(gtk::Align::Center);
        title.set_margin_top(10);
        panel.append(&title);

        let chip = gtk::Box::new(gtk::Orientation::Vertical, 6);
        chip.set_halign(gtk::Align::Center);
        chip.set_margin_top(12);
        let key = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        key.add_css_class("rd-key");
        let shortcut = gtk::Label::new(None);
        shortcut.add_css_class("rd-key-label");
        key.append(&shortcut);
        chip.append(&key);
        let caption = gtk::Label::new(Some("Convert formats"));
        caption.add_css_class("rd-caption");
        chip.append(&caption);
        panel.append(&chip);

        let wheel = Rc::new(ActionWheel::new());
        wheel.area.set_margin_top(6);
        wheel.area.set_vexpand(true);
        wheel.area.set_size_request(560, 360);
        {
            let handler = handler.clone();
            wheel.set_handler(Rc::new(move |event: WheelEvent| match event {
                WheelEvent::Run(id) => handler(ActionRequest::Run(id)),
                WheelEvent::Unavailable(reason) => handler(ActionRequest::Unavailable(reason)),
                WheelEvent::Dismiss => handler(ActionRequest::Dismiss),
            }));
        }
        panel.append(&wheel.area);

        let status = gtk::Label::new(None);
        status.add_css_class("rd-status");
        status.set_halign(gtk::Align::Center);
        status.set_visible(false);
        status.set_margin_top(2);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        status.set_max_width_chars(80);
        panel.append(&status);

        let footer = gtk::Label::new(None);
        footer.add_css_class("rd-footer");
        footer.set_halign(gtk::Align::Center);
        footer.set_margin_top(6);
        panel.append(&footer);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("rd-scrim");
        root.set_halign(gtk::Align::Fill);
        root.set_valign(gtk::Align::Fill);
        root.set_hexpand(true);
        root.set_vexpand(true);
        panel.set_halign(gtk::Align::Center);
        panel.set_valign(gtk::Align::Center);
        root.append(&panel);
        window.set_child(Some(&root));

        {
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            let window_ref = window.clone();
            keys.connect_key_pressed(move |_keys, keyval, _keycode, _modifiers| {
                if keyval == gdk::Key::Escape {
                    window_ref.set_visible(false);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            window.add_controller(keys);
        }

        {
            // Clicks on the dimmed area around the panel dismiss the wheel.
            let click = gtk::GestureClick::new();
            click.set_propagation_phase(gtk::PropagationPhase::Capture);
            let root_ref = root.clone();
            click.connect_pressed(move |_gesture, _n, x, y| {
                let picked = root_ref.pick(x, y, gtk::PickFlags::DEFAULT);
                if picked.as_ref().is_none_or(|widget| widget == &root_ref) {
                    root_ref.set_visible(false);
                }
            });
            root.add_controller(click);
        }

        // Dropping a file anywhere on the panel feeds the wheel.
        install_drop_target(&panel, wheel.clone(), handler.clone(), true);
        install_drop_target(&wheel.area, wheel.clone(), handler.clone(), false);

        // Clicking the dimmed backdrop dismisses the overlay.
        let click = gtk::GestureClick::new();
        {
            let panel = panel.clone();
            let window_ref = window.clone();
            let handler = handler.clone();
            click.connect_pressed(move |_gesture, _n, x, y| {
                let inside = panel
                    .compute_bounds(&window_ref)
                    .map(|bounds| {
                        let x = x as f32;
                        let y = y as f32;
                        x >= bounds.x()
                            && x <= bounds.x() + bounds.width()
                            && y >= bounds.y()
                            && y <= bounds.y() + bounds.height()
                    })
                    .unwrap_or(false);
                if !inside {
                    handler(ActionRequest::Dismiss);
                }
            });
        }
        window.add_controller(click);

        let keys = gtk::EventControllerKey::new();
        {
            let handler = handler.clone();
            keys.connect_key_pressed(move |_keys, keyval, _keycode, _modifiers| {
                if keyval == gdk::Key::Escape {
                    handler(ActionRequest::Dismiss);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
        }
        window.add_controller(keys);

        window.connect_close_request(|window| {
            window.set_visible(false);
            glib::Propagation::Stop
        });

        Self {
            window,
            panel,
            tabs: tab_buttons,
            title,
            shortcut,
            wheel,
            status,
            footer,
        }
    }

    pub fn render(&self, view: ActionView) {
        self.title.set_label(&view.title);
        self.shortcut.set_label(&view.shortcut.replace('+', " + "));
        for (category, button) in &self.tabs {
            if *category == view.category {
                button.add_css_class("active");
            } else {
                button.remove_css_class("active");
            }
        }

        if let Some(status) = view.status.clone() {
            self.status.set_label(&status);
            self.status.set_visible(true);
        } else {
            self.status.set_visible(false);
        }

        self.footer
            .set_label("Drop files anywhere on this panel, then choose a format  ·  Esc closes");

        self.wheel.set_state(crate::action_wheel::WheelState {
            targets: view.targets,
            source_label: view.source_label,
            empty_hint: view.empty_hint,
        });
        self.wheel.set_drag_hot(false);
        self.apply_theme();
    }

    fn apply_theme(&self) {
        if theme::is_dark() {
            self.panel.add_css_class("rd-panel-dark");
        } else {
            self.panel.remove_css_class("rd-panel-dark");
        }
        self.panel.queue_draw();
        self.wheel.queue_draw();
    }

    pub fn present(&self) {
        self.apply_theme();
        self.window.present();
        self.wheel.focus();
        let wheel = self.wheel.area.clone();
        glib::idle_add_local_once(move || {
            wheel.grab_focus();
        });
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
}

fn install_drop_target(
    widget: &impl IsA<gtk::Widget>,
    wheel: Rc<ActionWheel>,
    handler: RequestHandler,
    primary: bool,
) {
    let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    {
        let widget_ref = widget.clone().upcast::<gtk::Widget>();
        let wheel_hot = wheel.clone();
        target.connect_enter(move |_target, _x, _y| {
            widget_ref.add_css_class("rd-drop-hot");
            wheel_hot.set_drag_hot(true);
            gdk::DragAction::COPY
        });
    }
    {
        let widget_ref = widget.clone().upcast::<gtk::Widget>();
        let wheel_hot = wheel.clone();
        target.connect_leave(move |_target| {
            widget_ref.remove_css_class("rd-drop-hot");
            wheel_hot.set_drag_hot(false);
        });
    }
    {
        let handler = handler.clone();
        let widget_ref = widget.clone().upcast::<gtk::Widget>();
        let wheel_hot = wheel.clone();
        target.connect_drop(move |_target, value, _x, _y| {
            widget_ref.remove_css_class("rd-drop-hot");
            wheel_hot.set_drag_hot(false);
            let paths = value
                .get::<gdk::FileList>()
                .map(|list| {
                    list.files()
                        .into_iter()
                        .filter_map(|file| file.path())
                        .collect::<Vec<PathBuf>>()
                })
                .unwrap_or_default();
            if paths.is_empty() {
                return false;
            }
            handler(ActionRequest::Dropped(paths));
            true
        });
    }
    widget.add_controller(target);

    if primary {
        let uri_target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);
        let handler = handler.clone();
        let widget_ref = widget.clone().upcast::<gtk::Widget>();
        uri_target.connect_drop(move |_target, value, _x, _y| {
            widget_ref.remove_css_class("rd-drop-hot");
            let Ok(text) = value.get::<String>() else {
                return false;
            };
            let paths = parse_uri_list(&text);
            if paths.is_empty() {
                return false;
            }
            handler(ActionRequest::Dropped(paths));
            true
        });
        widget.add_controller(uri_target);
    }
}

fn parse_uri_list(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("file://"))
        .filter_map(|line| glib::filename_from_uri(line).ok().map(|(path, _)| path))
        .collect()
}
