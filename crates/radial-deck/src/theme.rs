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
.rd-badge-error,
.rd-badge-missing {
    color: #e5484d;
    background: alpha(#e5484d, 0.14);
}
.rd-mono {
    font-family: monospace;
}
.rd-progress-window {
    background: transparent;
}
.rd-progress-window .rd-card {
    background: @theme_bg_color;
    border: 1px solid alpha(currentColor, 0.12);
    border-radius: 12px;
}
.rd-progress-title {
    font-weight: 600;
}
.rd-progress-head {
    spacing: 8px;
}
.rd-progress-count {
    font-family: monospace;
    font-size: 11px;
    opacity: 0.62;
}
.rd-progress-failed {
    font-size: 11px;
}
.rd-progress-bar trough {
    background: alpha(currentColor, 0.12);
    border: none;
    border-radius: 4px;
    min-height: 6px;
}
.rd-progress-bar progress {
    border: none;
    border-radius: 4px;
    min-height: 6px;
    box-shadow: none;
}
/* Fill colours are per theme so the bar clears 3:1 against its trough. */
.rd-progress-window.rd-theme-light .rd-progress-bar progress {
    background: #d44900;
}
.rd-progress-window.rd-theme-dark .rd-progress-bar progress {
    background: #ff5a00;
}
.rd-progress-cancel {
    background: transparent;
    border: 1px solid alpha(currentColor, 0.16);
    border-radius: 9px;
    padding: 3px 14px;
    min-height: 26px;
    font-size: 12px;
    font-weight: 600;
    color: inherit;
    box-shadow: none;
}
.rd-progress-cancel:hover {
    background: alpha(currentColor, 0.07);
}
.rd-progress-cancel:active {
    background: alpha(currentColor, 0.12);
}
.rd-card {
    border-radius: 10px;
    padding: 12px;
    background: alpha(currentColor, 0.04);
}

/* Settings sidebar */
.rd-sidebar-pane {
    background: alpha(currentColor, 0.035);
}
.rd-sidebar {
    background: transparent;
}
.rd-sidebar row {
    border-radius: 8px;
    min-height: 30px;
    margin: 1px 0;
}
.rd-sidebar row:hover {
    background: alpha(currentColor, 0.06);
}
.rd-sidebar row:selected {
    background: alpha(currentColor, 0.12);
    /* Adwaita paints selected rows with the accent's foreground colour, which
       is white; the translucent pill above needs the normal text colour. */
    color: @theme_fg_color;
}
.rd-sidebar-icon {
    opacity: 0.7;
}
.rd-sidebar-app {
    font-weight: 700;
    font-size: 14px;
}
.rd-sidebar-sub {
    opacity: 0.55;
    font-size: 11px;
}
.rd-sidebar-version {
    font-size: 10px;
    opacity: 0.45;
}
.rd-page-title {
    font-size: 21px;
    font-weight: 700;
}
.rd-page-subtitle {
    opacity: 0.62;
    font-size: 12px;
}
.rd-tool-row {
    padding: 3px 4px;
}
.rd-tool-index {
    font-family: monospace;
    font-size: 11px;
    opacity: 0.45;
    min-width: 18px;
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
    color: #bf4000;
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
    /* currentColor adapts to panel/popover theme; 55% clears 3:1 in both. */
    border-color: alpha(currentColor, 0.55);
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
    color: #bf4000;
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
        // Accent rules sit one level above the static sheet so a KDE accent
        // colour can override the built-in orange without touching it.
        let accent_provider = CssProvider::new();
        accent_provider.load_from_string(&accent_css());
        gtk::style_context_add_provider_for_display(
            &display,
            &accent_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
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

/// The app accent: the KDE accent colour when the user picked one in System
/// Settings, else the RadialDeck orange. Read once per process; changing the
/// system accent takes effect on the next start.
pub fn accent() -> (f64, f64, f64) {
    static ACCENT: std::sync::OnceLock<(f64, f64, f64)> = std::sync::OnceLock::new();
    *ACCENT.get_or_init(|| {
        if let Some(forced) = std::env::var_os("RADIAL_DECK_FORCE_ACCENT") {
            if let Some(color) = parse_rgb_list(&forced.to_string_lossy()) {
                return color;
            }
        }
        read_kde_accent().unwrap_or((1.0, 0.353, 0.0)) // #ff5a00
    })
}

/// `AccentColor=r,g,b` from kdeglobals (Plasma 6), ignoring malformed or
/// absent values. Same file and fallback style as `kde_dark_scheme`.
fn read_kde_accent() -> Option<(f64, f64, f64)> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        })?;
    let text = std::fs::read_to_string(base.join("kdeglobals")).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("AccentColor="))
        .and_then(parse_rgb_list)
}

/// Parses a `r,g,b[,a]` list of 0-255 integers, as Plasma stores accents.
fn parse_rgb_list(value: &str) -> Option<(f64, f64, f64)> {
    let mut channels = value.split(',').map(str::trim).map(str::parse::<u32>);
    let r = channels.next()?.ok()?;
    let g = channels.next()?.ok()?;
    let b = channels.next()?.ok()?;
    if r.max(g).max(b) > 255 {
        return None;
    }
    Some((
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    ))
}

fn relative_luminance(color: (f64, f64, f64)) -> f64 {
    fn channel(value: f64) -> f64 {
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.0) + 0.7152 * channel(color.1) + 0.0722 * channel(color.2)
}

fn contrast(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let (light, dark) = if relative_luminance(a) >= relative_luminance(b) {
        (a, b)
    } else {
        (b, a)
    };
    (relative_luminance(light) + 0.05) / (relative_luminance(dark) + 0.05)
}

/// Nudges `color` toward black or white until it clears `target` contrast
/// against `against`, in 5% steps. Keeps accent-derived fills and text inside
/// WCAG without banning vivid accent choices outright.
fn scale_until_contrast(
    color: (f64, f64, f64),
    against: (f64, f64, f64),
    target: f64,
    darken: bool,
) -> (f64, f64, f64) {
    let mut current = color;
    for _ in 0..40 {
        if contrast(current, against) >= target {
            return current;
        }
        let factor = if darken { 0.95 } else { 1.05 };
        current = (
            (current.0 * factor).clamp(0.0, 1.0),
            (current.1 * factor).clamp(0.0, 1.0),
            (current.2 * factor).clamp(0.0, 1.0),
        );
    }
    current
}

fn hex(color: (f64, f64, f64)) -> String {
    let byte = |value: f64| (value * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        byte(color.0),
        byte(color.1),
        byte(color.2)
    )
}

/// Overrides for every accent-coloured rule, derived from one accent colour
/// with contrast computed instead of assumed. Trough colours mirror the
/// `.rd-progress-bar trough` blend over each panel background.
fn accent_css() -> String {
    let accent = accent();
    let panel_light = (0.984, 0.980, 0.976); // #fbfaf9
    let panel_dark = (0.106, 0.106, 0.122); // #1b1b1f
    let trough_light = (0.878, 0.870, 0.867); // currentColor 12% over the panel
    let trough_dark = (0.208, 0.207, 0.216);
    let on_accent = if relative_luminance(accent) > 0.35 {
        "#111111"
    } else {
        "#ffffff"
    };
    let fill_light = hex(scale_until_contrast(accent, trough_light, 3.0, true));
    let fill_dark = hex(scale_until_contrast(accent, trough_dark, 3.0, false));
    let text_light = hex(scale_until_contrast(accent, panel_light, 4.5, true));
    let text_dark = hex(scale_until_contrast(accent, panel_dark, 4.5, false));
    let hover = hex(scale_until_contrast(accent, panel_light, 1.15, true));
    let a = hex(accent);
    format!(
        r#"
.rd-mode.active {{
    background: alpha({a}, 0.14);
    border-color: {a};
    color: {text_light};
}}
.rd-panel-dark .rd-mode.active {{
    color: {text_dark};
}}
.rd-tab.active {{
    background: alpha({a}, 0.10);
    border-color: {a};
}}
.rd-form-run {{
    background: {a};
    color: {on_accent};
}}
.rd-form-run:hover {{
    background: {hover};
}}
.rd-key {{
    background: alpha({a}, 0.12);
    border-color: alpha({a}, 0.22);
}}
.rd-key-label {{
    color: {text_light};
}}
.rd-panel-dark .rd-key-label {{
    color: {text_dark};
}}
.rd-panel.rd-drop-hot {{
    border: 2px solid {a};
}}
.rd-progress-window.rd-theme-light .rd-progress-bar progress {{
    background: {fill_light};
}}
.rd-progress-window.rd-theme-dark .rd-progress-bar progress {{
    background: {fill_dark};
}}
"#
    )
}

#[cfg(test)]
mod accent_tests {
    use super::*;

    #[test]
    fn parses_kde_accent_lists() {
        assert_eq!(parse_rgb_list("255,90,0"), Some((1.0, 90.0 / 255.0, 0.0)));
        assert_eq!(
            parse_rgb_list("61, 129, 246, 255"),
            Some((61.0 / 255.0, 129.0 / 255.0, 246.0 / 255.0))
        );
        assert_eq!(parse_rgb_list("300,0,0"), None);
        assert_eq!(parse_rgb_list("red"), None);
    }

    #[test]
    fn scaled_fills_clear_their_targets() {
        let orange = (1.0, 0.353, 0.0);
        let trough_light = (0.878, 0.870, 0.867);
        let trough_dark = (0.208, 0.207, 0.216);
        let fill_light = scale_until_contrast(orange, trough_light, 3.0, true);
        let fill_dark = scale_until_contrast(orange, trough_dark, 3.0, false);
        assert!(contrast(fill_light, trough_light) >= 3.0);
        assert!(contrast(fill_dark, trough_dark) >= 3.0);
    }

    #[test]
    fn on_accent_text_follows_luminance() {
        // Dark accent gets white text; a near-white accent gets dark text.
        let dark = (0.1, 0.1, 0.45);
        assert!(relative_luminance(dark) <= 0.35);
        assert!(relative_luminance((0.95, 0.95, 0.95)) > 0.35);
    }
}
