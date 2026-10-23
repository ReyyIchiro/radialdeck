use std::rc::{Rc, Weak};

use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::controller::Controller;
use crate::wheel::{Action, Wheel, WheelData};

pub struct Launcher {
    window: gtk::ApplicationWindow,
    wheel: Wheel,
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

        let wheel = Wheel::new();
        window.set_child(Some(&wheel.area));

        let target = controller.clone();
        wheel.set_action_handler(Rc::new(move |action: Action| {
            if let Some(controller) = target.upgrade() {
                controller.handle_wheel_action(action);
            }
        }));

        let layer_shell = gtk4_layer_shell::is_supported();
        tracing::info!("layer-shell supported: {layer_shell}");
        if layer_shell {
            window.init_layer_shell();
            window.set_namespace(Some("radial-deck"));
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
            window.set_default_size(760, 760);
        }

        window.connect_close_request(|window| {
            window.set_visible(false);
            glib::Propagation::Stop
        });

        Self { window, wheel }
    }

    pub fn show(&self, data: WheelData) {
        self.wheel.set_data(data);
        self.wheel.reset();
        self.window.present();

        let area = self.wheel.area.clone();
        self.wheel.focus();
        glib::idle_add_local_once(move || {
            area.grab_focus();
        });
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    pub fn set_data(&self, data: WheelData) {
        self.wheel.set_data(data);
    }

    pub fn update_theme(&self) {
        self.wheel.update_theme();
    }
}
