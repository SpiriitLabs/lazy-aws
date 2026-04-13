use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::style::theme;

pub struct LogViewerPanel {
    pub lines: Vec<String>,
    filtered_indices: Vec<usize>, // indices into `lines` that match the filter
    pub filter: String,
    pub cursor: usize, // position in the filtered list
    pub scroll_y: usize,
    pub follow: bool,
    last_visible_height: usize,
}

impl Default for LogViewerPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl LogViewerPanel {
    pub fn new() -> Self {
        LogViewerPanel {
            lines: vec![],
            filtered_indices: vec![],
            filter: String::new(),
            cursor: 0,
            scroll_y: 0,
            follow: true,
            last_visible_height: 20,
        }
    }

    /// Returns the visible lines (filtered or all).
    pub fn visible_lines(&self) -> Vec<&str> {
        if self.filter.is_empty() {
            self.lines.iter().map(|s| s.as_str()).collect()
        } else {
            self.filtered_indices
                .iter()
                .filter_map(|&i| self.lines.get(i).map(|s| s.as_str()))
                .collect()
        }
    }

    fn visible_count(&self) -> usize {
        if self.filter.is_empty() {
            self.lines.len()
        } else {
            self.filtered_indices.len()
        }
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.rebuild_filter();
        self.cursor = 0;
        self.scroll_y = 0;
        if self.filter.is_empty() {
            self.follow = true;
        }
    }

    pub fn clear_filter(&mut self) {
        self.set_filter("");
    }

    fn rebuild_filter(&mut self) {
        let lower = self.filter.to_lowercase();
        self.filtered_indices = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.to_lowercase().contains(&lower))
            .map(|(i, _)| i)
            .collect();
    }

    pub fn append_line(&mut self, line: &str) {
        let idx = self.lines.len();
        self.lines.push(line.to_string());
        // Update filter index if line matches
        if !self.filter.is_empty() && line.to_lowercase().contains(&self.filter.to_lowercase()) {
            self.filtered_indices.push(idx);
        }
        if self.follow {
            let count = self.visible_count();
            self.cursor = count.saturating_sub(1);
            if count > self.last_visible_height {
                self.scroll_y = count - self.last_visible_height;
            }
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.filtered_indices.clear();
        self.cursor = 0;
        self.scroll_y = 0;
    }

    pub fn move_up(&mut self) {
        self.follow = false;
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        if self.cursor < self.scroll_y {
            self.scroll_y = self.cursor;
        }
    }

    pub fn move_down(&mut self) {
        self.follow = false;
        let count = self.visible_count();
        if count > 0 && self.cursor < count - 1 {
            self.cursor += 1;
        }
        if self.cursor >= self.scroll_y + self.last_visible_height {
            self.scroll_y = self.cursor - self.last_visible_height + 1;
        }
        if self.cursor == self.visible_count().saturating_sub(1) {
            self.follow = true;
        }
    }

    pub fn go_to_top(&mut self) {
        self.follow = false;
        self.cursor = 0;
        self.scroll_y = 0;
    }

    pub fn go_to_bottom(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.cursor = count - 1;
            self.follow = true;
            if count > self.last_visible_height {
                self.scroll_y = count - self.last_visible_height;
            }
        }
    }

    pub fn page_up(&mut self) {
        let page = self.last_visible_height.saturating_sub(2);
        self.follow = false;
        self.cursor = self.cursor.saturating_sub(page);
        if self.cursor < self.scroll_y {
            self.scroll_y = self.cursor;
        }
    }

    pub fn page_down(&mut self) {
        let page = self.last_visible_height.saturating_sub(2);
        self.follow = false;
        let count = self.visible_count();
        if count > 0 {
            self.cursor = (self.cursor + page).min(count - 1);
            if self.cursor >= self.scroll_y + self.last_visible_height {
                self.scroll_y = self.cursor - self.last_visible_height + 1;
            }
            if self.cursor == count - 1 {
                self.follow = true;
            }
        }
    }

    /// Returns the currently selected log line.
    pub fn selected_line(&self) -> Option<&str> {
        let lines = self.visible_lines();
        lines.get(self.cursor).copied()
    }

    /// Renders the log list with a cursor highlight.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, is_active: bool) {
        let border_color = if is_active {
            theme::color_border_focus()
        } else {
            theme::color_border()
        };

        let count = self.visible_count();
        let follow_indicator = if self.follow { " follow" } else { "" };
        let filter_indicator = if self.filter.is_empty() {
            String::new()
        } else {
            format!(" filter:\"{}\"", self.filter)
        };
        let title = format!(
            " Logs [{}/{}]{}{} ",
            if count == 0 { 0 } else { self.cursor + 1 },
            count,
            follow_indicator,
            filter_indicator,
        );

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        let visible_h = inner.height as usize;
        let visible_w = inner.width.saturating_sub(1) as usize;
        self.last_visible_height = visible_h;

        let display_lines = self.visible_lines();

        if display_lines.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            let msg = if self.filter.is_empty() {
                "No logs"
            } else {
                "No logs match filter"
            };
            buf.set_string(inner.x + 1, inner.y, msg, style);
            return;
        }

        let offset = if self.follow && display_lines.len() > visible_h {
            display_lines.len() - visible_h
        } else {
            self.scroll_y
        };

        let normal_style = Style::default().fg(theme::color_text());
        let selected_style = Style::default()
            .fg(theme::color_bright())
            .add_modifier(Modifier::BOLD | Modifier::REVERSED);
        let selected_inactive = Style::default()
            .fg(theme::color_text())
            .add_modifier(Modifier::REVERSED);
        let highlight_style = Style::default()
            .fg(theme::color_primary())
            .add_modifier(Modifier::BOLD);
        let arrow_style = Style::default().fg(theme::color_muted());

        for (i, line) in display_lines.iter().skip(offset).enumerate() {
            if i >= visible_h {
                break;
            }
            let y = inner.y + i as u16;
            let line_idx = i + offset;
            let is_selected = line_idx == self.cursor;

            let base_style = if is_selected && is_active {
                selected_style
            } else if is_selected {
                selected_inactive
            } else {
                normal_style
            };

            let truncated: String = line.chars().take(visible_w).collect();

            if is_selected {
                let padded = format!("{:<width$}", truncated, width = visible_w);
                buf.set_string(inner.x + 1, y, &padded, base_style);
            } else if !self.filter.is_empty() {
                // Highlight matching text in non-selected lines
                render_highlighted_line(
                    buf,
                    inner.x + 1,
                    y,
                    &truncated,
                    &self.filter,
                    normal_style,
                    highlight_style,
                    visible_w,
                );
            } else {
                buf.set_string(inner.x + 1, y, &truncated, base_style);
            }

            if line.len() > visible_w && !is_selected {
                let arrow_x = inner.x + inner.width.saturating_sub(1);
                buf.set_string(arrow_x, y, "→", arrow_style);
            }
        }
    }
}

/// Renders a line with filter matches highlighted.
#[allow(clippy::too_many_arguments)]
fn render_highlighted_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    text: &str,
    filter: &str,
    normal: Style,
    highlight: Style,
    max_w: usize,
) {
    let lower_text = text.to_lowercase();
    let lower_filter = filter.to_lowercase();
    let mut col = 0u16;
    let mut pos = 0;

    while pos < text.len() && (col as usize) < max_w {
        if let Some(match_start) = lower_text[pos..].find(&lower_filter) {
            let abs_start = pos + match_start;
            // Render text before match
            if abs_start > pos {
                let before: String = text[pos..abs_start]
                    .chars()
                    .take(max_w - col as usize)
                    .collect();
                buf.set_string(x + col, y, &before, normal);
                col += before.len() as u16;
            }
            // Render match
            let match_end = abs_start + filter.len();
            let matched: String = text[abs_start..match_end.min(text.len())]
                .chars()
                .take(max_w - col as usize)
                .collect();
            buf.set_string(x + col, y, &matched, highlight);
            col += matched.len() as u16;
            pos = match_end;
        } else {
            // No more matches, render rest
            let rest: String = text[pos..].chars().take(max_w - col as usize).collect();
            buf.set_string(x + col, y, &rest, normal);
            break;
        }
    }
}

/// Renders the full content of a selected log line with word-wrap.
/// `@ptr` fields are separated and displayed at the bottom in muted style.
pub fn render_log_detail(line: &str, area: Rect, buf: &mut Buffer, is_active: bool) {
    let border_color = if is_active {
        theme::color_border_focus()
    } else {
        theme::color_border()
    };
    let block = Block::default()
        .title(" Log Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    block.render(area, buf);

    if line.is_empty() {
        let style = Style::default().fg(theme::color_muted());
        buf.set_string(inner.x + 1, inner.y, "Select a log line", style);
        return;
    }

    let visible_w = inner.width.saturating_sub(2) as usize;
    let visible_h = inner.height as usize;

    if visible_w == 0 || visible_h == 0 {
        return;
    }

    // Separate @ptr from the rest of the content
    // Format from Insights: "key=value | key=value | @ptr=value"
    let mut main_parts: Vec<&str> = Vec::new();
    let mut ptr_value: Option<&str> = None;

    for part in line.split(" | ") {
        if let Some(val) = part.strip_prefix("@ptr=") {
            ptr_value = Some(val);
        } else {
            main_parts.push(part);
        }
    }

    let main_content = main_parts.join(" | ");

    // Reserve space for @ptr at the bottom (separator line + ptr line(s))
    let ptr_lines = if let Some(ptr) = ptr_value {
        let ptr_text = format!("@ptr {ptr}");
        let needed = ptr_text.len().div_ceil(visible_w); // ceil div
        needed + 1 // +1 for separator
    } else {
        0
    };
    let main_visible_h = visible_h.saturating_sub(ptr_lines);

    // Render main content with word-wrap
    let style = Style::default().fg(theme::color_bright());
    let mut y = 0usize;

    for text_line in main_content.split('\n') {
        if y >= main_visible_h {
            break;
        }
        if text_line.is_empty() {
            y += 1;
            continue;
        }
        let chars: Vec<char> = text_line.chars().collect();
        for chunk in chars.chunks(visible_w) {
            if y >= main_visible_h {
                break;
            }
            let s: String = chunk.iter().collect();
            buf.set_string(inner.x + 1, inner.y + y as u16, &s, style);
            y += 1;
        }
    }

    // Render @ptr at the bottom
    if let Some(ptr) = ptr_value {
        let ptr_style = Style::default().fg(theme::color_muted());
        let separator_y = inner.y + inner.height.saturating_sub(ptr_lines as u16);

        // Separator line
        let sep: String = "─".repeat(visible_w);
        buf.set_string(inner.x + 1, separator_y, &sep, ptr_style);

        // @ptr value
        let ptr_text = format!("@ptr {ptr}");
        let chars: Vec<char> = ptr_text.chars().collect();
        for (i, chunk) in chars.chunks(visible_w).enumerate() {
            let py = separator_y + 1 + i as u16;
            if py >= inner.y + inner.height {
                break;
            }
            let s: String = chunk.iter().collect();
            buf.set_string(inner.x + 1, py, &s, ptr_style);
        }
    }
}
