use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::style::theme;

/// TerminalPanel will embed a VT100 terminal emulator using portable-pty + vt100.
/// This is a stub for Phase 1; full implementation comes in Phase 3.
pub struct TerminalPanel {
    pub active: bool,
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalPanel {
    pub fn new() -> Self {
        TerminalPanel { active: false }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.active {
            theme::color_border_focus()
        } else {
            theme::color_border()
        };
        let block = Block::default()
            .title(" Terminal ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        if !self.active {
            let style = Style::default().fg(theme::color_muted());
            let max_w = inner.width.saturating_sub(2) as usize;
            let msg: String = "Press 'e' to exec into a container"
                .chars()
                .take(max_w)
                .collect();
            buf.set_string(inner.x + 1, inner.y, &msg, style);
        }
    }
}
