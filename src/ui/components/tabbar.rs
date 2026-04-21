use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::ui::style::theme;

const TAB_LABELS_FULL: &[&str] = &["1:ECS", "2:Tasks", "3:SSM", "4:Logs", "5:RDS", "6:S3"];
const TAB_LABELS_SHORT: &[&str] = &["1:E", "2:T", "3:S", "4:L", "5:R", "6:B"];
/// Below this width, the tab bar switches to the short label set.
const COMPACT_THRESHOLD: u16 = 50;

/// Height of the tab bar.
pub const TAB_BAR_H: u16 = 1;

fn labels_for(width: u16) -> &'static [&'static str] {
    if width < COMPACT_THRESHOLD {
        TAB_LABELS_SHORT
    } else {
        TAB_LABELS_FULL
    }
}

/// Renders the tab bar at the given area.
pub fn render_tab_bar(active: usize, area: Rect, buf: &mut Buffer) {
    let bg = Style::default().fg(theme::color_muted());
    for x in area.x..area.x + area.width {
        buf.set_string(x, area.y, " ", bg);
    }

    let labels = labels_for(area.width);
    let right_limit = area.x + area.width;
    let mut x = area.x + 1;
    for (i, label) in labels.iter().enumerate() {
        let tab_width = 1 + label.len() as u16 + 1;
        if x + tab_width > right_limit {
            break;
        }
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
/// `width` is the current tab bar width, used to choose between full and short labels.
pub fn tab_index_at(col: u16, width: u16) -> Option<usize> {
    let labels = labels_for(width);
    let mut x: u16 = 1;
    for (i, label) in labels.iter().enumerate() {
        let tab_width = 1 + label.len() as u16 + 1;
        if col >= x && col < x + tab_width {
            return Some(i);
        }
        x += tab_width + 1;
    }
    None
}

/// String-based tab bar view (for tests, uses full labels).
pub fn tab_bar_view(active: usize) -> String {
    TAB_LABELS_FULL
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
        assert!(view.contains(" 5:RDS "));
        assert!(view.contains(" 6:S3 "));
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

    #[test]
    fn labels_compact_under_threshold() {
        assert_eq!(labels_for(COMPACT_THRESHOLD - 1), TAB_LABELS_SHORT);
    }

    #[test]
    fn labels_full_at_or_above_threshold() {
        assert_eq!(labels_for(COMPACT_THRESHOLD), TAB_LABELS_FULL);
        assert_eq!(labels_for(200), TAB_LABELS_FULL);
    }

    #[test]
    fn tab_index_at_full_labels() {
        // Tab 0 "1:ECS" occupies cols 1..8 (width = 1 + 5 + 1); separator at col 8; Tab 1 starts at col 9.
        assert_eq!(tab_index_at(1, 200), Some(0));
        assert_eq!(tab_index_at(7, 200), Some(0));
        assert_eq!(tab_index_at(8, 200), None);
        assert_eq!(tab_index_at(9, 200), Some(1));
    }

    #[test]
    fn tab_index_at_compact_labels() {
        // Short labels have len 3; tab width = 5; separator +1 between tabs.
        // Tab 0: cols [1, 6); separator at 6; Tab 1: cols [7, 12); Tab 2: cols [13, 18).
        assert_eq!(tab_index_at(1, 40), Some(0));
        assert_eq!(tab_index_at(5, 40), Some(0));
        assert_eq!(tab_index_at(6, 40), None);
        assert_eq!(tab_index_at(7, 40), Some(1));
        assert_eq!(tab_index_at(13, 40), Some(2));
    }
}
