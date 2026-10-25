//! Wayland surface capability checks and pointer-monitor placement shared by
//! the overlays.

use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

/// Layer-shell is only usable on a Wayland display. `gtk4_layer_shell`'s own
/// `is_supported` also reports the library as available on X11, where calling
/// `init_layer_shell` only logs a GTK critical and never maps a surface.
pub fn layer_shell_available() -> bool {
    if !gtk4_layer_shell::is_supported() {
        return false;
    }
    gtk::gdk::Display::default()
        .map(|display| display.name().starts_with("wayland"))
        .unwrap_or(false)
}

fn pointer_monitor() -> Option<gtk::gdk::Monitor> {
    let display = gtk::gdk::Display::default()?;
    display
        .default_seat()
        .and_then(|seat| seat.pointer())
        .and_then(|pointer| pointer.surface_at_position().0)
        .and_then(|surface| display.monitor_at_surface(&surface))
}

/// Places a layer-shell surface on the monitor under the pointer so the wheel
/// opens where the user is looking, not on the compositor's default output.
pub fn anchor_to_pointer_monitor(window: &impl IsA<gtk::Window>) {
    if let Some(monitor) = pointer_monitor() {
        window.set_monitor(Some(&monitor));
    }
}

/// Without layer-shell there is no protocol for placing a window at an
/// absolute position, so the fallback covers the monitor under the pointer
/// instead of anchoring a layer-shell surface.
pub fn fill_monitor_for_pointer(window: &impl IsA<gtk::Window>) {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let monitor = pointer_monitor().or_else(|| {
        display
            .monitors()
            .item(0)
            .and_downcast::<gtk::gdk::Monitor>()
    });
    if let Some(monitor) = monitor {
        window.fullscreen_on_monitor(&monitor);
    }
}
