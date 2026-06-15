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
        // ECS / SSM
        "ACTIVE" | "RUNNING" | "ONLINE" => color_success(),
        "DRAINING" | "PENDING" | "PROVISIONING" => color_warning(),
        "STOPPED" | "INACTIVE" | "DEPROVISIONING" => color_danger(),
        "CONNECTIONLOST" => color_danger(),
        // RDS (DBInstanceStatus)
        "AVAILABLE" => color_success(),
        "STARTING"
        | "STOPPING"
        | "MODIFYING"
        | "BACKING-UP"
        | "REBOOTING"
        | "CREATING"
        | "MAINTENANCE"
        | "UPGRADING"
        | "CONFIGURING-ENHANCED-MONITORING"
        | "RENAMING" => color_warning(),
        "FAILED" | "STORAGE-FULL" | "INACCESSIBLE-ENCRYPTION-CREDENTIALS" | "RESTORE-ERROR" => {
            color_danger()
        }
        s if s.starts_with("INCOMPATIBLE-") => color_danger(),
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

/// Couleur dérivée du niveau de log détecté dans une ligne (None si aucun).
///
/// Cherche un token de niveau délimité (entre bornes de mot) pour éviter de
/// matcher « INFORMATION » ou une sous-chaîne au milieu d'une URL.
/// INFO/NOTICE retournent None : ce sont les lignes "normales", on ne les
/// sur-colorie pas pour laisser ressortir les warnings et erreurs.
pub fn log_level_color(line: &str) -> Option<Color> {
    let upper = line.to_uppercase();
    let bytes = upper.as_bytes();

    // Renvoie true si `token` apparaît dans `upper` entouré de non-alphanumériques.
    let has_token = |token: &str| -> bool {
        let tb = token.as_bytes();
        let mut start = 0;
        while let Some(off) = upper[start..].find(token) {
            let at = start + off;
            let before_ok = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
            let after = at + tb.len();
            let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
            start = at + 1;
        }
        false
    };

    if [
        "ERROR", "ERR", "FATAL", "CRITICAL", "CRIT", "PANIC", "SEVERE",
    ]
    .iter()
    .any(|t| has_token(t))
    {
        Some(color_danger())
    } else if ["WARN", "WARNING"].iter().any(|t| has_token(t)) {
        Some(color_warning())
    } else if ["DEBUG", "TRACE"].iter().any(|t| has_token(t)) {
        Some(color_muted())
    } else {
        None
    }
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

    #[test]
    fn status_color_rds() {
        assert_eq!(status_color("available"), color_success());
        assert_eq!(status_color("starting"), color_warning());
        assert_eq!(status_color("failed"), color_danger());
        assert_eq!(status_color("incompatible-network"), color_danger());
    }

    #[test]
    fn log_level_error() {
        assert_eq!(
            log_level_color("2026-06-15T12:00:00Z [ERROR] boom"),
            Some(color_danger())
        );
        assert_eq!(
            log_level_color("level=fatal msg=crash"),
            Some(color_danger())
        );
    }

    #[test]
    fn log_level_warn() {
        assert_eq!(
            log_level_color("12:00 WARN disk almost full"),
            Some(color_warning())
        );
        assert_eq!(
            log_level_color("WARNING: deprecated"),
            Some(color_warning())
        );
    }

    #[test]
    fn log_level_debug() {
        assert_eq!(log_level_color("DEBUG connecting"), Some(color_muted()));
    }

    #[test]
    fn log_level_info_is_none() {
        assert_eq!(log_level_color("INFO request handled"), None);
        assert_eq!(log_level_color("plain message"), None);
    }

    #[test]
    fn log_level_no_substring_false_positive() {
        // "ERROR" ne doit pas matcher à l'intérieur d'un mot
        assert_eq!(log_level_color("TERRORISM report"), None);
        assert_eq!(log_level_color("INFORMATION only"), None);
    }
}
