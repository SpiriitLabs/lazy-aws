use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::ui::style::theme;

const TAB_LABELS: &[&str] = &["1:ECS", "2:Tasks", "3:SSM", "4:Logs"];

/// Height of the tab bar.
pub const TAB_BAR_H: u16 = 1;

/// Renders the tab bar at the given area.
pub fn render_tab_bar(active: usize, area: Rect, buf: &mut Buffer) {
    let bg = Style::default().fg(theme::color_muted());
    for x in area.x..area.x + area.width {
        buf.set_string(x, area.y, " ", bg);
    }

    let mut x = area.x + 1;
    for (i, label) in TAB_LABELS.iter().enumerate() {
        let is_active = i == active;
        if is_active {
            let style = Style::default()
                .fg(theme::color_primary())
                .add_modifier(Modifier::BOLD);
            buf.set_string(x, area.y, "[", style);
            x += 1;
            buf.set_string(x, area.y, label, style);
            x += label.len() as u16;
            buf.set_string(x, area.y, "]", style);
            x += 1;
        } else {
            let style = Style::default().fg(theme::color_muted());
            buf.set_string(x, area.y, " ", style);
            x += 1;
            buf.set_string(x, area.y, label, style);
            x += label.len() as u16;
            buf.set_string(x, area.y, " ", style);
            x += 1;
        }
        x += 1;
    }
}

/// Returns the tab index at the given column position, or None if outside any tab.
pub fn tab_index_at(col: u16) -> Option<usize> {
    let mut x: u16 = 1;
    for (i, label) in TAB_LABELS.iter().enumerate() {
        let tab_width = 1 + label.len() as u16 + 1;
        if col >= x && col < x + tab_width {
            return Some(i);
        }
        x += tab_width + 1;
    }
    None
}

/// String-based tab bar view (for tests).
pub fn tab_bar_view(active: usize) -> String {
    TAB_LABELS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            if i == active {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_bar_view_ecs() {
        let view = tab_bar_view(0);
        assert!(view.contains("[1:ECS]"));
        assert!(view.contains(" 2:Tasks "));
        assert!(view.contains(" 3:SSM "));
        assert!(view.contains(" 4:Logs "));
    }

    #[test]
    fn tab_bar_view_tasks() {
        let view = tab_bar_view(1);
        assert!(view.contains(" 1:ECS "));
        assert!(view.contains("[2:Tasks]"));
    }

    #[test]
    fn tab_bar_view_ssm() {
        let view = tab_bar_view(2);
        assert!(view.contains("[3:SSM]"));
    }

    #[test]
    fn tab_bar_view_logs() {
        let view = tab_bar_view(3);
        assert!(view.contains("[4:Logs]"));
    }
}
