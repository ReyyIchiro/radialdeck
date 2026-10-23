use gtk::{gdk, CssProvider};

use deck_core::Theme;

const CSS: &str = r#"
window.rd-overlay {
    background: transparent;
}
window.rd-dialog {
    background: @theme_bg_color;
}
.rd-title {
    font-size: 15px;
    font-weight: 600;
}
.rd-subtle {
    opacity: 0.62;
    font-size: 12px;
}
.rd-hint {
    opacity: 0.7;
    font-size: 12px;
}
.rd-project-row {
    padding: 2px 4px;
}
.rd-badge {
    border-radius: 6px;
    padding: 1px 7px;
    font-size: 11px;
    background: alpha(currentColor, 0.10);
}
.rd-error {
    color: #e5484d;
    font-size: 12px;
}
.rd-badge-error {
    color: #e5484d;
    background: alpha(#e5484d, 0.14);
}
.rd-card {
    border-radius: 10px;
    padding: 12px;
    background: alpha(currentColor, 0.04);
}
.rd-footer {
    font-size: 11px;
    opacity: 0.6;
}
"#;

pub fn install_css() {
    let provider = CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn is_dark() -> bool {
    let settings = gtk::Settings::default();
    let theme = settings
        .as_ref()
        .and_then(|settings| settings.gtk_theme_name())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let prefer_dark = settings
        .as_ref()
        .is_some_and(|settings| settings.is_gtk_application_prefer_dark_theme());
    prefer_dark || theme.contains("dark")
}

/// Applies the user's theme preference. `System` leaves GTK's default
/// (which follows the desktop's GTK theme/portal) untouched.
pub fn apply_theme(theme: Theme) {
    if let Some(settings) = gtk::Settings::default() {
        match theme {
            Theme::System => settings.set_gtk_application_prefer_dark_theme(false),
            Theme::Light => settings.set_gtk_application_prefer_dark_theme(false),
            Theme::Dark => settings.set_gtk_application_prefer_dark_theme(true),
        }
    }
}

pub fn parse_hex(color: &str) -> (f64, f64, f64) {
    let hex = color.trim_start_matches('#');
    let parsed = u32::from_str_radix(hex.get(..6).unwrap_or("3b82f6"), 16).unwrap_or(0x3b82f6);
    (
        ((parsed >> 16) & 0xff) as f64 / 255.0,
        ((parsed >> 8) & 0xff) as f64 / 255.0,
        (parsed & 0xff) as f64 / 255.0,
    )
}
