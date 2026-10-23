//! Small widgets shared by the launcher and management surfaces.

use gtk::prelude::*;

use crate::theme;

/// A flat colour marker. The colour itself is the data, not decoration.
pub fn color_dot(color: &str, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("rd-color-dot");
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_valign(gtk::Align::Center);
    area.set_can_target(false);
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
