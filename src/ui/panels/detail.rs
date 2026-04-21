use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::style::theme;

pub struct DetailPanel {
    pub lines: Vec<String>,
    pub scroll: u16,
}

impl Default for DetailPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DetailPanel {
    pub fn new() -> Self {
        DetailPanel {
            lines: vec![],
            scroll: 0,
        }
    }

    pub fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = lines;
        self.scroll = 0;
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, visible: u16) {
        let max = (self.lines.len() as u16).saturating_sub(visible);
        if self.scroll < max {
            self.scroll += 1;
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, _is_active: bool) {
        let inner_h = area.height.saturating_sub(2); // borders
        let total = self.lines.len();
        let has_more_below = (self.scroll as usize + inner_h as usize) < total;
        let has_more_above = self.scroll > 0;

        let scroll_info = if total > inner_h as usize {
            format!(" {}/{} ", self.scroll + 1, total)
        } else {
            String::new()
        };

        let block = Block::default()
            .title(format!(" Detail{} ", scroll_info))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::color_border()));
        let inner = block.inner(area);
        block.render(area, buf);

        // Keep one column of right padding so text never hits the right border.
        let max_w = inner.width.saturating_sub(2) as usize;

        if self.lines.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            let placeholder: String = "Select an item".chars().take(max_w).collect();
            buf.set_string(inner.x + 1, inner.y, &placeholder, style);
            return;
        }

        let visible = inner.height as usize;
        let offset = self.scroll as usize;

        for (i, line) in self.lines.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let style = Style::default().fg(theme::color_text());
            let truncated: String = line.chars().take(max_w).collect();
            buf.set_string(inner.x + 1, y, &truncated, style);
        }

        // Scroll indicators
        let indicator_style = Style::default().fg(theme::color_primary());
        if has_more_above {
            let x = inner.x + inner.width.saturating_sub(6);
            buf.set_string(x, inner.y, "^ more", indicator_style);
        }
        if has_more_below {
            let y = inner.y + inner.height.saturating_sub(1);
            let x = inner.x + inner.width.saturating_sub(6);
            buf.set_string(x, y, "v more", indicator_style);
        }
    }
}
