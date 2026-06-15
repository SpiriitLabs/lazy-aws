use ratatui::style::{Modifier, Style};

use super::theme;

pub fn title_style() -> Style {
    Style::default()
        .fg(theme::color_bright())
        .add_modifier(Modifier::BOLD)
}

pub fn description_style() -> Style {
    Style::default().fg(theme::color_text())
}

pub fn key_style() -> Style {
    Style::default()
        .fg(theme::color_primary())
        .add_modifier(Modifier::BOLD)
}

pub fn muted_style() -> Style {
    Style::default().fg(theme::color_muted())
}

pub fn success_style() -> Style {
    Style::default().fg(theme::color_success())
}

pub fn warning_style() -> Style {
    Style::default().fg(theme::color_warning())
}

pub fn error_style() -> Style {
    Style::default()
        .fg(theme::color_danger())
        .add_modifier(Modifier::BOLD)
}

pub fn section_header_style() -> Style {
    Style::default()
        .fg(theme::color_muted())
        .add_modifier(Modifier::BOLD)
}

/// Style d'une ligne sélectionnée dans une liste.
///
/// `is_active` indique que le panel a le focus. On utilise un fond plein
/// (orange primaire si actif, gris secondaire sinon) plutôt que l'ancien
/// `REVERSED` : c'est plus calme à l'œil et cohérent entre tous les panels.
pub fn selection_style(is_active: bool) -> Style {
    if is_active {
        Style::default()
            .fg(theme::color_bright())
            .bg(theme::color_primary())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme::color_text())
            .bg(theme::color_secondary())
            .add_modifier(Modifier::BOLD)
    }
}
