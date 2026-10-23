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
pub struct ProjectTab {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub folder_count: usize,
}

#[derive(Debug, Clone)]
pub struct WheelData {
    pub active_project: Uuid,
    pub project_name: String,
    pub project_color: String,
    pub shortcut: String,
    pub folders: Vec<WheelFolder>,
    pub projects: Vec<ProjectTab>,
}

impl Default for WheelData {
    fn default() -> Self {
        Self {
            active_project: Uuid::nil(),
            project_name: "My Project".into(),
            project_color: "#3b82f6".into(),
            shortcut: "CTRL+ALT+P".into(),
            folders: Vec::new(),
            projects: Vec::new(),
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
    focused: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    Sector(usize),
    Center,
    Outside,
}

fn hit_test(geo: &Geometry, state: &State, x: f64, y: f64) -> Hit {
    let dx = x - geo.cx;
    let dy = y - geo.cy;
    let distance = (dx * dx + dy * dy).sqrt();
    if state.filtered.len() == 1 {
        return if distance <= geo.outer {
            Hit::Sector(0)
        } else {
            Hit::Outside
        };
    }

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
        {
            let state = state.clone();
            let area = area.clone();
            motion.connect_leave(move |_| {
                let mut state = state.borrow_mut();
                if state.hover.take().is_some() {
                    drop(state);
                    area.queue_draw();
                }
                area.set_cursor_from_name(Some("default"));
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
                            let intent = if button == 2 {
                                Intent::CopyPath
                            } else if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
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
                        let had_query = !state_ref.query.is_empty();
                        if had_query {
                            state_ref.query.clear();
                            state_ref.rebuild();
                            state_ref.selected = None;
                            drop(state_ref);
                            wheel.queue_draw();
                        } else if state_ref.filtered.len() == 1 {
                            let folder = state_ref.folder_at(0);
                            drop(state_ref);
                            if let Some(folder) = folder {
                                emit(&actions, Action::Launch(folder, Intent::Open));
                            }
                        } else {
                            drop(state_ref);
                            emit(&actions, Action::Manage);
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

        let focus = gtk::EventControllerFocus::new();
        {
            let state = state.clone();
            let area = area.clone();
            focus.connect_enter(move |_| {
                state.borrow_mut().focused = true;
                area.queue_draw();
            });
        }
        {
            let state = state.clone();
            let area = area.clone();
            focus.connect_leave(move |_| {
                state.borrow_mut().focused = false;
                area.queue_draw();
            });
        }
        area.add_controller(focus);

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
        state.selected = None;
        state.hover = None;
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
    let halo = gtk::cairo::RadialGradient::new(
        geo.cx,
        geo.cy + 6.0,
        geo.outer * 0.85,
        geo.cx,
        geo.cy + 6.0,
        geo.outer * 1.18,
    );
    halo.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, palette.shadow_alpha);
    halo.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    let _ = cr.set_source(&halo);
    cr.paint().ok();

    if state.filtered.is_empty() {
        draw_empty(cr, &geo, state, &palette);
        draw_focus(cr, &geo, state);
        return;
    }
    if state.filtered.len() == 1 {
        draw_single(cr, &geo, state, &palette);
        draw_focus(cr, &geo, state);
        return;
    }

    let count = state.filtered.len();
    let step = TAU / count as f64;
    let gap = (step * 0.075).clamp(0.016, 0.055);
    let (pr, pg, pb) = theme::parse_hex(&state.data.project_color);

    for (position, folder_index) in state.filtered.iter().enumerate() {
        let folder = &state.data.folders[*folder_index];
        let start = -FRAC_PI_2 - step / 2.0 + gap / 2.0 + position as f64 * step;
        let end = start + step - gap;
        let active = state.hover == Some(position) || state.selected == Some(position);
        let (fr, fg, fb) = folder
            .color
            .as_deref()
            .map(theme::parse_hex)
            .unwrap_or((pr, pg, pb));
        let tint = if active { 0.46 } else { 0.10 };
        let inner = mix(palette.ring_inner, (fr, fg, fb), tint * 0.55);
        let outer = mix(palette.ring_outer, (fr, fg, fb), tint);

        let gradient =
            gtk::cairo::RadialGradient::new(geo.cx, geo.cy, geo.inner, geo.cx, geo.cy, geo.outer);
        gradient.add_color_stop_rgb(0.0, inner.0, inner.1, inner.2);
        gradient.add_color_stop_rgb(1.0, outer.0, outer.1, outer.2);
        wedge(cr, geo.cx, geo.cy, geo.inner, geo.outer, start, end);
        let _ = cr.set_source(&gradient);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(fr, fg, fb, if active { 0.95 } else { 0.34 });
        cr.set_line_width(if active { 2.0 } else { 1.0 });
        let _ = cr.stroke();

        if folder.availability != Availability::Available {
            cr.new_path();
            cr.arc(geo.cx, geo.cy, geo.outer - 3.0, start, end);
            cr.set_source_rgba(0.90, 0.28, 0.30, 0.92);
            cr.set_line_width(3.0);
            let _ = cr.stroke();
        }

        let mid = (start + end) / 2.0;
        let label_radius = (geo.inner + geo.outer) / 2.0;
        let mut max_width = if count <= 2 {
            (geo.inner * 1.55).clamp(120.0, 210.0)
        } else {
            (2.0 * label_radius * (step / 2.0).sin() * 0.84).clamp(58.0, 170.0)
        };
        let side_label = mid.cos().abs() > 0.4;
        if side_label {
            let inward = (label_radius - geo.inner - 4.0) * 2.0;
            max_width = max_width.min(inward.max(48.0));
        }
        let base_size = if count <= 8 {
            12.5
        } else if count <= 13 {
            11.5
        } else {
            10.5
        };
        let font_size = if side_label {
            base_size - 1.5
        } else {
            base_size
        };
        let color = if folder.availability == Availability::Available {
            palette.text
        } else {
            palette.muted
        };
        text(
            cr,
            geo.cx + mid.cos() * label_radius,
            geo.cy + mid.sin() * label_radius,
            &folder.label,
            font_size,
            (color.0, color.1, color.2, if active { 1.0 } else { 0.92 }),
            max_width,
            true,
        );
    }

    draw_center(cr, &geo, state, &palette, count);
    draw_caption(cr, &geo, state, &palette, count);
    draw_focus(cr, &geo, state);
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
    let tint = if active { 0.46 } else { 0.18 };
    let inner = mix(palette.ring_inner, (fr, fg, fb), tint * 0.55);
    let outer = mix(palette.ring_outer, (fr, fg, fb), tint);

    let gradient =
        gtk::cairo::RadialGradient::new(geo.cx, geo.cy, geo.inner, geo.cx, geo.cy, geo.outer);
    gradient.add_color_stop_rgb(0.0, inner.0, inner.1, inner.2);
    gradient.add_color_stop_rgb(1.0, outer.0, outer.1, outer.2);
    circle(cr, geo.cx, geo.cy, geo.outer);
    let _ = cr.set_source(&gradient);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(fr, fg, fb, if active { 0.98 } else { 0.52 });
    cr.set_line_width(if active { 2.2 } else { 1.2 });
    let _ = cr.stroke();

    if folder.availability != Availability::Available {
        cr.new_path();
        cr.arc(geo.cx, geo.cy, geo.outer - 4.0, 0.0, TAU);
        cr.set_source_rgba(0.90, 0.28, 0.30, 0.92);
        cr.set_line_width(3.0);
        let _ = cr.stroke();
    }

    circle(cr, geo.cx, geo.cy, geo.inner * 0.86);
    cr.set_source_rgba(palette.center.0, palette.center.1, palette.center.2, 0.98);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(palette.rim.0, palette.rim.1, palette.rim.2, 0.12);
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    let color = if folder.availability == Availability::Available {
        palette.text
    } else {
        palette.muted
    };
    text(
        cr,
        geo.cx,
        geo.cy - 7.0,
        &folder.label,
        15.0,
        (color.0, color.1, color.2, 0.98),
        geo.inner * 1.72,
        true,
    );
    text(
        cr,
        geo.cx,
        geo.cy + 15.0,
        if state.query.is_empty() {
            "Open"
        } else {
            "1 match"
        },
        10.0,
        (palette.muted.0, palette.muted.1, palette.muted.2, 0.9),
        geo.inner * 1.5,
        false,
    );
    draw_caption(cr, geo, state, palette, 1);
}

fn draw_empty(cr: &gtk::cairo::Context, geo: &Geometry, state: &State, palette: &Palette) {
    let muted = (palette.muted.0, palette.muted.1, palette.muted.2, 0.9);
    cr.set_dash(&[6.0, 7.0], 0.0);
    cr.set_line_width(2.0);
    cr.set_source_rgba(muted.0, muted.1, muted.2, 0.42);
    cr.new_path();
    cr.arc(geo.cx, geo.cy, geo.outer, 0.0, TAU);
    let _ = cr.stroke();
    cr.set_dash(&[], 0.0);

    circle(cr, geo.cx, geo.cy, geo.inner * 0.86);
    cr.set_source_rgba(palette.center.0, palette.center.1, palette.center.2, 0.98);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(palette.rim.0, palette.rim.1, palette.rim.2, 0.10);
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    let width = geo.inner * 1.7;
    if state.query.is_empty() {
        text(
            cr,
            geo.cx,
            geo.cy - 7.0,
            "No folders",
            14.0,
            (palette.text.0, palette.text.1, palette.text.2, 0.98),
            width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy + 14.0,
            "Click to manage",
            10.0,
            muted,
            width,
            false,
        );
    } else {
        text(
            cr,
            geo.cx,
            geo.cy - 7.0,
            &state.query,
            15.0,
            (palette.text.0, palette.text.1, palette.text.2, 0.98),
            width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy + 14.0,
            "No matches",
            10.0,
            muted,
            width,
            false,
        );
    }
    draw_caption(cr, geo, state, palette, 0);
}

fn draw_center(
    cr: &gtk::cairo::Context,
    geo: &Geometry,
    state: &State,
    palette: &Palette,
    count: usize,
) {
    circle(cr, geo.cx, geo.cy, geo.inner * 0.86);
    cr.set_source_rgba(palette.center.0, palette.center.1, palette.center.2, 0.98);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(palette.rim.0, palette.rim.1, palette.rim.2, 0.10);
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    let width = geo.inner * 1.5;
    let muted = (palette.muted.0, palette.muted.1, palette.muted.2, 0.9);
    let strong = (palette.text.0, palette.text.1, palette.text.2, 0.98);
    if state.query.is_empty() {
        text(
            cr,
            geo.cx,
            geo.cy - 6.0,
            "Manage",
            11.5,
            strong,
            width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy + 12.0,
            "folders",
            9.5,
            muted,
            width,
            false,
        );
    } else {
        text(
            cr,
            geo.cx,
            geo.cy - 7.0,
            &state.query,
            14.0,
            strong,
            width,
            true,
        );
        text(
            cr,
            geo.cx,
            geo.cy + 12.0,
            &format!("{count} match{}", if count == 1 { "" } else { "es" }),
            9.5,
            muted,
            width,
            false,
        );
    }
}

fn draw_caption(
    cr: &gtk::cairo::Context,
    geo: &Geometry,
    state: &State,
    palette: &Palette,
    count: usize,
) {
    let muted = (palette.muted.0, palette.muted.1, palette.muted.2, 0.9);
    let error = (0.90, 0.28, 0.30, 0.98);
    let hovered = state
        .hover
        .or(state.selected)
        .and_then(|position| state.filtered.get(position))
        .and_then(|index| state.data.folders.get(*index));

    let (label, color) = if !state.query.is_empty() {
        (
            format!(
                "{count} match{} · click center clears",
                if count == 1 { "" } else { "es" }
            ),
            muted,
        )
    } else if let Some(folder) = hovered {
        match folder.availability {
            Availability::Available => (format!("Open {}", folder.label), muted),
            Availability::Missing => ("Folder is not on disk".to_string(), error),
            Availability::Offline => ("Drive is offline".to_string(), error),
        }
    } else if state.data.folders.is_empty() {
        ("Add a folder to get started".to_string(), muted)
    } else if count == 1 {
        (
            "Click to open · right-click center to manage".to_string(),
            muted,
        )
    } else {
        (
            "Choose a folder · click center to manage".to_string(),
            muted,
        )
    };

    text(
        cr,
        geo.cx,
        geo.caption_y,
        &label,
        11.0,
        color,
        geo.outer * 2.8,
        false,
    );
}

fn draw_focus(cr: &gtk::cairo::Context, geo: &Geometry, state: &State) {
    if !state.focused {
        return;
    }
    cr.new_path();
    cr.arc(geo.cx, geo.cy, geo.outer + 7.0, 0.0, TAU);
    cr.set_source_rgba(1.0, 0.353, 0.0, 0.9);
    cr.set_line_width(2.0);
    let _ = cr.stroke();
}

fn mix(base: (f64, f64, f64), accent: (f64, f64, f64), amount: f64) -> (f64, f64, f64) {
    (
        base.0 + (accent.0 - base.0) * amount,
        base.1 + (accent.1 - base.1) * amount,
        base.2 + (accent.2 - base.2) * amount,
    )
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
