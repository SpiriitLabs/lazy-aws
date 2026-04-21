use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::style::theme;

pub struct RdsTablesPanel {
    pub tables: Vec<String>,
    filtered: Vec<usize>,
    pub filter: String,
    pub cursor: usize,
}

impl Default for RdsTablesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl RdsTablesPanel {
    pub fn new() -> Self {
        RdsTablesPanel {
            tables: vec![],
            filtered: vec![],
            filter: String::new(),
            cursor: 0,
        }
    }

    pub fn set_tables(&mut self, tables: Vec<String>) {
        self.tables = tables;
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
        let lower = self.filter.to_lowercase();
        self.filtered = self
            .tables
            .iter()
            .enumerate()
            .filter(|(_, t)| lower.is_empty() || t.to_lowercase().contains(&lower))
            .map(|(i, _)| i)
            .collect();
        let count = self.filtered.len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        } else if count == 0 {
            self.cursor = 0;
        }
    }

    fn visible(&self) -> Vec<&String> {
        self.filtered
            .iter()
            .filter_map(|&i| self.tables.get(i))
            .collect()
    }

    pub fn selected(&self) -> Option<&String> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.tables.get(i))
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
            format!("{}/{}", self.filtered.len(), self.tables.len())
        };
        let block = Block::default()
            .title(format!(" Tables [{}]{} ", ct, filter_text))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        let items = self.visible();
        let max_w = inner.width.saturating_sub(2) as usize;
        if loading {
            let style = Style::default().fg(theme::color_primary());
            let msg: String = "Loading...".chars().take(max_w).collect();
            buf.set_string(inner.x + 1, inner.y, &msg, style);
            return;
        }
        if items.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            let raw = if self.tables.is_empty() {
                "Connect first (c)"
            } else {
                "No match"
            };
            let msg: String = raw.chars().take(max_w).collect();
            buf.set_string(inner.x + 1, inner.y, &msg, style);
            return;
        }

        let visible = inner.height as usize;
        let offset = if self.cursor >= visible {
            self.cursor - visible + 1
        } else {
            0
        };

        for (i, table) in items.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let is_selected = (i + offset) == self.cursor;

            let style = if is_selected && is_active {
                Style::default()
                    .fg(theme::color_bright())
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if is_selected {
                Style::default()
                    .fg(theme::color_text())
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(theme::color_text())
            };

            let line = format!(" {table}");
            let truncated: String = line.chars().take(max_w).collect();
            buf.set_string(inner.x + 1, y, &truncated, style);
        }
    }
}
