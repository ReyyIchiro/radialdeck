//! Small, non-focusable panel that shows the running operation.
//!
//! It never takes keyboard focus: the user may keep typing while a long FFmpeg
//! run finishes. Only the pointer can reach the Cancel button.

use std::rc::Rc;

use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::surface;

pub struct ProgressWindow {
    window: gtk::Window,
    title: gtk::Label,
    detail: gtk::Label,
    bar: gtk::ProgressBar,
    cancel: gtk::Button,
    visible: Rc<std::cell::Cell<bool>>,
}

impl ProgressWindow {
    pub fn new(app: &gtk::Application) -> Self {
        let window = gtk::Window::builder()
            .application(app)
            .title("RadialDeck")
            .decorated(false)
            .resizable(false)
            .build();
        window.add_css_class("rd-progress-window");

        if surface::layer_shell_available() {
            window.init_layer_shell();
            window.set_namespace(Some("radial-deck-progress"));
            window.set_layer(gtk4_layer_shell::Layer::Overlay);
            // Clicks reach the Cancel button, but the keyboard stays with the
            // user's current window.
            window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
            window.set_anchor(gtk4_layer_shell::Edge::Bottom, true);
            window.set_anchor(gtk4_layer_shell::Edge::Right, true);
            window.set_margin(gtk4_layer_shell::Edge::Bottom, 24);
            window.set_margin(gtk4_layer_shell::Edge::Right, 24);
            window.set_exclusive_zone(0);
        } else {
            window.set_default_size(360, -1);
        }

        let title = gtk::Label::new(None);
        title.add_css_class("rd-progress-title");
        title.set_halign(gtk::Align::Start);
        title.set_xalign(0.0);

        let detail = gtk::Label::new(None);
        detail.add_css_class("rd-subtle");
        detail.set_halign(gtk::Align::Start);
        detail.set_xalign(0.0);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        detail.set_max_width_chars(44);

        let bar = gtk::ProgressBar::new();
        bar.set_show_text(false);

        let cancel = gtk::Button::with_label("Cancel");
        cancel.add_css_class("rd-progress-cancel");
        cancel.set_halign(gtk::Align::End);

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        panel.add_css_class("rd-card");
        panel.set_width_request(340);
        panel.set_margin_top(14);
        panel.set_margin_bottom(14);
        panel.set_margin_start(16);
        panel.set_margin_end(16);
        panel.append(&title);
        panel.append(&detail);
        panel.append(&bar);
        panel.append(&cancel);
        window.set_child(Some(&panel));

        Self {
            window,
            title,
            detail,
            bar,
            cancel,
            visible: Rc::new(std::cell::Cell::new(false)),
        }
    }

    pub fn connect_cancel(&self, handler: impl Fn() + 'static) {
        self.cancel.connect_clicked(move |_| handler());
    }

    pub fn show_operation(&self, title: &str, total: usize) {
        self.title.set_text(title);
        self.detail.set_text("Starting…");
        self.bar.set_fraction(0.0);
        self.cancel.set_sensitive(true);
        self.cancel.set_label("Cancel");
        if total > 1 {
            self.bar.set_show_text(true);
            self.bar.set_text(Some(&format!("0 / {total}")));
        } else {
            self.bar.set_show_text(false);
        }
        self.visible.set(true);
        self.window.present();
    }

    pub fn update(&self, progress: &deck_core::Progress) {
        if !self.visible.get() {
            return;
        }
        if !progress.current.is_empty() {
            self.detail.set_text(&progress.current);
        } else if let Some(detail) = &progress.detail {
            self.detail.set_text(detail);
        }
        if progress.done > 0 && progress.total > 0 {
            let fraction = (progress.done as f64 / progress.total as f64).clamp(0.0, 1.0);
            self.bar.set_fraction(fraction);
            if progress.total > 1 {
                self.bar
                    .set_text(Some(&format!("{} / {}", progress.done, progress.total)));
            }
        }
    }

    pub fn finish(&self) {
        self.visible.set(false);
        self.window.set_visible(false);
    }
}
