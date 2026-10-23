//! Tangerine-style radial format picker.
//!
//! The wheel is drawn with Cairo so the geometry, gaps and accent colour stay
//! identical across themes; the surrounding chrome is regular GTK widgets.

use std::cell::RefCell;
use std::f64::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;

use gtk::cairo;
use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use pangocairo::functions as pangocairo;

use crate::theme;

const ACCENT: (f64, f64, f64) = (1.0, 0.353, 0.0);

#[derive(Debug, Clone)]
pub struct WheelTarget {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct WheelState {
    pub targets: Vec<WheelTarget>,
    pub source_label: String,
    pub empty_hint: String,
}

#[derive(Debug, Clone)]
pub enum WheelEvent {
    Run(String),
    Unavailable(String),
    Dismiss,
}

type Handler = Rc<dyn Fn(WheelEvent)>;
type HandlerSlot = Rc<RefCell<Option<Handler>>>;

#[derive(Default)]
struct Interaction {
    hover: Option<usize>,
    selected: Option<usize>,
    drag_hot: bool,
}

struct Geometry {
    cx: f64,
    cy: f64,
    outer: f64,
    inner: f64,
    caption_y: f64,
}

impl Geometry {
    fn new(width: f64, height: f64) -> Self {
        let outer = (width.min(height) * 0.40).clamp(128.0, 176.0);
        let inner = outer * 0.46;
        let cx = width / 2.0;
        let cy = height / 2.0 - 6.0;
        Self {
            cx,
            cy,
            outer,
            inner,
            caption_y: cy + outer + 24.0,
        }
    }
}

enum Hit {
    Wedge(usize),
    Center,
    Outside,
}

fn hit_test(geometry: &Geometry, count: usize, x: f64, y: f64) -> Hit {
    let dx = x - geometry.cx;
    let dy = y - geometry.cy;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= geometry.inner {
        return Hit::Center;
    }
    if distance > geometry.outer || count == 0 {
        return Hit::Outside;
    }
    let step = TAU / count as f64;
    let angle = (dy.atan2(dx) + FRAC_PI_2).rem_euclid(TAU);
    let index = ((angle + step / 2.0) / step).floor() as usize % count;
    Hit::Wedge(index)
}

pub struct ActionWheel {
    pub area: gtk::DrawingArea,
    state: Rc<RefCell<WheelState>>,
    interaction: Rc<RefCell<Interaction>>,
    handlers: HandlerSlot,
}

impl ActionWheel {
    pub fn new() -> Self {
        let state = Rc::new(RefCell::new(WheelState::default()));
        let interaction = Rc::new(RefCell::new(Interaction::default()));
        let handlers: HandlerSlot = Rc::new(RefCell::new(None));

        let area = gtk::DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.set_can_focus(true);
        area.set_focusable(true);
        area.set_size_request(420, 360);
        area.set_cursor_from_name(Some("default"));

        {
            let state = state.clone();
            let interaction = interaction.clone();
            area.set_draw_func(move |_area, cr, width, height| {
                draw(cr, width, height, &state.borrow(), &interaction.borrow());
            });
        }

        let motion = gtk::EventControllerMotion::new();
        {
            let state = state.clone();
            let interaction = interaction.clone();
            let area_ref = area.clone();
            motion.connect_motion(move |_motion, x, y| {
                let count = state.borrow().targets.len();
                let geometry = Geometry::new(area_ref.width() as f64, area_ref.height() as f64);
                let hit = hit_test(&geometry, count, x, y);
                let hover = match hit {
                    Hit::Wedge(index) => Some(index),
                    _ => None,
                };
                let mut interaction = interaction.borrow_mut();
                if interaction.hover != hover {
                    interaction.hover = hover;
                    drop(interaction);
                    area_ref.queue_draw();
                }
                let (available, reason) = match hover {
                    Some(index) => {
                        let state = state.borrow();
                        let target = &state.targets[index];
                        (target.available, target.reason.clone())
                    }
                    None => (true, None),
                };
                area_ref.set_cursor_from_name(Some(match (hover.is_some(), available) {
                    (true, true) => "pointer",
                    (true, false) => "not-allowed",
                    _ => "default",
                }));
                area_ref.set_tooltip_text(reason.as_deref());
            });
        }
        area.add_controller(motion);

        let click = gtk::GestureClick::new();
        click.set_button(0);
        {
            let state = state.clone();
            let interaction = interaction.clone();
            let handlers = handlers.clone();
            let area_ref = area.clone();
            click.connect_pressed(move |gesture, _n, x, y| {
                let button = gesture.current_button();
                let geometry = Geometry::new(area_ref.width() as f64, area_ref.height() as f64);
                let count = state.borrow().targets.len();
                match (hit_test(&geometry, count, x, y), button) {
                    (Hit::Wedge(index), 1) => {
                        interaction.borrow_mut().selected = Some(index);
                        area_ref.queue_draw();
                        let state = state.borrow();
                        let target = &state.targets[index];
                        let id = target.id.clone();
                        let reason = target.reason.clone();
                        let available = target.available;
                        drop(state);
                        if available {
                            emit(&handlers, WheelEvent::Run(id));
                        } else if let Some(reason) = reason {
                            emit(&handlers, WheelEvent::Unavailable(reason));
                        }
                    }
                    (Hit::Center, 3) => emit(&handlers, WheelEvent::Dismiss),
                    (Hit::Outside, 3) => emit(&handlers, WheelEvent::Dismiss),
                    _ => {}
                }
            });
        }
        area.add_controller(click);

        let keys = gtk::EventControllerKey::new();
        {
            let state = state.clone();
            let interaction = interaction.clone();
            let handlers = handlers.clone();
            let area_ref = area.clone();
            keys.connect_key_pressed(move |_keys, keyval, _keycode, _modifiers| {
                let count = state.borrow().targets.len();
                if count == 0 {
                    if keyval == gdk::Key::Escape {
                        emit(&handlers, WheelEvent::Dismiss);
                        return glib::Propagation::Stop;
                    }
                    return glib::Propagation::Proceed;
                }
                let mut selected = interaction.borrow().selected.or(interaction.borrow().hover);
                match keyval {
                    gdk::Key::Escape => {
                        emit(&handlers, WheelEvent::Dismiss);
                        return glib::Propagation::Stop;
                    }
                    gdk::Key::Left | gdk::Key::Up => {
                        selected = Some(selected.map(|i| (i + count - 1) % count).unwrap_or(0));
                    }
                    gdk::Key::Right | gdk::Key::Down | gdk::Key::Tab => {
                        selected = Some(selected.map(|i| (i + 1) % count).unwrap_or(0));
                    }
                    gdk::Key::Return | gdk::Key::KP_Enter => {
                        if let Some(index) = selected {
                            let state = state.borrow();
                            let target = &state.targets[index];
                            if target.available {
                                let id = target_id(&state, index);
                                drop(state);
                                emit(&handlers, WheelEvent::Run(id));
                            } else if let Some(reason) = target.reason.clone() {
                                drop(state);
                                emit(&handlers, WheelEvent::Unavailable(reason));
                            }
                        }
                        return glib::Propagation::Stop;
                    }
                    key => {
                        if let Some(ch) = key.to_unicode() {
                            if let Some(digit) = ch.to_digit(10) {
                                let index = if digit == 0 { 9 } else { digit as usize - 1 };
                                if index < count {
                                    let state = state.borrow();
                                    let target = &state.targets[index];
                                    if target.available {
                                        let id = target_id(&state, index);
                                        drop(state);
                                        emit(&handlers, WheelEvent::Run(id));
                                    } else if let Some(reason) = target.reason.clone() {
                                        drop(state);
                                        emit(&handlers, WheelEvent::Unavailable(reason));
                                    }
                                    return glib::Propagation::Stop;
                                }
                            }
                        }
                        return glib::Propagation::Proceed;
                    }
                }
                interaction.borrow_mut().selected = selected;
                interaction.borrow_mut().hover = None;
                area_ref.queue_draw();
                glib::Propagation::Stop
            });
        }
        area.add_controller(keys);

        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        {
            let state = state.clone();
            let interaction = interaction.clone();
            let area_ref = area.clone();
            scroll.connect_scroll(move |_scroll, _dx, dy| {
                let count = state.borrow().targets.len();
                if count == 0 || dy == 0.0 {
                    return glib::Propagation::Proceed;
                }
                let current = interaction
                    .borrow()
                    .hover
                    .or(interaction.borrow().selected)
                    .unwrap_or(0);
                let next = if dy > 0.0 {
                    (current + 1) % count
                } else {
                    (current + count - 1) % count
                };
                interaction.borrow_mut().hover = Some(next);
                area_ref.queue_draw();
                glib::Propagation::Stop
            });
        }
        area.add_controller(scroll);

        Self {
            area,
            state,
            interaction,
            handlers,
        }
    }

    pub fn set_handler(&self, handler: Rc<dyn Fn(WheelEvent)>) {
        *self.handlers.borrow_mut() = Some(handler);
    }

    pub fn set_state(&self, state: WheelState) {
        *self.state.borrow_mut() = state;
        self.interaction.borrow_mut().hover = None;
        self.interaction.borrow_mut().selected = None;
        self.area.queue_draw();
    }

    pub fn set_drag_hot(&self, hot: bool) {
        if self.interaction.borrow().drag_hot != hot {
            self.interaction.borrow_mut().drag_hot = hot;
            self.area.queue_draw();
        }
    }

    pub fn focus(&self) {
        self.area.grab_focus();
    }

    pub fn queue_draw(&self) {
        self.area.queue_draw();
    }
}

fn target_id(state: &WheelState, index: usize) -> String {
    state
        .targets
        .get(index)
        .map(|target| target.id.clone())
        .unwrap_or_default()
}

fn emit(handlers: &HandlerSlot, event: WheelEvent) {
    if let Some(handler) = handlers.borrow().as_ref() {
        handler(event);
    }
}

pub(crate) struct Palette {
    pub(crate) ring_inner: (f64, f64, f64),
    pub(crate) ring_outer: (f64, f64, f64),
    pub(crate) text: (f64, f64, f64),
    pub(crate) muted: (f64, f64, f64),
    pub(crate) center: (f64, f64, f64),
    pub(crate) rim: (f64, f64, f64),
    pub(crate) shadow_alpha: f64,
}

pub(crate) fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            ring_inner: (0.235, 0.235, 0.247),
            ring_outer: (0.153, 0.153, 0.165),
            text: (0.96, 0.96, 0.97),
            muted: (0.60, 0.60, 0.64),
            center: (0.125, 0.125, 0.137),
            rim: (1.0, 1.0, 1.0),
            shadow_alpha: 0.45,
        }
    } else {
        Palette {
            ring_inner: (0.949, 0.949, 0.953),
            ring_outer: (0.855, 0.855, 0.863),
            text: (0.11, 0.11, 0.12),
            muted: (0.40, 0.40, 0.43),
            center: (1.0, 1.0, 1.0),
            rim: (0.0, 0.0, 0.0),
            shadow_alpha: 0.14,
        }
    }
}

fn draw(
    cr: &cairo::Context,
    width: i32,
    height: i32,
    state: &WheelState,
    interaction: &Interaction,
) {
    let palette = palette(theme::is_dark());
    let geometry = Geometry::new(width as f64, height as f64);
    cr.set_antialias(cairo::Antialias::Best);

    // Soft halo so the wheel separates from the panel without a hard stroke.
    let halo = cairo::RadialGradient::new(
        geometry.cx,
        geometry.cy + 6.0,
        geometry.outer * 0.85,
        geometry.cx,
        geometry.cy + 6.0,
        geometry.outer * 1.18,
    );
    halo.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, palette.shadow_alpha);
    halo.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    let _ = cr.set_source(&halo);
    cr.paint().ok();

    if state.targets.is_empty() {
        draw_empty(cr, &geometry, state, &palette, interaction.drag_hot);
        return;
    }

    let count = state.targets.len();
    let step = TAU / count as f64;
    let gap = (step * 0.075).clamp(0.016, 0.055);
    let active = interaction.hover.or(interaction.selected);

    for (index, target) in state.targets.iter().enumerate() {
        let start = -FRAC_PI_2 - step / 2.0 + gap / 2.0 + index as f64 * step;
        let end = start + step - gap;
        let is_active = active == Some(index);
        let is_available = target.available;

        let gradient = cairo::RadialGradient::new(
            geometry.cx,
            geometry.cy,
            geometry.inner,
            geometry.cx,
            geometry.cy,
            geometry.outer,
        );
        if is_active && is_available {
            gradient.add_color_stop_rgb(0.0, ACCENT.0, ACCENT.1, ACCENT.2);
            gradient.add_color_stop_rgb(1.0, 0.95, 0.26, 0.0);
        } else if is_available {
            gradient.add_color_stop_rgb(
                0.0,
                palette.ring_inner.0,
                palette.ring_inner.1,
                palette.ring_inner.2,
            );
            gradient.add_color_stop_rgb(
                1.0,
                palette.ring_outer.0,
                palette.ring_outer.1,
                palette.ring_outer.2,
            );
        } else {
            gradient.add_color_stop_rgba(
                0.0,
                palette.ring_inner.0,
                palette.ring_inner.1,
                palette.ring_inner.2,
                0.45,
            );
            gradient.add_color_stop_rgba(
                1.0,
                palette.ring_outer.0,
                palette.ring_outer.1,
                palette.ring_outer.2,
                0.35,
            );
        }

        wedge_path(cr, &geometry, start, end);
        let _ = cr.set_source(&gradient);
        cr.fill().ok();

        let mid = (start + end) / 2.0;
        let radius = (geometry.outer + geometry.inner) / 2.0;
        let x = geometry.cx + radius * mid.cos();
        let y = geometry.cy + radius * mid.sin();
        let color = if is_active && is_available {
            (1.0, 1.0, 1.0)
        } else if is_available {
            palette.text
        } else {
            palette.muted
        };
        draw_text(
            cr,
            &target.label,
            x,
            y,
            12.5,
            pango::Weight::Semibold,
            color,
            if is_available { 1.0 } else { 0.55 },
            !is_available,
        );
    }

    // Center chip. Start a fresh path: text drawing leaves a current point
    // behind, and cairo would otherwise connect it to the arc with a chord.
    cr.new_path();
    cr.arc(geometry.cx, geometry.cy, geometry.inner * 0.86, 0.0, TAU);
    cr.set_source_rgba(palette.center.0, palette.center.1, palette.center.2, 1.0);
    cr.fill_preserve().ok();
    cr.set_source_rgba(palette.rim.0, palette.rim.1, palette.rim.2, 0.10);
    cr.set_line_width(1.0);
    cr.stroke().ok();

    let center_label = match interaction.hover {
        Some(index) => state
            .targets
            .get(index)
            .map(|target| target.label.clone())
            .unwrap_or_else(|| state.source_label.clone()),
        None => {
            if state.source_label.is_empty() {
                "DROP".into()
            } else {
                state.source_label.clone()
            }
        }
    };
    draw_text(
        cr,
        &center_label,
        geometry.cx,
        geometry.cy,
        13.0,
        pango::Weight::Bold,
        palette.text,
        1.0,
        false,
    );

    let caption = match (interaction.hover, interaction.selected) {
        (Some(index), _) | (None, Some(index)) => state
            .targets
            .get(index)
            .map(|target| {
                if target.available {
                    format!("Convert to {}", target.label)
                } else {
                    target
                        .reason
                        .clone()
                        .unwrap_or_else(|| "Not available".into())
                }
            })
            .unwrap_or_else(|| "Choose a format".into()),
        _ => "Choose a format".into(),
    };
    draw_text(
        cr,
        &caption,
        geometry.cx,
        geometry.caption_y,
        12.0,
        pango::Weight::Normal,
        palette.muted,
        0.9,
        false,
    );
}

fn draw_empty(
    cr: &cairo::Context,
    geometry: &Geometry,
    state: &WheelState,
    palette: &Palette,
    drag_hot: bool,
) {
    let (r, g, b) = if drag_hot { ACCENT } else { palette.muted };
    cr.set_dash(&[6.0, 7.0], 0.0);
    cr.set_line_width(2.0);
    cr.set_source_rgba(r, g, b, if drag_hot { 0.95 } else { 0.45 });
    cr.arc(geometry.cx, geometry.cy, geometry.outer, 0.0, TAU);
    cr.stroke().ok();
    cr.set_dash(&[], 0.0);

    cr.arc(geometry.cx, geometry.cy, geometry.inner * 0.86, 0.0, TAU);
    cr.set_source_rgba(palette.center.0, palette.center.1, palette.center.2, 1.0);
    cr.fill_preserve().ok();
    cr.set_source_rgba(palette.rim.0, palette.rim.1, palette.rim.2, 0.08);
    cr.set_line_width(1.0);
    cr.stroke().ok();

    let label = if state.empty_hint.is_empty() {
        "Drop files"
    } else {
        state.empty_hint.as_str()
    };
    draw_text(
        cr,
        label,
        geometry.cx,
        geometry.cy - 6.0,
        14.0,
        pango::Weight::Semibold,
        palette.text,
        1.0,
        false,
    );
    draw_text(
        cr,
        "from your file manager",
        geometry.cx,
        geometry.cy + 16.0,
        10.5,
        pango::Weight::Normal,
        palette.muted,
        0.9,
        false,
    );
    draw_text(
        cr,
        "or right-click a file → RadialDeck",
        geometry.cx,
        geometry.caption_y,
        11.0,
        pango::Weight::Normal,
        palette.muted,
        0.75,
        false,
    );
}

fn wedge_path(cr: &cairo::Context, geometry: &Geometry, start: f64, end: f64) {
    cr.new_path();
    cr.arc(geometry.cx, geometry.cy, geometry.outer, start, end);
    cr.arc_negative(geometry.cx, geometry.cy, geometry.inner, end, start);
    cr.close_path();
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    cr: &cairo::Context,
    text: &str,
    x: f64,
    y: f64,
    size: f64,
    weight: pango::Weight,
    color: (f64, f64, f64),
    alpha: f64,
    strike: bool,
) {
    let layout = pangocairo::create_layout(cr);
    layout.set_text(text);
    let mut font = pango::FontDescription::from_string("Sans");
    font.set_absolute_size(size * f64::from(pango::SCALE));
    font.set_weight(weight);
    layout.set_font_description(Some(&font));
    if strike {
        let attrs = pango::AttrList::new();
        attrs.insert(pango::AttrInt::new_strikethrough(true));
        layout.set_attributes(Some(&attrs));
    }
    let (text_width, text_height) = layout.pixel_size();
    cr.set_source_rgba(color.0, color.1, color.2, alpha);
    cr.move_to(x - text_width as f64 / 2.0, y - text_height as f64 / 2.0);
    pangocairo::show_layout(cr, &layout);
    // move_to() opens a path subpath at the text anchor. Clear it so a later
    // arc/fill/stroke cannot draw a line from the text to its own path.
    cr.new_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_text_leaves_no_path() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 32, 32).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        draw_text(
            &cr,
            "PDF",
            16.0,
            16.0,
            12.0,
            pango::Weight::Semibold,
            (1.0, 1.0, 1.0),
            1.0,
            false,
        );
        assert!(!cr.has_current_point().unwrap());
    }
}
