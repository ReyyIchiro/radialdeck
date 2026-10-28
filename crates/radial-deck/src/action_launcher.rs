//! Full-screen overlay that hosts the radial action wheel.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::action_wheel::{ActionWheel, WheelEvent, WheelTarget};
use crate::theme;
use deck_core::tools::{Field, ToolId};
use deck_core::Category;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelMode {
    Convert,
    Tools,
    Actions,
}

impl WheelMode {
    /// Primary modifier shown in the UI. KDE Plasma reserves bare Super for
    /// the application launcher, so Tools advertises the Alt alias instead.
    pub fn key_label(self) -> &'static str {
        match self {
            Self::Convert => "Shift",
            Self::Tools => "Alt",
            Self::Actions => "Ctrl",
        }
    }

    pub fn caption(self) -> &'static str {
        match self {
            Self::Convert => "Convert formats",
            Self::Tools => "Advanced tools",
            Self::Actions => "File actions",
        }
    }

    pub fn mode_label(self) -> &'static str {
        match self {
            Self::Convert => "Convert",
            Self::Tools => "Tools",
            Self::Actions => "Actions",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActionRequest {
    Category(Category),
    Mode(WheelMode),
    /// A modifier-key switch (Shift/Ctrl/Super) while the wheel is open.
    /// The controller honours the live-modifier preference before acting.
    LiveMode(WheelMode),
    Run(String),
    Unavailable(String),
    Dismiss,
    Dropped(Vec<PathBuf>),
    ToolRun {
        id: ToolId,
        values: BTreeMap<String, String>,
    },
    ToolCancel,
}

type RequestHandler = Rc<dyn Fn(ActionRequest)>;

#[derive(Debug, Clone)]
pub struct FormView {
    pub tool: ToolId,
    pub title: String,
    pub fields: Vec<Field>,
}

#[derive(Clone)]
struct FormField {
    key: String,
    widget: gtk::Widget,
    choices: Vec<String>,
}

pub struct ActionView {
    pub category: Category,
    pub mode: WheelMode,
    pub title: String,
    pub source_label: String,
    pub targets: Vec<WheelTarget>,
    pub shortcut: String,
    pub global_shortcut: String,
    pub status: Option<String>,
    pub empty_hint: String,
    pub form: Option<FormView>,
}

pub struct ActionLauncher {
    window: gtk::ApplicationWindow,
    panel: gtk::Box,
    tabs: Vec<(Category, gtk::Button)>,
    modes: Vec<(WheelMode, gtk::Button)>,
    title: gtk::Label,
    shortcut: gtk::Label,
    caption: gtk::Label,
    wheel: Rc<ActionWheel>,
    form: gtk::Box,
    fields: RefCell<Vec<FormField>>,
    handler: RequestHandler,
    status: gtk::Label,
    footer: gtk::Label,
    form_open: Rc<Cell<bool>>,
    visible: Rc<Cell<bool>>,
    destroyed: Rc<Cell<bool>>,
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

        let layer_shell = crate::surface::layer_shell_available();
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

        let modes_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        modes_box.add_css_class("rd-modes");
        modes_box.set_halign(gtk::Align::Center);
        modes_box.set_margin_bottom(8);
        let mut mode_buttons = Vec::new();
        for mode in [WheelMode::Convert, WheelMode::Tools, WheelMode::Actions] {
            let button =
                gtk::Button::with_label(&format!("{}  {}", mode.key_label(), mode.mode_label()));
            button.add_css_class("rd-mode");
            let alias = match mode {
                WheelMode::Tools => " (or Super)",
                _ => "",
            };
            button.set_tooltip_text(Some(&format!(
                "{}{} while the wheel is open, or press F{}",
                mode.key_label(),
                alias,
                match mode {
                    WheelMode::Convert => 1,
                    WheelMode::Tools => 2,
                    WheelMode::Actions => 3,
                }
            )));
            let handler = handler.clone();
            button.connect_clicked(move |_| handler(ActionRequest::Mode(mode)));
            modes_box.append(&button);
            mode_buttons.push((mode, button));
        }
        panel.append(&modes_box);

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

        let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
        form.add_css_class("rd-form");
        form.set_halign(gtk::Align::Center);
        form.set_valign(gtk::Align::Start);
        form.set_size_request(720, 360);
        form.set_margin_top(8);
        form.set_visible(false);
        panel.append(&form);

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

        // Dismissal is owned by the controller. Do not hide the window or the
        // root widget directly here: a hidden-but-mapped overlay makes the
        // next global-shortcut press toggle stale state instead of opening.
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

        let form_open = Rc::new(Cell::new(false));
        let keys = gtk::EventControllerKey::new();
        {
            let handler = handler.clone();
            let form_open = form_open.clone();
            keys.connect_key_pressed(move |_keys, keyval, _keycode, _modifiers| {
                // A form owns the keyboard: Ctrl/Shift/Super are modifier keys
                // users press while editing, so mode switching waits until the
                // form closes. Escape still leaves the form.
                if form_open.get() {
                    if keyval == gdk::Key::Escape {
                        handler(ActionRequest::ToolCancel);
                        return glib::Propagation::Stop;
                    }
                    return glib::Propagation::Proceed;
                }
                let mode = match keyval {
                    gdk::Key::Shift_L | gdk::Key::Shift_R | gdk::Key::F1 => {
                        Some(WheelMode::Convert)
                    }
                    // Alt is the reliable alias for Tools: Plasma grabs bare
                    // Super for its own launcher before the overlay sees it.
                    gdk::Key::Alt_L
                    | gdk::Key::Alt_R
                    | gdk::Key::Super_L
                    | gdk::Key::Super_R
                    | gdk::Key::F2 => Some(WheelMode::Tools),
                    gdk::Key::Control_L | gdk::Key::Control_R | gdk::Key::F3 => {
                        Some(WheelMode::Actions)
                    }
                    _ => None,
                };
                if let Some(mode) = mode {
                    handler(ActionRequest::LiveMode(mode));
                    glib::Propagation::Stop
                } else if keyval == gdk::Key::Escape {
                    handler(ActionRequest::Dismiss);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
        }
        window.add_controller(keys);

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
            tabs: tab_buttons,
            modes: mode_buttons,
            title,
            shortcut,
            caption,
            wheel,
            form,
            form_open,
            fields: RefCell::new(Vec::new()),
            handler: handler.clone(),
            status,
            footer,
            visible,
            destroyed,
        }
    }

    pub fn render(&self, view: ActionView) {
        self.title.set_label(&view.title);
        self.shortcut.set_label(&view.shortcut);
        self.caption.set_label(view.mode.caption());
        for (mode, button) in &self.modes {
            if *mode == view.mode {
                button.add_css_class("active");
            } else {
                button.remove_css_class("active");
            }
        }
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

        self.footer.set_label(&format!(
            "{}  ·  {}  ·  Esc closes",
            view.global_shortcut,
            match view.mode {
                WheelMode::Convert => "Drop files, then choose a format",
                WheelMode::Tools => "Press Enter to run or fill the form",
                WheelMode::Actions => "Choose what happens to the selected files",
            }
        ));

        if let Some(form) = view.form {
            self.render_form(form);
        } else {
            self.clear_form();
            self.wheel.area.set_visible(true);
            self.wheel.set_state(crate::action_wheel::WheelState {
                targets: view.targets,
                source_label: view.source_label,
                empty_hint: view.empty_hint.clone(),
                caption: view.empty_hint,
                hover_prefix: match view.mode {
                    WheelMode::Convert => "Convert to",
                    WheelMode::Tools => "Run",
                    WheelMode::Actions => "",
                }
                .to_owned(),
            });
            self.wheel.set_drag_hot(false);
            self.wheel.focus();
        }
        self.apply_theme();
    }

    fn clear_form(&self) {
        while let Some(child) = self.form.first_child() {
            self.form.remove(&child);
        }
        self.fields.borrow_mut().clear();
        self.form_open.set(false);
        self.form.set_visible(false);
    }

    fn render_form(&self, form: FormView) {
        self.clear_form();
        self.wheel.area.set_visible(false);

        let heading = gtk::Label::new(Some(&form.title));
        heading.add_css_class("rd-form-title");
        heading.set_xalign(0.0);
        self.form.append(&heading);

        let hint = gtk::Label::new(Some(
            "Defaults are prefilled. Change a value, then press Run.",
        ));
        hint.add_css_class("rd-form-hint");
        hint.set_xalign(0.0);
        self.form.append(&hint);

        for field in form.fields {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
            row.add_css_class("rd-form-row");
            let label = gtk::Label::new(Some(&field.label));
            label.add_css_class("rd-form-label");
            label.set_xalign(0.0);
            label.set_size_request(190, -1);
            row.append(&label);

            if field.choices.is_empty() {
                let entry = gtk::Entry::new();
                entry.set_text(&field.value);
                entry.set_hexpand(true);
                if !field.hint.is_empty() {
                    entry.set_placeholder_text(Some(&field.hint));
                    entry.set_tooltip_text(Some(&field.hint));
                }
                let handler = self.handler.clone();
                let fields = self.fields.clone();
                let tool = form.tool;
                entry.connect_activate(move |_| {
                    handler(ActionRequest::ToolRun {
                        id: tool,
                        values: collect_values(&fields),
                    });
                });
                row.append(&entry);
                self.fields.borrow_mut().push(FormField {
                    key: field.key,
                    widget: entry.upcast(),
                    choices: Vec::new(),
                });
            } else {
                let choices = field.choices;
                let labels: Vec<&str> = choices.iter().map(String::as_str).collect();
                let dropdown = gtk::DropDown::from_strings(&labels);
                let selected = choices
                    .iter()
                    .position(|choice| choice == &field.value)
                    .unwrap_or(0);
                dropdown.set_selected(selected as u32);
                dropdown.set_hexpand(true);
                row.append(&dropdown);
                self.fields.borrow_mut().push(FormField {
                    key: field.key,
                    widget: dropdown.upcast(),
                    choices,
                });
            }
            self.form.append(&row);
        }

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        actions.set_margin_top(10);
        let cancel = gtk::Button::with_label("Cancel");
        cancel.add_css_class("rd-form-button");
        let handler = self.handler.clone();
        cancel.connect_clicked(move |_| handler(ActionRequest::ToolCancel));
        actions.append(&cancel);

        let run = gtk::Button::with_label("Run");
        run.add_css_class("rd-form-run");
        let handler = self.handler.clone();
        let fields = self.fields.clone();
        let tool = form.tool;
        run.connect_clicked(move |_| {
            handler(ActionRequest::ToolRun {
                id: tool,
                values: collect_values(&fields),
            });
        });
        actions.append(&run);
        self.form.append(&actions);
        self.form_open.set(true);
        self.form.set_visible(true);

        if let Some(first) = self.fields.borrow().first() {
            first.widget.grab_focus();
        }
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
        if self.destroyed.get() {
            tracing::warn!("action overlay was destroyed; refusing to present");
            return;
        }
        self.apply_theme();
        self.visible.set(true);
        if crate::surface::layer_shell_available() {
            crate::surface::anchor_to_pointer_monitor(&self.window);
        } else {
            crate::surface::fill_monitor_for_pointer(&self.window);
        }
        self.window.set_visible(true);
        self.window.present();
        self.wheel.focus();
        let wheel = self.wheel.area.clone();
        let window = self.window.clone();
        let visible = self.visible.clone();
        glib::idle_add_local_once(move || {
            wheel.grab_focus();
        });
        // Layer-shell surfaces occasionally miss the first present after a
        // rapid hide/show cycle. Verify the map and retry once.
        let window = window.clone();
        let visible = visible.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(80), move || {
            if visible.get() && !window.is_mapped() {
                tracing::warn!("action overlay was not mapped; presenting again");
                window.present();
            }
        });
    }

    pub fn hide(&self) {
        self.visible.set(false);
        self.window.set_visible(false);
    }

    pub fn window(&self) -> &gtk::ApplicationWindow {
        &self.window
    }

    pub fn is_visible(&self) -> bool {
        self.visible.get() && !self.destroyed.get()
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }
}

fn collect_values(fields: &RefCell<Vec<FormField>>) -> BTreeMap<String, String> {
    fields
        .borrow()
        .iter()
        .map(|field| {
            let value = if field.choices.is_empty() {
                field
                    .widget
                    .downcast_ref::<gtk::Entry>()
                    .map(|entry| entry.text().to_string())
                    .unwrap_or_default()
            } else {
                let index = field
                    .widget
                    .downcast_ref::<gtk::DropDown>()
                    .map(|dropdown| dropdown.selected() as usize)
                    .unwrap_or(0);
                field.choices.get(index).cloned().unwrap_or_default()
            };
            (field.key.clone(), value)
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn find_button(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            if button.label().as_deref() == Some(label) {
                return Some(button.clone());
            }
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(found) = find_button(&current, label) {
                return Some(found);
            }
            child = current.next_sibling();
        }
        None
    }

    fn view(mode: WheelMode, form: Option<FormView>) -> ActionView {
        ActionView {
            category: Category::Images,
            mode,
            title: "Test".into(),
            source_label: "PNG".into(),
            targets: Vec::new(),
            shortcut: mode.key_label().into(),
            global_shortcut: "Ctrl+Alt+Space opens this wheel".into(),
            status: None,
            empty_hint: "Drop files".into(),
            form,
        }
    }

    #[test]
    fn form_submits_typed_values_and_mode_buttons_switch() {
        if gtk::init().is_err() {
            eprintln!("skipping GTK test: no display");
            return;
        }
        let app = gtk::Application::builder()
            .application_id("io.github.raihan.RadialDeck.Test")
            .build();
        app.register(None::<&gtk::gio::Cancellable>)
            .expect("register test application");
        let requests = Rc::new(RefCell::new(Vec::new()));
        let sink = requests.clone();
        let launcher = ActionLauncher::new(
            &app,
            Rc::new(move |request| sink.borrow_mut().push(request)),
        );

        launcher.render(view(
            WheelMode::Tools,
            Some(FormView {
                tool: ToolId::Compress,
                title: "Compress".into(),
                fields: vec![
                    Field {
                        key: "quality".into(),
                        label: "Quality".into(),
                        value: "82".into(),
                        hint: "1–100".into(),
                        choices: Vec::new(),
                    },
                    Field {
                        key: "preset".into(),
                        label: "Preset".into(),
                        value: "medium".into(),
                        hint: String::new(),
                        choices: vec!["fast".into(), "medium".into()],
                    },
                ],
            }),
        ));

        assert!(launcher.form.get_visible());
        assert!(!launcher.wheel.area.get_visible());
        assert_eq!(launcher.fields.borrow().len(), 2);
        let entry = launcher.fields.borrow()[0]
            .widget
            .downcast_ref::<gtk::Entry>()
            .expect("quality entry")
            .clone();
        entry.set_text("70");
        let dropdown = launcher.fields.borrow()[1]
            .widget
            .downcast_ref::<gtk::DropDown>()
            .expect("preset dropdown")
            .clone();
        dropdown.set_selected(0);
        find_button(launcher.form.upcast_ref(), "Run")
            .expect("run button")
            .emit_clicked();
        match requests.borrow().last() {
            Some(ActionRequest::ToolRun { id, values }) => {
                assert_eq!(*id, ToolId::Compress);
                assert_eq!(values.get("quality").map(String::as_str), Some("70"));
                assert_eq!(values.get("preset").map(String::as_str), Some("fast"));
            }
            other => panic!("unexpected request: {other:?}"),
        }

        launcher.render(view(WheelMode::Convert, None));
        assert!(!launcher.form.get_visible());
        assert!(launcher.wheel.area.get_visible());
        launcher.modes[1].1.emit_clicked();
        assert!(matches!(
            requests.borrow().last(),
            Some(ActionRequest::Mode(WheelMode::Tools))
        ));
        launcher.window.destroy();
    }
}
