use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use gtk::{gdk, CssProvider};

use deck_core::Theme;

/// Cached so Cairo repaints never touch the filesystem or GTK settings.
static DARK: AtomicBool = AtomicBool::new(false);
/// Explicit user preference: 0 = system, 1 = light, 2 = dark.
static PREFERENCE: AtomicU8 = AtomicU8::new(0);

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
.rd-color-selected {
    border: 2px solid currentColor;
    border-radius: 8px;
    padding: 2px;
}
.rd-footer {
    font-size: 11px;
    opacity: 0.6;
}

/* Radial action overlay */
window.rd-action-overlay {
    background: transparent;
}
.rd-scrim {
    background: rgba(4, 4, 6, 0.62);
}
.rd-panel {
    background: #fbfaf9;
    color: #1d1d1f;
    border: 1px solid rgba(0, 0, 0, 0.07);
    border-radius: 30px;
    padding: 18px 28px 16px;
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.32);
}
.rd-panel-dark {
    background: #1b1b1f;
    color: #f5f5f7;
    border-color: rgba(255, 255, 255, 0.08);
    box-shadow: 0 30px 80px rgba(0, 0, 0, 0.55);
}
.rd-panel.rd-drop-hot {
    border: 2px solid #ff5a00;
}
.rd-modes {
    margin-bottom: 2px;
}
.rd-mode {
    background: transparent;
    border: 1px solid alpha(currentColor, 0.12);
    border-radius: 9px;
    padding: 3px 10px;
    min-height: 26px;
    font-size: 11px;
    font-weight: 600;
    color: inherit;
    box-shadow: none;
}
.rd-mode:hover {
    background: alpha(currentColor, 0.07);
}
.rd-mode.active {
    background: rgba(255, 90, 0, 0.14);
    border-color: #ff5a00;
    color: #d9480f;
}
.rd-panel-dark .rd-mode.active {
    color: #ff9a62;
}
.rd-form {
    margin-top: 6px;
    padding: 8px 6px 4px;
}
.rd-form-title {
    font-size: 20px;
    font-weight: 700;
}
.rd-form-hint {
    font-size: 11px;
    opacity: 0.6;
}
.rd-form-row {
    min-height: 38px;
}
.rd-form-label {
    font-size: 12px;
    font-weight: 600;
}
.rd-form-button {
    border-radius: 9px;
    padding: 5px 14px;
}
.rd-form-run {
    border-radius: 9px;
    padding: 5px 18px;
    background: #ff5a00;
    color: white;
    font-weight: 700;
}
.rd-form-run:hover {
    background: #e65000;
}
.rd-tab {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 11px;
    padding: 4px 10px;
    min-height: 28px;
    color: inherit;
    box-shadow: none;
}
.rd-tab:hover {
    background: alpha(currentColor, 0.07);
}
.rd-tab.active {
    background: rgba(255, 90, 0, 0.10);
    border-color: #ff5a00;
}
.rd-tab:focus,
.rd-rail-action:focus,
.rd-tab-add:focus,
.rd-menu-item:focus {
    border-color: rgba(245, 245, 247, 0.85);
}
.rd-tab-label {
    font-size: 11px;
    font-weight: 500;
}
.rd-tab-more {
    min-width: 30px;
}
.rd-tab-add {
    min-width: 30px;
    min-height: 30px;
    padding: 3px;
    border-radius: 9px;
}
.rd-rail-action {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 10px;
    padding: 4px 10px;
    min-height: 28px;
    font-size: 11px;
    color: inherit;
    box-shadow: none;
}
.rd-rail-action:hover {
    background: alpha(currentColor, 0.07);
}
.rd-heading {
    font-size: 30px;
    font-weight: 700;
}
.rd-caption {
    font-size: 12px;
    opacity: 0.66;
}
.rd-key {
    background: rgba(255, 90, 0, 0.12);
    border: 1px solid rgba(255, 90, 0, 0.22);
    border-radius: 10px;
    padding: 6px 12px;
}
.rd-key-label {
    font-size: 11px;
    font-weight: 700;
    color: #d9480f;
}
.rd-panel-dark .rd-key-label {
    color: #ff8a4c;
}
.rd-status {
    font-size: 12px;
    color: #c2410c;
}

/* Float shelf */
.rd-float-window {
    background: transparent;
}
.rd-float-panel {
    background: @theme_bg_color;
    color: @theme_fg_color;
    border: 1px solid alpha(currentColor, 0.14);
    border-radius: 20px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.30);
}
.rd-float-header {
    padding: 12px 14px 8px;
}
.rd-float-title {
    font-size: 15px;
    font-weight: 700;
}
.rd-float-row {
    padding: 7px 8px;
    border-radius: 9px;
    background: alpha(currentColor, 0.04);
}
.rd-float-row:hover {
    background: alpha(currentColor, 0.09);
}
.rd-float-empty {
    font-size: 12px;
    opacity: 0.55;
    margin: 24px;
}

/* Management window */
.rd-manage-window {
    background: @theme_bg_color;
}
.rd-manage-heading {
    font-size: 21px;
    font-weight: 700;
}
.rd-manage-subtitle {
    font-size: 12px;
    opacity: 0.62;
}
.rd-folder-list {
    background: transparent;
}
.rd-folder-list row.rd-folder-row {
    background: transparent;
    border: 1px solid alpha(currentColor, 0.09);
    border-radius: 10px;
    margin-bottom: 4px;
}
.rd-folder-list row.rd-folder-row:hover {
    background: alpha(currentColor, 0.05);
}
.rd-folder-name {
    font-size: 13px;
    font-weight: 600;
}
.rd-folder-path {
    font-size: 11px;
    opacity: 0.55;
}
.rd-row-action {
    min-height: 26px;
    padding: 2px 10px;
    border-radius: 8px;
    font-size: 11px;
}
button.rd-icon-button,
menubutton.rd-icon-button {
    background: transparent;
    border: 0;
    box-shadow: none;
    padding: 4px;
    min-width: 28px;
    min-height: 28px;
    color: inherit;
}
button.rd-icon-button:hover,
menubutton.rd-icon-button:hover {
    background: alpha(currentColor, 0.09);
    border-radius: 8px;
}
.rd-menu-popover {
    background: @theme_bg_color;
    color: @theme_fg_color;
    border: 1px solid alpha(currentColor, 0.14);
    border-radius: 12px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.22);
}
.rd-menu {
    background: transparent;
}
.rd-menu-item {
    background: transparent;
    border: 0;
    border-radius: 7px;
    padding: 6px 10px;
    min-height: 26px;
    color: inherit;
    box-shadow: none;
}
.rd-menu-item:hover {
    background: alpha(currentColor, 0.08);
}
.rd-menu-item.destructive-action {
    color: #e5484d;
}
.rd-menu-meta {
    font-size: 10px;
    opacity: 0.55;
    margin-left: 10px;
}
"#;

pub fn install_css() {
    refresh_dark();
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
    DARK.load(Ordering::Relaxed)
}

/// Recomputes the effective colour scheme; called at startup and whenever the
/// user changes the theme preference.
pub fn refresh_dark() {
    DARK.store(detect_dark(), Ordering::Relaxed);
}

fn detect_dark() -> bool {
    match PREFERENCE.load(Ordering::Relaxed) {
        1 => return false,
        2 => return true,
        _ => {}
    }
    let settings = gtk::Settings::default();
    let theme = settings
        .as_ref()
        .and_then(|settings| settings.gtk_theme_name())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let prefer_dark = settings
        .as_ref()
        .is_some_and(|settings| settings.is_gtk_application_prefer_dark_theme());
    prefer_dark || theme.contains("dark") || kde_dark_scheme()
}

/// GTK on Plasma does not always expose the colour scheme, so consult the
/// active KDE colour scheme as a fallback.
fn kde_dark_scheme() -> bool {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        });
    let Some(base) = base else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(base.join("kdeglobals")) else {
        return false;
    };
    text.lines()
        .find_map(|line| line.strip_prefix("ColorScheme="))
        .map(|scheme| scheme.to_ascii_lowercase().contains("dark"))
        .unwrap_or(false)
}

/// Applies the user's theme preference. `System` leaves GTK's default
/// (which follows the desktop's GTK theme/portal) untouched.
pub fn apply_theme(theme: Theme) {
    PREFERENCE.store(
        match theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        },
        Ordering::Relaxed,
    );
    if let Some(settings) = gtk::Settings::default() {
        match theme {
            Theme::System => {}
            Theme::Light => settings.set_gtk_application_prefer_dark_theme(false),
            Theme::Dark => settings.set_gtk_application_prefer_dark_theme(true),
        }
    }
    refresh_dark();
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
