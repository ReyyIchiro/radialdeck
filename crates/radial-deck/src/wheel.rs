use std::cell::RefCell;
use std::f64::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;

use deck_core::Availability;
use gtk::gdk;
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use pangocairo::functions as pangocairo;
use uuid::Uuid;

use crate::action_wheel::{palette, Palette};
use crate::theme;

type ActionHandler = Rc<dyn Fn(Action)>;
type ActionSlot = Rc<RefCell<Option<ActionHandler>>>;

#[derive(Debug, Clone)]
pub struct WheelFolder {
    pub id: Uuid,
    pub label: String,
    pub color: Option<String>,
    pub availability: Availability,
}

#[derive(Debug, Clone)]
pub struct WheelData {
    pub project_name: String,
    pub project_color: String,
    pub project_total: usize,
    pub project_index: usize,
    pub shortcut: String,
    pub folders: Vec<WheelFolder>,
}

impl Default for WheelData {
    fn default() -> Self {
        Self {
            project_name: "My Project".into(),
            project_color: "#3b82f6".into(),
            project_total: 1,
            project_index: 0,
            shortcut: "CTRL+ALT+P".into(),
            folders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Open,
    Terminal,
    CopyPath,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Launch(Uuid, Intent),
    Dismiss,
    CycleProject(isize),
    Manage,
    Remove(Uuid),
}

#[derive(Default)]
struct State {
    data: WheelData,
    query: String,
    filtered: Vec<usize>,
    selected: Option<usize>,
    hover: Option<usize>,
}

impl State {
    fn rebuild(&mut self) {
        let query = self.query.trim().to_lowercase();
        self.filtered = self
            .data
            .folders
            .iter()
            .enumerate()
            .filter_map(|(index, folder)| {
                if query.is_empty() {
                    return Some(index);
                }
                let label = folder.label.to_lowercase();
                label.contains(&query).then_some(index)
            })
            .collect();
        if self
            .selected
            .is_some_and(|index| index >= self.filtered.len())
        {
            self.selected = None;
        }
    }

    fn folder_at(&self, position: usize) -> Option<Uuid> {
        self.filtered
            .get(position)
            .and_then(|index| self.data.folders.get(*index))
            .map(|folder| folder.id)
    }

    fn selected_folder(&self) -> Option<Uuid> {
        let position = self.selected.or(Some(0))?;
        self.folder_at(position)
    }
}

struct Geometry {
    cx: f64,
    cy: f64,
    outer: f64,
    inner: f64,
    /// Card used when exactly one folder is visible.
    single: Rect,
    /// Clickable project heading above the single-folder card.
    heading: Rect,
}

type Rect = (f64, f64, f64, f64);

impl Geometry {
    fn new(width: f64, height: f64) -> Self {
        let outer = (width.min(height) * 0.42).clamp(150.0, 245.0);
        let inner = outer * 0.38;
        let cx = width / 2.0;
        let cy = height / 2.0 - 4.0;
        let card_width = (width * 0.52).clamp(300.0, 400.0);
        let card_height = 76.0;
        Self {
            cx,
            cy,
            outer,
            inner,
            single: (cx - card_width / 2.0, cy + 2.0, card_width, card_height),
            heading: (cx - card_width / 2.0, cy - 104.0, card_width, 92.0),
        }
    }
}

fn hit_rect(rect: Rect, x: f64, y: f64) -> bool {
    let (rx, ry, rw, rh) = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    Sector(usize),
    Center,
    Outside,
}

fn hit_test(geo: &Geometry, state: &State, x: f64, y: f64) -> Hit {
    if state.filtered.len() == 1 {
        if hit_rect(geo.single, x, y) {
            return Hit::Sector(0);
        }
        if hit_rect(geo.heading, x, y) {
            return Hit::Center;
        }
        return Hit::Outside;
    }

    let dx = x - geo.cx;
    let dy = y - geo.cy;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= geo.inner {
        return Hit::Center;
    }

    if distance > geo.inner && distance <= geo.outer && !state.filtered.is_empty() {
        let angle = (dy.atan2(dx) + FRAC_PI_2).rem_euclid(TAU);
        let step = TAU / state.filtered.len() as f64;
        let index = (angle / step).floor() as usize;
        return Hit::Sector(index.min(state.filtered.len() - 1));
    }

    Hit::Outside
}

pub struct Wheel {
    pub area: gtk::DrawingArea,
    state: Rc<RefCell<State>>,
    actions: ActionSlot,
}

impl Wheel {
    pub fn new() -> Self {
        let state = Rc::new(RefCell::new(State::default()));
        let actions: ActionSlot = Rc::new(RefCell::new(None));
        let area = gtk::DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        area.set_can_focus(true);
        area.set_focusable(true);
        area.set_cursor_from_name(Some("default"));

        {
            let state = state.clone();
            area.set_draw_func(move |_area, cr, width, height| {
                draw(cr, width, height, &state.borrow());
            });
        }

        let motion = gtk::EventControllerMotion::new();
        {
            let state = state.clone();
            let area_ref = area.clone();
            motion.connect_motion(move |_motion, x, y| {
                let geo = Geometry::new(area_ref.width() as f64, area_ref.height() as f64);
                let mut state = state.borrow_mut();
                let hit = hit_test(&geo, &state, x, y);
                let hover = match hit {
                    Hit::Sector(index) => Some(index),
                    _ => None,
                };
                let inside_wheel = matches!(hit, Hit::Center | Hit::Sector(_));
                if hover != state.hover {
                    state.hover = hover;
                    drop(state);
                    area_ref.queue_draw();
                }
                area_ref.set_cursor_from_name(Some(if inside_wheel {
                    "pointer"
                } else {
                    "default"
                }));
            });
        }
        area.add_controller(motion);

        let click = gtk::GestureClick::new();
        click.set_button(0);
        {
            let state = state.clone();
            let actions = actions.clone();
            let wheel = area.clone();
            click.connect_pressed(move |gesture, _n_press, x, y| {
                let button = gesture.current_button();
                let modifiers = gesture.current_event_state();
                let geo = Geometry::new(wheel.width() as f64, wheel.height() as f64);
                let mut state_ref = state.borrow_mut();
                let hit = hit_test(&geo, &state_ref, x, y);

                match (hit, button) {
                    (Hit::Sector(position), 1 | 2) => {
                        state_ref.selected = Some(position);
                        let folder = state_ref.folder_at(position);
                        drop(state_ref);
                        wheel.queue_draw();
                        if let Some(folder) = folder {
                            let intent = if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                                Intent::Terminal
                            } else if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                                Intent::CopyPath
                            } else {
                                Intent::Open
                            };
                            emit(&actions, Action::Launch(folder, intent));
                        }
                    }
                    (Hit::Sector(position), 3) => {
                        let folder = state_ref.folder_at(position);
                        drop(state_ref);
                        if let Some(folder) = folder {
                            show_context_menu(&wheel, &actions, x, y, folder);
                        }
                    }
                    (Hit::Center, 1) => {
                        let empty = state_ref.data.folders.is_empty();
                        drop(state_ref);
                        if empty {
                            emit(&actions, Action::Manage);
                        } else {
                            emit(
                                &actions,
                                Action::CycleProject(
                                    if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                                        -1
                                    } else {
                                        1
                                    },
                                ),
                            );
                        }
                    }
                    (Hit::Center, 3) => {
                        drop(state_ref);
                        emit(&actions, Action::Manage);
                    }
                    (Hit::Outside, 1) => {
                        drop(state_ref);
                        emit(&actions, Action::Dismiss);
                    }
                    _ => {}
                }
            });
        }
        area.add_controller(click);

        let keys = gtk::EventControllerKey::new();
        {
            let state = state.clone();
            let actions = actions.clone();
            let area = area.clone();
            keys.connect_key_pressed(move |_keys, keyval, _keycode, modifiers| {
                let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
                let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
                let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                let mut state_ref = state.borrow_mut();
                let mut action = None;
                let mut handled = true;

                match keyval {
                    gdk::Key::Escape => {
                        if state_ref.query.is_empty() {
                            action = Some(Action::Dismiss);
                        } else {
                            state_ref.query.clear();
                            state_ref.rebuild();
                            state_ref.selected = None;
                        }
                    }
                    gdk::Key::BackSpace => {
                        state_ref.query.pop();
                        state_ref.rebuild();
                        state_ref.selected = None;
                    }
                    gdk::Key::Tab => action = Some(Action::CycleProject(1)),
                    gdk::Key::ISO_Left_Tab => action = Some(Action::CycleProject(-1)),
                    gdk::Key::Up | gdk::Key::Left => {
                        move_selection(&mut state_ref, -1);
                    }
                    gdk::Key::Down | gdk::Key::Right => {
                        move_selection(&mut state_ref, 1);
                    }
                    gdk::Key::Return | gdk::Key::KP_Enter => {
                        if let Some(folder) = state_ref.selected_folder() {
                            let intent = if control {
                                Intent::Terminal
                            } else if shift {
                                Intent::CopyPath
                            } else {
                                Intent::Open
                            };
                            action = Some(Action::Launch(folder, intent));
                        }
                    }
                    key if !control && !alt => {
                        if let Some(ch) = key.to_unicode().filter(|ch| !ch.is_control()) {
                            if state_ref.query.is_empty() && ch.is_ascii_digit() && ch != '0' {
                                let position = ch.to_digit(10).unwrap() as usize - 1;
                                if let Some(folder) = state_ref.folder_at(position) {
                                    let intent = if shift {
                                        Intent::CopyPath
                                    } else {
                                        Intent::Open
                                    };
                                    action = Some(Action::Launch(folder, intent));
                                }
                            } else if state_ref.query.len() < 64 {
                                state_ref.query.push(ch);
                                state_ref.rebuild();
                                state_ref.selected = None;
                            }
                        }
                    }
                    _ => handled = false,
                }

                drop(state_ref);
                area.queue_draw();
                if let Some(action) = action {
                    emit(&actions, action);
                }
                if handled {
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
        }
        area.add_controller(keys);

        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        {
            let state = state.clone();
            let area = area.clone();
            scroll.connect_scroll(move |_scroll, _dx, dy| {
                if dy != 0.0 {
                    move_selection(&mut state.borrow_mut(), if dy > 0.0 { 1 } else { -1 });
                    area.queue_draw();
                }
                glib::Propagation::Stop
            });
        }
        area.add_controller(scroll);

        Self {
            area,
            state,
            actions,
        }
    }

    pub fn set_action_handler(&self, handler: Rc<dyn Fn(Action)>) {
        *self.actions.borrow_mut() = Some(handler);
    }

    /// Replaces the wheel contents. Typing state is kept so a config change
    /// (e.g. removing a folder from the context menu) does not clear the query.
    pub fn set_data(&self, data: WheelData) {
        let mut state = self.state.borrow_mut();
        state.data = data;
        state.rebuild();
        self.area.queue_draw();
    }

    pub fn reset(&self) {
        let mut state = self.state.borrow_mut();
        state.query.clear();
        state.selected = None;
        state.hover = None;
        state.rebuild();
        self.area.queue_draw();
    }

    pub fn focus(&self) {
        self.area.grab_focus();
    }

    pub fn update_theme(&self) {
        self.area.queue_draw();
    }
}

fn move_selection(state: &mut State, offset: isize) {
    if state.filtered.is_empty() {
        return;
    }
    let len = state.filtered.len() as isize;
    let current = state.selected.or(state.hover).unwrap_or(0) as isize;
    state.selected = Some((current + offset).rem_euclid(len) as usize);
}

fn emit(actions: &ActionSlot, action: Action) {
    if let Some(handler) = actions.borrow().as_ref() {
        handler(action);
    }
}

fn show_context_menu(area: &gtk::DrawingArea, actions: &ActionSlot, x: f64, y: f64, folder: Uuid) {
    let popover = gtk::Popover::new();
    popover.set_parent(area);
    popover.set_has_arrow(false);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    let items: [(&str, Action); 4] = [
        ("Open", Action::Launch(folder, Intent::Open)),
        ("Open in Terminal", Action::Launch(folder, Intent::Terminal)),
        ("Copy Path", Action::Launch(folder, Intent::CopyPath)),
        ("Manage Folders…", Action::Manage),
    ];

    for (label, action) in items {
        let button = gtk::Button::with_label(label);
        button.add_css_class("flat");
        button.set_halign(gtk::Align::Fill);
        let actions = actions.clone();
        let weak = popover.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popover) = weak.upgrade() {
                popover.popdown();
            }
            emit(&actions, action);
        });
        menu.append(&button);
    }

    menu.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let remove = gtk::Button::with_label("Remove from wheel");
    remove.add_css_class("flat");
    remove.set_halign(gtk::Align::Fill);
    {
        let actions = actions.clone();
        let weak = popover.downgrade();
        remove.connect_clicked(move |_| {
            if let Some(popover) = weak.upgrade() {
                popover.popdown();
            }
            emit(&actions, Action::Remove(folder));
        });
    }
    menu.append(&remove);

    popover.set_child(Some(&menu));
    popover.connect_closed(|popover| popover.unparent());
    popover.popup();
}

fn draw(cr: &gtk::cairo::Context, width: i32, height: i32, state: &State) {
    let geo = Geometry::new(width as f64, height as f64);
    let palette = palette(theme::is_dark());

    cr.set_antialias(gtk::cairo::Antialias::Best);

    // Soft halo so the wheel separates from the panel surface.
    let halo = gtk::cairo::RadialGradient::new(
        geo.cx,
        geo.cy + 4.0,
        geo.outer * 0.9,
        geo.cx,
        geo.cy + 4.0,
        geo.outer * 1.18,
    );
    halo.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, palette.shadow_alpha);
    halo.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    let _ = cr.set_source(&halo);
    cr.paint().ok();

    if state.filtered.is_empty() {
        draw_center(cr, &geo, state, &palette, 0);
        draw_project_dots(cr, &geo, state, &palette);
        return;
    }

    let count = state.filtered.len();
    if count == 1 {
        draw_single(cr, &geo, state, &palette);
        draw_project_dots(cr, &geo, state, &palette);
        return;
    }
    let step = TAU / count as f64;
    let gap = if count <= 1 {
        0.0
    } else {
        (step * 0.055).clamp(0.012, 0.04)
    };
    let (pr, pg, pb) = theme::parse_hex(&state.data.project_color);

    for (position, folder_index) in state.filtered.iter().enumerate() {
        let folder = &state.data.folders[*folder_index];
        let start = -FRAC_PI_2 + position as f64 * step + gap / 2.0;
        let end = -FRAC_PI_2 + (position + 1) as f64 * step - gap / 2.0;
        let active = state.hover == Some(position) || state.selected == Some(position);
        let (fr, fg, fb) = folder
            .color
            .as_deref()
            .map(theme::parse_hex)
            .unwrap_or((pr, pg, pb));

        let base = mix(
            palette.ring_inner,
            (fr, fg, fb),
            if active { 0.62 } else { 0.28 },
        );
        wedge(cr, geo.cx, geo.cy, geo.inner, geo.outer, start, end);
        cr.set_source_rgb(base.0, base.1, base.2);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(fr, fg, fb, if active { 0.98 } else { 0.48 });
        cr.set_line_width(if active { 1.8 } else { 1.0 });
        let _ = cr.stroke();

        if folder.availability != Availability::Available {
            let _ = cr.save();
            cr.arc(geo.cx, geo.cy, geo.outer - 3.0, start, end);
            cr.set_source_rgba(0.90, 0.28, 0.30, 0.90);
            cr.set_line_width(3.0);
            let _ = cr.stroke();
            let _ = cr.restore();
        }

        let mid = (start + end) / 2.0;
        let label_radius = (geo.inner + geo.outer) / 2.0 + 4.0;
        // A single folder spans 2π, so the normal chord formula collapses to
        // zero and would ellipsize every label to a few characters.
        let max_width = if count <= 2 {
            (geo.inner * 1.55).clamp(140.0, 240.0)
        } else {
            (2.0 * label_radius * (step / 2.0).sin() * 0.86).clamp(64.0, 190.0)
        };
        let font_size = if count <= 6 {
            14.0
        } else if count <= 12 {
            12.5
        } else {
            11.0
        };

        let text_color = (
            palette.text.0,
            palette.text.1,
            palette.text.2,
            if active { 1.0 } else { 0.92 },
        );
        text(
            cr,
            geo.cx + mid.cos() * label_radius,
            geo.cy + mid.sin() * label_radius,
            &folder.label,
            font_size,
            text_color,
            max_width,
            true,
        );
    }

    draw_center(cr, &geo, state, &palette, count);
    draw_project_dots(cr, &geo, state, &palette);
}

fn draw_single(cr: &gtk::cairo::Context, geo: &Geometry, state: &State, palette: &Palette) {
    let Some(folder_index) = state.filtered.first() else {
        return;
    };
    let folder = &state.data.folders[*folder_index];
    let (pr, pg, pb) = theme::parse_hex(&state.data.project_color);
    let (fr, fg, fb) = folder
        .color
        .as_deref()
        .map(theme::parse_hex)
        .unwrap_or((pr, pg, pb));
    let active = state.hover == Some(0) || state.selected == Some(0);

    let strong = (palette.text.0, palette.text.1, palette.text.2, 0.98);
    let muted = (palette.muted.0, palette.muted.1, palette.muted.2, 0.82);
    let heading_width = geo.single.2;

    if state.query.is_empty() {
        text(
            cr,
            geo.cx,
            geo.cy - 66.0,
            &state.data.project_name,
            18.0,
            strong,
            heading_width,
            true,
        );
        let mut hint = format!(
            "{} folder{}",
            state.data.folders.len(),
            if state.data.folders.len() == 1 {
                ""
            } else {
                "s"
            }
        );
        if state.data.project_total > 1 {
            hint.push_str("  ·  Tab switches project");
        }
        text(
            cr,
            geo.cx,
            geo.cy - 42.0,
            &hint,
            10.5,
            muted,
            heading_width,
            false,
        );
    } else {
        text(
            cr,
            geo.cx,
            geo.cy - 66.0,
            &state.query,
            17.0,
            strong,
            heading_width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy - 42.0,
            "1 match",
            10.5,
            muted,
            heading_width,
            false,
        );
    }

    let (x, y, width, height) = geo.single;
    let base = mix(
        palette.ring_inner,
        (fr, fg, fb),
        if active { 0.68 } else { 0.44 },
    );
    rounded_rect(cr, x, y, width, height, 18.0);
    cr.set_source_rgb(base.0, base.1, base.2);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(fr, fg, fb, if active { 0.98 } else { 0.62 });
    cr.set_line_width(if active { 2.0 } else { 1.0 });
    let _ = cr.stroke();

    let badge_x = x + 38.0;
    let badge_y = y + height / 2.0;
    circle(cr, badge_x, badge_y, 13.0);
    cr.set_source_rgba(palette.center.0, palette.center.1, palette.center.2, 0.98);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(palette.rim.0, palette.rim.1, palette.rim.2, 0.18);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
    text(
        cr,
        badge_x,
        badge_y,
        "1",
        11.0,
        (palette.muted.0, palette.muted.1, palette.muted.2, 0.9),
        22.0,
        true,
    );

    text(
        cr,
        geo.cx + 12.0,
        badge_y,
        &folder.label,
        16.0,
        (palette.text.0, palette.text.1, palette.text.2, 1.0),
        width - 102.0,
        true,
    );

    if folder.availability != Availability::Available {
        let reason = if folder.availability == Availability::Missing {
            "Folder not found on disk"
        } else {
            "Drive is not connected"
        };
        text(
            cr,
            geo.cx,
            y + height + 26.0,
            reason,
            10.5,
            (0.90, 0.28, 0.30, 0.95),
            width,
            false,
        );
    }
}

fn draw_center(
    cr: &gtk::cairo::Context,
    geo: &Geometry,
    state: &State,
    palette: &Palette,
    count: usize,
) {
    let (pr, pg, pb) = theme::parse_hex(&state.data.project_color);
    circle(cr, geo.cx, geo.cy, geo.inner - 4.0);
    let center = mix(palette.center, (pr, pg, pb), 0.16);
    cr.set_source_rgb(center.0, center.1, center.2);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(palette.rim.0, palette.rim.1, palette.rim.2, 0.16);
    cr.set_line_width(1.2);
    let _ = cr.stroke();

    let inner_width = (geo.inner - 12.0).max(90.0) * 1.6;
    let muted = (palette.muted.0, palette.muted.1, palette.muted.2, 0.82);
    let strong = (palette.text.0, palette.text.1, palette.text.2, 0.98);
    if !state.query.is_empty() {
        text(
            cr,
            geo.cx,
            geo.cy - 18.0,
            &state.query,
            17.0,
            strong,
            inner_width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy + 8.0,
            &format!("{count} match{}", if count == 1 { "" } else { "es" }),
            10.5,
            muted,
            inner_width,
            false,
        );
    } else if state.data.folders.is_empty() {
        text(
            cr,
            geo.cx,
            geo.cy - 10.0,
            "No folders",
            13.0,
            strong,
            inner_width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy + 12.0,
            "Click to manage",
            10.0,
            muted,
            inner_width,
            false,
        );
    } else {
        text(
            cr,
            geo.cx,
            geo.cy - 16.0,
            &state.data.project_name,
            15.0,
            strong,
            inner_width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy + 6.0,
            &format!(
                "{} folder{}",
                state.data.folders.len(),
                if state.data.folders.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            10.5,
            muted,
            inner_width,
            false,
        );
        if state.data.project_total > 1 {
            text(
                cr,
                geo.cx,
                geo.cy + 26.0,
                "Tab to switch",
                9.5,
                (palette.muted.0, palette.muted.1, palette.muted.2, 0.62),
                inner_width,
                false,
            );
        }
    }
}

fn draw_project_dots(cr: &gtk::cairo::Context, geo: &Geometry, state: &State, palette: &Palette) {
    let total = state.data.project_total;
    if total < 2 {
        return;
    }
    let spacing = 14.0;
    let start_x = geo.cx - (total.saturating_sub(1) as f64 * spacing) / 2.0;
    let y = (geo.cy - geo.outer - 20.0).max(18.0);
    let project_color = theme::parse_hex(&state.data.project_color);
    for index in 0..total.min(12) {
        let x = start_x + index as f64 * spacing;
        circle(
            cr,
            x,
            y,
            if index == state.data.project_index {
                3.5
            } else {
                2.2
            },
        );
        if index == state.data.project_index {
            cr.set_source_rgb(project_color.0, project_color.1, project_color.2);
        } else {
            cr.set_source_rgba(palette.muted.0, palette.muted.1, palette.muted.2, 0.42);
        }
        let _ = cr.fill();
    }
}

fn mix(base: (f64, f64, f64), accent: (f64, f64, f64), amount: f64) -> (f64, f64, f64) {
    (
        base.0 + (accent.0 - base.0) * amount,
        base.1 + (accent.1 - base.1) * amount,
        base.2 + (accent.2 - base.2) * amount,
    )
}

fn rounded_rect(cr: &gtk::cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    cr.new_sub_path();
    cr.arc(x + width - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    cr.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    cr.arc(
        x + radius,
        y + height - radius,
        radius,
        FRAC_PI_2,
        std::f64::consts::PI,
    );
    cr.arc(
        x + radius,
        y + radius,
        radius,
        std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    );
    cr.close_path();
}

fn circle(cr: &gtk::cairo::Context, x: f64, y: f64, radius: f64) {
    cr.new_path();
    cr.arc(x, y, radius, 0.0, TAU);
    cr.close_path();
}

fn wedge(cr: &gtk::cairo::Context, cx: f64, cy: f64, inner: f64, outer: f64, start: f64, end: f64) {
    cr.new_path();
    cr.arc(cx, cy, outer, start, end);
    cr.arc_negative(cx, cy, inner, end, start);
    cr.close_path();
}

#[allow(clippy::too_many_arguments)]
fn text(
    cr: &gtk::cairo::Context,
    x: f64,
    y: f64,
    value: &str,
    size: f64,
    color: (f64, f64, f64, f64),
    max_width: f64,
    bold: bool,
) {
    let layout = pangocairo::create_layout(cr);
    let mut description = pango::FontDescription::from_string(&format!("Sans {size}"));
    if bold {
        description.set_weight(pango::Weight::Semibold);
    }
    layout.set_font_description(Some(&description));
    layout.set_text(value);
    layout.set_width((max_width.max(1.0) * pango::SCALE as f64) as i32);
    layout.set_ellipsize(pango::EllipsizeMode::End);
    layout.set_alignment(pango::Alignment::Center);
    let (text_width, text_height) = layout.pixel_size();
    cr.set_source_rgba(color.0, color.1, color.2, color.3);
    cr.move_to(x - text_width as f64 / 2.0, y - text_height as f64 / 2.0);
    pangocairo::show_layout(cr, &layout);
}
