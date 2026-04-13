use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::style::theme;

pub struct OutputPanel {
    pub lines: Vec<String>,
}

impl Default for OutputPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputPanel {
    pub fn new() -> Self {
        OutputPanel { lines: vec![] }
    }

    pub fn append_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Output ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::color_border()));
        let inner = block.inner(area);
        block.render(area, buf);

        let visible = inner.height as usize;
        let offset = if self.lines.len() > visible {
            self.lines.len() - visible
        } else {
            0
        };

        for (i, line) in self.lines.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let style = Style::default().fg(theme::color_text());
            buf.set_string(inner.x + 1, y, line, style);
        }
    }
}
