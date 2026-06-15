use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Widget};

use crate::aws::LogGroup;
use crate::ui::fuzzy::fuzzy_match;
use crate::ui::style::{styles, theme};

pub struct LogGroupsPanel {
    pub groups: Vec<LogGroup>,
    filtered: Vec<usize>,
    pub filter: String,
    pub cursor: usize,
}

impl Default for LogGroupsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl LogGroupsPanel {
    pub fn new() -> Self {
        LogGroupsPanel {
            groups: vec![],
            filtered: vec![],
            filter: String::new(),
            cursor: 0,
        }
    }

    pub fn set_groups(&mut self, groups: Vec<LogGroup>) {
        self.groups = groups;
        self.rebuild_filter();
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.rebuild_filter();
    }

    pub fn clear_filter(&mut self) {
        self.set_filter("");
    }

    fn rebuild_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered = (0..self.groups.len()).collect();
        } else {
            let mut scored: Vec<(usize, i32)> = self
                .groups
                .iter()
                .enumerate()
                .filter_map(|(i, g)| fuzzy_match(&g.log_group_name, &self.filter).map(|s| (i, s)))
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }
        let count = self.filtered.len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        } else if count == 0 {
            self.cursor = 0;
        }
    }

    fn visible(&self) -> Vec<&LogGroup> {
        self.filtered
            .iter()
            .filter_map(|&i| self.groups.get(i))
            .collect()
    }

    pub fn selected(&self) -> Option<&LogGroup> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.groups.get(i))
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let count = self.filtered.len();
        if count > 0 && self.cursor < count - 1 {
            self.cursor += 1;
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, is_active: bool, loading: bool) {
        let border_color = if is_active {
            theme::color_border_focus()
        } else {
            theme::color_border()
        };
        let filter_text = if self.filter.is_empty() {
            String::new()
        } else {
            format!(" /{}", self.filter)
        };
        let ct = if self.filter.is_empty() {
            format!("{}", self.filtered.len())
        } else {
            format!("{}/{}", self.filtered.len(), self.groups.len())
        };
        let block = Block::default()
            .title(format!(" Log Groups [{}]{} ", ct, filter_text))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        let items = self.visible();
        if loading {
            crate::ui::panels::render_placeholder(buf, inner, "Loading...", true);
            return;
        }
        if items.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            buf.set_string(
                inner.x + 1,
                inner.y,
                if self.filter.is_empty() {
                    "No log groups found"
                } else {
                    "No match"
                },
                style,
            );
            return;
        }

        let visible = inner.height as usize;
        let offset = if self.cursor >= visible {
            self.cursor - visible + 1
        } else {
            0
        };

        for (i, group) in items.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let is_selected = (i + offset) == self.cursor;

            let style = if is_selected {
                styles::selection_style(is_active)
            } else {
                Style::default().fg(theme::color_text())
            };

            buf.set_string(inner.x + 1, y, format!(" {}", group.log_group_name), style);
        }
    }
}
