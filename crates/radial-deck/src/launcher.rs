//! Full-screen overlay that hosts the project-folder wheel.

use std::cell::Cell;
use std::rc::{Rc, Weak};

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::controller::Controller;
use crate::project_tabs::ProjectTabs;
use crate::theme;
use crate::wheel::{Action, Wheel, WheelData};

const PANEL_WIDTH: i32 = 980;
const WHEEL_WIDTH: i32 = 900;
const WHEEL_HEIGHT: i32 = 380;

pub struct Launcher {
    window: gtk::ApplicationWindow,
    panel: gtk::Box,
    tabs: ProjectTabs,
    title: gtk::Label,
    shortcut: gtk::Label,
    caption: gtk::Label,
    wheel: Wheel,
    footer: gtk::Label,
    visible: Rc<Cell<bool>>,
    destroyed: Rc<Cell<bool>>,
}

impl Launcher {
    pub fn new(app: &gtk::Application, controller: Weak<Controller>) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("RadialDeck")
            .decorated(false)
            .build();
        window.add_css_class("rd-overlay");
        window.set_can_focus(true);

        let tabs = ProjectTabs::new();
        {
            let target = controller.clone();
            tabs.set_handler(Rc::new(move |id| {
                if let Some(controller) = target.upgrade() {
                    controller.set_active_project(id);
                }
            }));
        }
        let manage = gtk::Button::with_label("Manage");
        manage.add_css_class("rd-rail-action");
        manage.set_tooltip_text(Some("Manage projects and folders"));
        {
            let target = controller.clone();
            manage.connect_clicked(move |_| {
                if let Some(controller) = target.upgrade() {
                    controller.open_manage();
                }
            });
        }
        let rail = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        rail.set_halign(gtk::Align::Center);
        rail.append(&tabs.container);
        rail.append(&manage);

        let title = gtk::Label::new(None);
        title.add_css_class("rd-heading");
        title.set_halign(gtk::Align::Center);
        title.set_margin_top(8);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(48);

        let chip = gtk::Box::new(gtk::Orientation::Vertical, 6);
        chip.set_halign(gtk::Align::Center);
        chip.set_margin_top(10);
        let key = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        key.add_css_class("rd-key");
        let shortcut = gtk::Label::new(None);
        shortcut.add_css_class("rd-key-label");
        key.append(&shortcut);
        chip.append(&key);
        let caption = gtk::Label::new(None);
        caption.add_css_class("rd-caption");
        chip.append(&caption);

        let wheel = Wheel::new();
        wheel.area.add_css_class("rd-wheel");
        wheel.area.set_size_request(WHEEL_WIDTH, WHEEL_HEIGHT);

        let footer = gtk::Label::new(None);
        footer.add_css_class("rd-footer");
        footer.set_halign(gtk::Align::Center);
        footer.set_margin_top(2);

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("rd-panel");
        panel.set_halign(gtk::Align::Center);
        panel.set_valign(gtk::Align::Center);
        panel.set_width_request(PANEL_WIDTH);
        panel.set_margin_top(24);
        panel.set_margin_bottom(24);
        panel.append(&rail);
        panel.append(&title);
        panel.append(&chip);
        panel.append(&wheel.area);
        panel.append(&footer);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("rd-scrim");
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_halign(gtk::Align::Fill);
        root.set_valign(gtk::Align::Fill);
        root.append(&panel);
        window.set_child(Some(&root));

        let target = controller.clone();
        wheel.set_action_handler(Rc::new(move |action: Action| {
            if let Some(controller) = target.upgrade() {
                controller.handle_wheel_action(action);
            }
        }));

        let layer_shell = crate::surface::layer_shell_available();
        tracing::info!("layer-shell supported: {layer_shell}");
        if layer_shell {
            window.init_layer_shell();
            window.set_namespace(Some("radial-deck-folders"));
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
            window.set_resizable(false);
            window.set_default_size(1040, 820);
        }

        {
            let panel = panel.clone();
            let window_ref = window.clone();
            let target = controller.clone();
            let click = gtk::GestureClick::new();
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
                    if let Some(controller) = target.upgrade() {
                        controller.hide_launcher();
                    }
                }
            });
            window.add_controller(click);
        }

        {
            let target = controller.clone();
            let tabs = tabs.clone();
            let wheel_area = wheel.area.clone();
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            keys.connect_key_pressed(move |_keys, keyval, _keycode, modifiers| {
                if keyval == gdk::Key::Escape {
                    if wheel_area.has_focus() {
                        return glib::Propagation::Proceed;
                    }
                    if let Some(controller) = target.upgrade() {
                        controller.hide_launcher();
                    }
                    glib::Propagation::Stop
                } else if keyval == gdk::Key::Tab
                    && !modifiers.contains(gdk::ModifierType::SHIFT_MASK)
                    && wheel_area.has_focus()
                {
                    tabs.focus_first();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
            window.add_controller(keys);
        }

        let visible = Rc::new(Cell::new(false));
        let destroyed = Rc::new(Cell::new(false));
        {
            let visible = visible.clone();
            window.connect_close_request(move |window| {
                window.set_visible(false);
                visible.set(false);
                glib::Propagation::Stop
            });
        }
        {
            let visible = visible.clone();
            let destroyed = destroyed.clone();
            window.connect_destroy(move |_| {
                visible.set(false);
                destroyed.set(true);
            });
        }

        Self {
            window,
            panel,
            tabs,
            title,
            shortcut,
            caption,
            wheel,
            footer,
            visible,
            destroyed,
        }
    }

    pub fn show(&self, data: WheelData) {
        if self.destroyed.get() {
            tracing::warn!("folder overlay was destroyed; refusing to show");
            return;
        }
        self.apply_theme();
        self.set_data(data);
        self.wheel.reset();
        self.visible.set(true);
        if crate::surface::layer_shell_available() {
            crate::surface::anchor_to_pointer_monitor(&self.window);
        } else {
            crate::surface::fill_monitor_for_pointer(&self.window);
        }
        self.window.set_visible(true);
        self.window.present();
        self.wheel.focus();

        let area = self.wheel.area.clone();
        let window = self.window.clone();
        let visible = self.visible.clone();
        glib::idle_add_local_once(move || {
            area.grab_focus();
        });
        glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
            if visible.get() && !window.is_mapped() {
                tracing::warn!("folder overlay was not mapped; presenting again");
                window.present();
            }
        });
    }

    pub fn hide(&self) {
        self.visible.set(false);
        self.window.set_visible(false);
    }

    pub fn is_visible(&self) -> bool {
        self.visible.get() && !self.destroyed.get()
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub fn set_data(&self, data: WheelData) {
        self.tabs.render(&data.projects, data.active_project);
        self.title.set_text(&data.project_name);
        self.shortcut.set_label(&data.shortcut.replace('+', " + "));
        self.caption.set_text(&format!(
            "{} folder{}",
            data.folders.len(),
            if data.folders.len() == 1 { "" } else { "s" }
        ));

        let folder_hint = if data.folders.len() > 1 {
            "1–9 opens  ·  "
        } else {
            ""
        };
        self.footer.set_label(&format!(
            "{folder_hint}Type to search  ·  Enter opens  ·  Ctrl+Enter terminal  ·  Esc closes"
        ));
        self.wheel.set_data(data);
        self.wheel.focus();
    }

    pub fn update_theme(&self) {
        self.apply_theme();
        self.wheel.update_theme();
    }

    fn apply_theme(&self) {
        if theme::is_dark() {
            self.panel.add_css_class("rd-panel-dark");
        } else {
            self.panel.remove_css_class("rd-panel-dark");
        }
        self.wheel.update_theme();
    }
}
