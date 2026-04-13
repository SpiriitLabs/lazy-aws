use ratatui::style::Color;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
}

static THEME_MODE: Mutex<ThemeMode> = Mutex::new(ThemeMode::Dark);

pub fn set_mode(mode: ThemeMode) {
    *THEME_MODE.lock().unwrap() = mode;
}

pub fn toggle_mode() {
    let mut m = THEME_MODE.lock().unwrap();
    *m = match *m {
        ThemeMode::Dark => ThemeMode::Light,
        ThemeMode::Light => ThemeMode::Dark,
    };
}

pub fn mode() -> ThemeMode {
    *THEME_MODE.lock().unwrap()
}

fn is_light() -> bool {
    mode() == ThemeMode::Light
}

// --- Dynamic colors based on theme ---

pub fn color_primary() -> Color {
    Color::Rgb(0xC4, 0x72, 0x00)
} // darker orange, readable on both
pub fn color_secondary() -> Color {
    if is_light() {
        Color::Rgb(0xE8, 0xEB, 0xF0)
    } else {
        Color::Rgb(0x23, 0x2F, 0x3E)
    }
}
pub fn color_success() -> Color {
    if is_light() {
        Color::Rgb(0x00, 0x80, 0x00)
    } else {
        Color::Rgb(0x00, 0xCC, 0x00)
    }
}
pub fn color_warning() -> Color {
    if is_light() {
        Color::Rgb(0x99, 0x80, 0x00)
    } else {
        Color::Rgb(0xCC, 0xCC, 0x00)
    }
}
pub fn color_danger() -> Color {
    Color::Rgb(0xCC, 0x00, 0x00)
}
pub fn color_info() -> Color {
    if is_light() {
        Color::Rgb(0x00, 0x80, 0x99)
    } else {
        Color::Rgb(0x00, 0xCC, 0xCC)
    }
}
pub fn color_muted() -> Color {
    Color::Rgb(0x80, 0x80, 0x80)
}
pub fn color_text() -> Color {
    if is_light() {
        Color::Rgb(0x1A, 0x1A, 0x1A)
    } else {
        Color::Rgb(0xCC, 0xCC, 0xCC)
    }
}
pub fn color_bright() -> Color {
    if is_light() {
        Color::Rgb(0x00, 0x00, 0x00)
    } else {
        Color::Rgb(0xFF, 0xFF, 0xFF)
    }
}
pub fn color_background() -> Color {
    if is_light() {
        Color::Rgb(0xFF, 0xFF, 0xFF)
    } else {
        Color::Rgb(0x00, 0x00, 0x00)
    }
}
pub fn color_border() -> Color {
    if is_light() {
        Color::Rgb(0xAA, 0xAA, 0xAA)
    } else {
        Color::Rgb(0x80, 0x80, 0x80)
    }
}
pub fn color_border_focus() -> Color {
    color_primary()
}
pub fn color_tab_active() -> Color {
    color_primary()
}
pub fn color_tab_inactive() -> Color {
    color_muted()
}

// Keep backward-compatible constants that delegate to functions
// These are used by all panels and components
pub const COLOR_PRIMARY: Color = Color::Rgb(0xC4, 0x72, 0x00);

// Status colors (same in both themes -- already high contrast)
pub fn status_color(status: &str) -> Color {
    match status.to_uppercase().as_str() {
        "ACTIVE" | "RUNNING" => color_success(),
        "DRAINING" | "PENDING" | "PROVISIONING" => color_warning(),
        "STOPPED" | "INACTIVE" | "DEPROVISIONING" => color_danger(),
        "ONLINE" => color_success(),
        "CONNECTIONLOST" => color_danger(),
        _ => color_muted(),
    }
}

/// Auto-detect terminal background color.
/// Uses COLORFGBG env var if available, defaults to Dark otherwise.
/// Use `--light` flag or `Ctrl+L` to manually switch.
pub fn detect_mode() -> ThemeMode {
    // Try COLORFGBG env var (format: "fg;bg", bg >= 7 means light background)
    if let Ok(val) = std::env::var("COLORFGBG") {
        if let Some(bg) = val.rsplit(';').next() {
            if let Ok(n) = bg.parse::<u32>() {
                return if n >= 7 && n != 8 {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
            }
        }
    }

    ThemeMode::Dark
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_color_active() {
        assert_eq!(status_color("ACTIVE"), color_success());
    }

    #[test]
    fn status_color_running() {
        assert_eq!(status_color("RUNNING"), color_success());
    }

    #[test]
    fn status_color_stopped() {
        assert_eq!(status_color("STOPPED"), color_danger());
    }

    #[test]
    fn status_color_unknown() {
        assert_eq!(status_color("something-else"), color_muted());
    }
}
