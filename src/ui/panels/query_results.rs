use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::style::theme;

pub struct QueryResultsPanel {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    filtered: Vec<usize>,
    pub filter: String,
    pub cursor: usize,
    pub scroll_x: usize,
    pub query: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    col_widths: Vec<usize>,
}

impl Default for QueryResultsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryResultsPanel {
    pub fn new() -> Self {
        QueryResultsPanel {
            columns: vec![],
            rows: vec![],
            filtered: vec![],
            filter: String::new(),
            cursor: 0,
            scroll_x: 0,
            query: String::new(),
            error: None,
            duration_ms: 0,
            col_widths: vec![],
        }
    }

    pub fn set_results(
        &mut self,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        query: String,
        duration_ms: u64,
    ) {
        self.error = None;
        self.query = query;
        self.duration_ms = duration_ms;
        self.cursor = 0;
        self.scroll_x = 0;

        // Compute column widths from headers and data
        self.col_widths = columns.iter().map(|c| c.len()).collect();
        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                if i < self.col_widths.len() {
                    self.col_widths[i] = self.col_widths[i].max(cell.len());
                }
            }
        }
        // Cap column widths at 40 chars
        for w in &mut self.col_widths {
            *w = (*w).min(40);
        }

        self.columns = columns;
        self.rows = rows;
        self.filter.clear();
        self.rebuild_filter();
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.columns.clear();
        self.rows.clear();
        self.filtered.clear();
        self.col_widths.clear();
    }

    pub fn clear(&mut self) {
        self.columns.clear();
        self.rows.clear();
        self.filtered.clear();
        self.col_widths.clear();
        self.query.clear();
        self.filter.clear();
        self.error = None;
        self.cursor = 0;
        self.scroll_x = 0;
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
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                lower.is_empty() || row.iter().any(|cell| cell.to_lowercase().contains(&lower))
            })
            .map(|(i, _)| i)
            .collect();
        let count = self.filtered.len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        } else if count == 0 {
            self.cursor = 0;
        }
    }

    fn visible_rows(&self) -> Vec<&Vec<String>> {
        self.filtered
            .iter()
            .filter_map(|&i| self.rows.get(i))
            .collect()
    }

    pub fn selected_line(&self) -> Option<String> {
        let &row_idx = self.filtered.get(self.cursor)?;
        let row = self.rows.get(row_idx)?;
        Some(
            self.columns
                .iter()
                .zip(row.iter())
                .map(|(col, val)| format!("{col}: {val}"))
                .collect::<Vec<_>>()
                .join(" | "),
        )
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

    pub fn scroll_left(&mut self) {
        if self.scroll_x > 0 {
            self.scroll_x -= 1;
        }
    }

    pub fn scroll_right(&mut self) {
        if self.scroll_x < self.columns.len().saturating_sub(1) {
            self.scroll_x += 1;
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
            format!(
                " /{} ({}/{})",
                self.filter,
                self.filtered.len(),
                self.rows.len()
            )
        };
        let title = if !self.query.is_empty() {
            let truncated = if self.query.len() > 50 {
                format!("{}...", &self.query[..50])
            } else {
                self.query.clone()
            };
            format!(" Query: {}{} ", truncated, filter_text)
        } else {
            format!(" Query Results{} ", filter_text)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width < 4 {
            return;
        }

        if loading {
            let style = Style::default().fg(theme::color_primary());
            buf.set_string(inner.x + 1, inner.y, "Executing query...", style);
            return;
        }

        // Show error
        if let Some(ref err) = self.error {
            let style = Style::default().fg(theme::color_danger());
            buf.set_string(inner.x + 1, inner.y, "Error:", style);
            let msg_style = Style::default().fg(theme::color_text());
            let lines: Vec<&str> = err.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if (i as u16 + 1) >= inner.height {
                    break;
                }
                buf.set_string(inner.x + 1, inner.y + 1 + i as u16, line, msg_style);
            }
            return;
        }

        // No results yet
        if self.columns.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            buf.set_string(
                inner.x + 1,
                inner.y,
                "No query results. Press s for SELECT, e for modify.",
                style,
            );
            return;
        }

        let available_width = inner.width as usize;

        // Render header
        let header_style = Style::default()
            .fg(theme::color_primary())
            .add_modifier(Modifier::BOLD);
        let mut x = inner.x;
        for (i, col) in self.columns.iter().enumerate().skip(self.scroll_x) {
            let w = self.col_widths.get(i).copied().unwrap_or(10);
            if (x - inner.x) as usize + w + 1 > available_width {
                break;
            }
            let display = if col.len() > w { &col[..w] } else { col };
            buf.set_string(x + 1, inner.y, display, header_style);
            x += w as u16 + 2; // column + separator gap
        }

        // Separator line
        if inner.height > 1 {
            let sep_style = Style::default().fg(theme::color_muted());
            let sep: String = "\u{2500}".repeat(available_width);
            buf.set_string(inner.x, inner.y + 1, &sep, sep_style);
        }

        // Rows
        let data_start_y = inner.y + 2;
        let max_visible = (inner.height as usize).saturating_sub(3); // header + sep + footer
        let rows = self.visible_rows();
        let offset = if self.cursor >= max_visible {
            self.cursor - max_visible + 1
        } else {
            0
        };

        for (i, row) in rows.iter().skip(offset).enumerate() {
            if i >= max_visible {
                break;
            }
            let y = data_start_y + i as u16;
            let is_selected = (i + offset) == self.cursor;

            let row_style = if is_selected && is_active {
                Style::default()
                    .fg(theme::color_bright())
                    .add_modifier(Modifier::REVERSED)
            } else if is_selected {
                Style::default()
                    .fg(theme::color_text())
                    .add_modifier(Modifier::REVERSED)
            } else if (i + offset) % 2 == 1 {
                Style::default().fg(theme::color_muted())
            } else {
                Style::default().fg(theme::color_text())
            };

            let mut rx = inner.x;
            for (j, cell) in row.iter().enumerate().skip(self.scroll_x) {
                let w = self.col_widths.get(j).copied().unwrap_or(10);
                if (rx - inner.x) as usize + w + 1 > available_width {
                    break;
                }
                let display = if cell.len() > w { &cell[..w] } else { cell };
                buf.set_string(rx + 1, y, display, row_style);
                rx += w as u16 + 2;
            }
        }

        // Footer with row count and duration
        let footer_y = inner.y + inner.height.saturating_sub(1);
        let row_count = if self.filter.is_empty() {
            format!("{}", self.rows.len())
        } else {
            format!("{}/{}", self.filtered.len(), self.rows.len())
        };
        let footer = format!(
            " {} rows ({}.{:03}s) ",
            row_count,
            self.duration_ms / 1000,
            self.duration_ms % 1000
        );
        let footer_style = Style::default().fg(theme::color_muted());
        buf.set_string(inner.x + 1, footer_y, &footer, footer_style);
    }
}
