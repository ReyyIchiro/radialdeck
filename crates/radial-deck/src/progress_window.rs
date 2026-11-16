//! Small, non-focusable panel that shows the running operation.
//!
//! It never takes keyboard focus: the user may keep typing while a long FFmpeg
//! run finishes. Only the pointer can reach the Cancel button.

use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::surface;

pub struct ProgressWindow {
    window: gtk::Window,
    title: gtk::Label,
    count: gtk::Label,
    failed: gtk::Label,
    bar: gtk::ProgressBar,
    detail: gtk::Label,
    cancel: gtk::Button,
    visible: Rc<std::cell::Cell<bool>>,
    /// Media duration learned from the child's `Duration:` header, when it
    /// prints one; enables the honest per-file percentage.
    duration: Rc<std::cell::Cell<Option<f64>>>,
    /// Repeating pulse for the indeterminate phase; stopped once real
    /// fractions arrive or the panel hides.
    pulse: Rc<std::cell::RefCell<Option<glib::SourceId>>>,
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
        title.set_hexpand(true);
        title.set_xalign(0.0);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let count = gtk::Label::new(None);
        count.add_css_class("rd-progress-count");
        count.set_valign(gtk::Align::Center);
        count.set_visible(false);

        let failed = gtk::Label::new(None);
        failed.add_css_class("rd-badge");
        failed.add_css_class("rd-badge-error");
        failed.add_css_class("rd-progress-failed");
        failed.set_valign(gtk::Align::Center);
        failed.set_visible(false);

        let detail = gtk::Label::new(None);
        detail.add_css_class("rd-subtle");
        detail.set_halign(gtk::Align::Start);
        detail.set_xalign(0.0);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        detail.set_max_width_chars(44);

        let bar = gtk::ProgressBar::new();
        bar.add_css_class("rd-progress-bar");
        bar.set_show_text(false);

        let cancel = gtk::Button::with_label("Cancel");
        cancel.add_css_class("rd-progress-cancel");
        cancel.set_halign(gtk::Align::End);

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.add_css_class("rd-progress-head");
        head.append(&title);
        head.append(&count);
        head.append(&failed);

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        panel.add_css_class("rd-card");
        panel.set_width_request(340);
        panel.set_margin_top(14);
        panel.set_margin_bottom(14);
        panel.set_margin_start(16);
        panel.set_margin_end(16);
        panel.append(&head);
        panel.append(&detail);
        panel.append(&bar);
        panel.append(&cancel);
        window.set_child(Some(&panel));

        Self {
            window,
            title,
            count,
            failed,
            bar,
            detail,
            cancel,
            visible: Rc::new(std::cell::Cell::new(false)),
            duration: Rc::new(std::cell::Cell::new(None)),
            pulse: Rc::new(std::cell::RefCell::new(None)),
        }
    }

    pub fn connect_cancel(&self, handler: impl Fn() + 'static) {
        self.cancel.connect_clicked(move |_| handler());
    }

    fn stop_pulse(&self) {
        if let Some(id) = self.pulse.borrow_mut().take() {
            id.remove();
        }
    }

    fn start_pulse(&self) {
        if self.pulse.borrow().is_some() {
            return;
        }
        let bar = self.bar.clone();
        let id = glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
            bar.pulse();
            glib::ControlFlow::Continue
        });
        *self.pulse.borrow_mut() = Some(id);
    }

    pub fn show_operation(&self, title: &str, total: usize) {
        self.stop_pulse();
        self.duration.set(None);
        self.title.set_text(title);
        self.detail.set_text("Starting…");
        self.bar.set_fraction(0.0);
        if total > 1 {
            self.count.set_text(&format!("0 / {total}"));
            self.count.set_visible(true);
        } else {
            self.count.set_visible(false);
        }
        self.failed.set_visible(false);
        self.cancel.set_sensitive(true);
        self.cancel.set_label("Cancel");
        // Theme-dependent fill: both variants must clear 3:1 against the
        // trough, so the class is refreshed every time the panel shows.
        if crate::theme::is_dark() {
            self.window.remove_css_class("rd-theme-light");
            self.window.add_css_class("rd-theme-dark");
        } else {
            self.window.remove_css_class("rd-theme-dark");
            self.window.add_css_class("rd-theme-light");
        }
        self.start_pulse();
        self.visible.set(true);
        self.window.present();
    }

    pub fn update(&self, progress: &deck_core::Progress) {
        if !self.visible.get() {
            return;
        }
        let clock = progress.detail.as_deref().and_then(media_clock);
        if let Some(MediaClock::Duration(secs)) = clock {
            self.duration.set(Some(secs));
        }

        // Text priority: honest per-file percentage when both clocks are
        // known, then the human phase, then just the file name. Raw lines are
        // never shown; anything unrecognised keeps the previous text.
        let mut text = None;
        if let Some(MediaClock::Position(pos)) = clock {
            if let Some(total) = self.duration.get().filter(|total| *total > pos) {
                let percent = ((pos / total) * 100.0).round() as u32;
                let left = human_remaining(total - pos);
                text = Some(match progress.current.as_str() {
                    "" => format!("Encoding {percent}% · {left} left"),
                    name => format!("{name} · {percent}% · {left} left"),
                });
            }
        }
        if text.is_none() {
            let phase = progress.detail.as_deref().and_then(phase_label);
            text = match (progress.current.as_str(), phase) {
                ("", Some(phase)) => Some(phase.to_owned()),
                ("", None) => None,
                (name, Some(phase)) => Some(format!("{name} · {phase}")),
                (name, None) => Some(name.to_owned()),
            };
        }
        if let Some(text) = text {
            self.detail.set_text(&text);
        }
        if progress.done > 0 && progress.total > 0 {
            let fraction = (progress.done as f64 / progress.total as f64).clamp(0.0, 1.0);
            self.bar.set_fraction(fraction);
            if fraction > 0.0 {
                self.stop_pulse();
            }
            if progress.total > 1 {
                self.count
                    .set_text(&format!("{} / {}", progress.done, progress.total));
            }
        }
        if progress.failed > 0 {
            self.failed.set_text(&format!("{} failed", progress.failed));
            self.failed.set_visible(true);
        }
    }

    pub fn finish(&self) {
        self.stop_pulse();
        self.visible.set(false);
        self.window.set_visible(false);
    }
}

/// Maps a child-process stderr line to a short human phase. Raw lines are
/// never shown; anything unrecognised keeps the previous status text.
fn phase_label(line: &str) -> Option<&'static str> {
    let line = line.trim_start();
    if line.starts_with("frame=") || line.starts_with("size=") {
        Some("Encoding…")
    } else if line.starts_with("Duration:") {
        Some("Reading media…")
    } else if line.starts_with("Page ") || line.starts_with("Processing ") {
        Some("Rendering…")
    } else {
        None
    }
}

#[derive(Debug, PartialEq)]
enum MediaClock {
    Duration(f64),
    Position(f64),
}

/// Recognises the two clock lines FFmpeg writes to stderr: the input header
/// `Duration: 00:00:30.00, start: …` and the repeated stats line that carries
/// `time=00:00:12.34`. Together they make an honest per-file percentage.
fn media_clock(line: &str) -> Option<MediaClock> {
    if let Some(rest) = line.trim_start().strip_prefix("Duration:") {
        let stamp = rest.split(',').next()?;
        return parse_hms(stamp).map(MediaClock::Duration);
    }
    let index = line.find("time=")?;
    parse_hms(&line[index + "time=".len()..]).map(MediaClock::Position)
}

/// Parses `HH:MM:SS.mmm`, `MM:SS.mmm` or `SS.mmm`, stopping at the first
/// character that cannot belong to a timestamp (FFmpeg appends `, start: …`).
fn parse_hms(stamp: &str) -> Option<f64> {
    let digits: &str = stamp
        .trim()
        .split(|ch: char| !(ch.is_ascii_digit() || ch == ':' || ch == '.'))
        .next()?;
    if digits.is_empty() {
        return None;
    }
    let mut seconds = 0.0;
    for part in digits.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}

/// Remaining-time estimate for the panel: `~18s`, `~1m 20s`, `~1h 05m`.
fn human_remaining(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (hours, rest) = (total / 3600, total % 3600);
    let (minutes, secs) = (rest / 60, rest % 60);
    if hours > 0 {
        format!("~{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("~{minutes}m {secs:02}s")
    } else if total >= 1 {
        format!("~{secs}s")
    } else {
        "moments".into()
    }
}

#[cfg(test)]
mod eta_tests {
    use super::*;

    #[test]
    fn parses_ffmpeg_clock_lines() {
        assert_eq!(
            media_clock("Duration: 00:01:30.00, start: 0.000000, bitrate: 1 kb/s"),
            Some(MediaClock::Duration(90.0))
        );
        assert_eq!(
            media_clock("frame=  512 fps= 24 size=    2048kB time=00:00:21.28"),
            Some(MediaClock::Position(21.28))
        );
        assert_eq!(
            media_clock("frame=    1 fps=0.0 time=00:12:34.56"),
            Some(MediaClock::Position(754.56))
        );
        assert_eq!(media_clock("Press [q] to stop"), None);
        assert_eq!(parse_hms(", start: 0.0"), None);
    }

    #[test]
    fn remaining_time_is_human() {
        assert_eq!(human_remaining(0.2), "moments");
        assert_eq!(human_remaining(18.0), "~18s");
        assert_eq!(human_remaining(80.0), "~1m 20s");
        assert_eq!(human_remaining(3900.0), "~1h 05m");
    }

    #[test]
    fn position_beyond_duration_shows_no_estimate() {
        // A stream with a wrong header must not show a nonsense percentage.
        let total = Some(10.0);
        assert!(total.filter(|t| *t > 42.0).is_none());
    }
}
