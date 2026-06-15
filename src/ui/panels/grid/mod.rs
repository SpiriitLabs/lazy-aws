//! Spreadsheet-style results grid (DataGrip-inspired).
//!
//! `DataGridPanel` is a thin orchestrator composing three collaborators, each
//! with a single responsibility:
//!   - [`GridModel`]      — what to show (columns, rows, filter, sort)
//!   - [`GridSelection`]  — what the user picked (cursor cell + range)
//!   - [`GridViewport`]   — how to project it (scroll + hit-test geometry)
//!
//! Copy/export live in [`copy`]. The cell type [`Cell`] is the shared foundation.

mod cell;
pub mod copy;
mod model;
mod selection;
mod viewport;

pub use cell::Cell;

use model::GridModel;
use selection::GridSelection;
use viewport::GridViewport;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::ui::style::{styles, theme};
use crate::ui::text::truncate_chars;

/// Modal viewer for a single (possibly large) cell value.
struct ValueView {
    column: String,
    lines: Vec<String>,
    scroll: usize,
    is_null: bool,
}

#[derive(Default)]
pub struct DataGridPanel {
    model: GridModel,
    selection: GridSelection,
    viewport: GridViewport,
    query: String,
    error: Option<String>,
    duration_ms: u64,
    value_view: Option<ValueView>,
    /// Visual mode: plain cursor moves extend the range instead of resetting it.
    visual: bool,
}

impl DataGridPanel {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- data in ---------------------------------------------------------

    /// Populate the grid from raw `mysql --batch` rows (each field a `String`).
    /// Conversion to [`Cell`] happens here (`\N` → `Cell::Null`).
    pub fn set_results(
        &mut self,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        query: String,
        duration_ms: u64,
    ) {
        let cells: Vec<Vec<Cell>> = rows
            .into_iter()
            .map(|row| row.iter().map(|s| Cell::from_batch(s)).collect())
            .collect();
        self.model.set(columns, cells);
        self.selection = GridSelection::new();
        self.viewport.reset();
        self.visual = false;
        self.query = query;
        self.duration_ms = duration_ms;
        self.error = None;
        self.value_view = None;
    }

    pub fn set_error(&mut self, error: String) {
        self.model.clear();
        self.selection = GridSelection::new();
        self.viewport.reset();
        self.visual = false;
        self.error = Some(error);
        self.value_view = None;
    }

    pub fn clear(&mut self) {
        self.model.clear();
        self.selection = GridSelection::new();
        self.viewport.reset();
        self.visual = false;
        self.query.clear();
        self.error = None;
        self.value_view = None;
    }

    pub fn is_empty(&self) -> bool {
        self.model.is_empty()
    }

    pub fn total_rows(&self) -> usize {
        self.model.total_rows()
    }

    // ---- filter ----------------------------------------------------------

    pub fn filter(&self) -> &str {
        self.model.filter()
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.model.set_filter(filter);
        self.selection
            .on_reindex(self.model.display_len(), self.model.col_count());
    }

    pub fn clear_filter(&mut self) {
        self.set_filter("");
    }

    // ---- navigation (keyboard) ------------------------------------------

    /// Move the cursor to `(row, col)`, extending the range in visual mode.
    fn go(&mut self, row: usize, col: usize) {
        if self.visual {
            self.selection.extend_to(row, col);
        } else {
            self.selection.set_cursor(row, col);
        }
        self.after_move();
    }

    pub fn move_up(&mut self) {
        let (r, c) = self.selection.cur();
        self.go(r.saturating_sub(1), c);
    }

    pub fn move_down(&mut self) {
        let (r, c) = self.selection.cur();
        let max = self.model.display_len().saturating_sub(1);
        self.go((r + 1).min(max), c);
    }

    pub fn move_left(&mut self) {
        let (r, c) = self.selection.cur();
        self.go(r, c.saturating_sub(1));
    }

    pub fn move_right(&mut self) {
        let (r, c) = self.selection.cur();
        let max = self.model.col_count().saturating_sub(1);
        self.go(r, (c + 1).min(max));
    }

    pub fn go_top(&mut self) {
        let (_, c) = self.selection.cur();
        self.go(0, c);
    }

    pub fn go_bottom(&mut self) {
        let (_, c) = self.selection.cur();
        let max = self.model.display_len().saturating_sub(1);
        self.go(max, c);
    }

    /// Toggle visual (range) mode. Turning it on anchors a range at the cursor.
    pub fn toggle_visual(&mut self) {
        self.visual = !self.visual;
        if self.visual {
            let (r, c) = self.selection.cur();
            self.selection.begin(r, c);
        } else {
            self.selection.clear_range();
        }
    }

    /// Extend the range with Shift+arrow style moves.
    pub fn extend_up(&mut self) {
        let (r, c) = self.selection.cur();
        self.selection.extend_to(r.saturating_sub(1), c);
        self.after_move();
    }

    pub fn extend_down(&mut self) {
        let (r, c) = self.selection.cur();
        let max = self.model.display_len().saturating_sub(1);
        self.selection.extend_to((r + 1).min(max), c);
        self.after_move();
    }

    pub fn extend_left(&mut self) {
        let (r, c) = self.selection.cur();
        self.selection.extend_to(r, c.saturating_sub(1));
        self.after_move();
    }

    pub fn extend_right(&mut self) {
        let (r, c) = self.selection.cur();
        let max = self.model.col_count().saturating_sub(1);
        self.selection.extend_to(r, (c + 1).min(max));
        self.after_move();
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear_range();
        self.visual = false;
    }

    fn after_move(&mut self) {
        let (r, c) = self.selection.cur();
        self.viewport.ensure_visible(r, c, self.model.display_len());
    }

    // ---- sort ------------------------------------------------------------

    /// Sort by the column under the cursor (toggles asc/desc).
    pub fn sort_current_column(&mut self) {
        let (_, c) = self.selection.cur();
        self.model.toggle_sort(c);
        self.selection
            .on_reindex(self.model.display_len(), self.model.col_count());
    }

    /// Hide the column under the cursor (visual only).
    pub fn hide_current_column(&mut self) {
        let (_, c) = self.selection.cur();
        self.viewport.toggle_hidden(c);
    }

    pub fn unhide_all_columns(&mut self) {
        self.viewport.unhide_all();
    }

    /// Pin/unpin the first column so it stays visible while scrolling.
    pub fn toggle_freeze_first_column(&mut self) {
        self.viewport.toggle_freeze();
    }

    // ---- copy ------------------------------------------------------------

    /// Text to copy on `y`: the selection (TSV) if a range is active, else the
    /// single cell under the cursor.
    pub fn copy_text(&self) -> String {
        if self.selection.has_range() {
            copy::copy_selection_tsv(&self.model, &self.selection)
        } else {
            copy::copy_cell(&self.model, &self.selection)
        }
    }

    /// Whole row(s) spanned by the selection (for `Y`).
    pub fn copy_rows_text(&self) -> String {
        copy::copy_rows(&self.model, &self.selection)
    }

    pub fn to_csv(&self) -> String {
        copy::all_csv(&self.model)
    }

    pub fn to_json(&self) -> String {
        copy::all_json(&self.model)
    }

    /// Whole current row as `col: value | …` (used by the global yank fallback).
    pub fn selected_line(&self) -> Option<String> {
        if self.model.is_empty() {
            return None;
        }
        let (r, _) = self.selection.cur();
        let parts: Vec<String> = (0..self.model.col_count())
            .map(|c| {
                let col = self.model.header(c).unwrap_or("");
                let val = self.model.cell(r, c).map(|x| x.render_text()).unwrap_or("");
                format!("{col}: {val}")
            })
            .collect();
        Some(parts.join(" | "))
    }

    // ---- value viewer ----------------------------------------------------

    pub fn open_value_viewer(&mut self) {
        let (r, c) = self.selection.cur();
        let Some(cell) = self.model.cell(r, c) else {
            return;
        };
        let column = self.model.header(c).unwrap_or("").to_string();
        let is_null = cell.is_null();
        let lines: Vec<String> = cell
            .render_text()
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        self.value_view = Some(ValueView {
            column,
            lines,
            scroll: 0,
            is_null,
        });
    }

    pub fn value_viewer_active(&self) -> bool {
        self.value_view.is_some()
    }

    pub fn close_value_viewer(&mut self) {
        self.value_view = None;
    }

    pub fn value_scroll_up(&mut self) {
        if let Some(v) = &mut self.value_view {
            v.scroll = v.scroll.saturating_sub(1);
        }
    }

    pub fn value_scroll_down(&mut self) {
        if let Some(v) = &mut self.value_view {
            if v.scroll + 1 < v.lines.len() {
                v.scroll += 1;
            }
        }
    }

    /// The raw current cell value (used to copy from inside the value viewer).
    pub fn current_value_text(&self) -> Option<String> {
        let (r, c) = self.selection.cur();
        self.model.cell(r, c).map(|x| x.render_text().to_string())
    }

    // ---- mouse hit-test (delegated to viewport; garde-fou: app never reads geometry) ----

    pub fn cell_at(&self, col_x: u16, row_y: u16) -> Option<(usize, usize)> {
        self.viewport.cell_at(col_x, row_y)
    }

    pub fn header_at(&self, col_x: u16, row_y: u16) -> Option<usize> {
        self.viewport.header_at(col_x, row_y)
    }

    pub fn set_cursor_cell(&mut self, row: usize, col: usize) {
        self.selection.set_cursor(row, col);
        self.after_move();
    }

    pub fn begin_selection(&mut self, row: usize, col: usize) {
        self.selection.begin(row, col);
        self.after_move();
    }

    pub fn drag_selection(&mut self, row: usize, col: usize) {
        self.selection.extend_to(row, col);
        self.after_move();
    }

    pub fn sort_column(&mut self, col: usize) {
        self.model.toggle_sort(col);
        self.selection
            .on_reindex(self.model.display_len(), self.model.col_count());
    }

    // ---- render ----------------------------------------------------------

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, is_active: bool, loading: bool) {
        let border_color = if is_active {
            theme::color_border_focus()
        } else {
            theme::color_border()
        };

        let filter_text = if self.model.filter().is_empty() {
            String::new()
        } else {
            format!(
                " /{} ({}/{})",
                self.model.filter(),
                self.model.display_len(),
                self.model.total_rows()
            )
        };
        let title = if self.query.is_empty() {
            format!(" Results{filter_text} ")
        } else {
            format!(" {}{} ", truncate_chars(&self.query, 50), filter_text)
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
            buf.set_string(
                inner.x + 1,
                inner.y,
                "Executing query...",
                Style::default().fg(theme::color_primary()),
            );
            return;
        }

        if let Some(err) = &self.error {
            buf.set_string(
                inner.x + 1,
                inner.y,
                "Error:",
                Style::default().fg(theme::color_danger()),
            );
            let msg_style = Style::default().fg(theme::color_text());
            for (i, line) in err.lines().enumerate() {
                if (i as u16 + 1) >= inner.height {
                    break;
                }
                buf.set_string(inner.x + 1, inner.y + 1 + i as u16, line, msg_style);
            }
            return;
        }

        if self.model.is_empty() {
            buf.set_string(
                inner.x + 1,
                inner.y,
                "No results. Press s for SELECT, e to modify.",
                Style::default().fg(theme::color_muted()),
            );
            return;
        }

        self.render_grid(inner, buf, is_active);

        if self.value_view.is_some() {
            self.render_value_viewer(inner, buf);
        }
    }

    fn render_grid(&mut self, inner: Rect, buf: &mut Buffer, is_active: bool) {
        let widths = self.model.column_widths();
        let available = inner.width as usize;

        // Decide the order of visible columns: frozen prefix (pinned), then the
        // scrollable region, skipping hidden columns.
        let frozen = self.viewport.frozen_cols;
        let scroll_x = self.viewport.scroll_x.max(frozen);
        let mut order: Vec<usize> = Vec::new();
        for c in 0..frozen.min(self.model.col_count()) {
            if !self.viewport.is_hidden(c) {
                order.push(c);
            }
        }
        for c in scroll_x..self.model.col_count() {
            if c >= frozen && !self.viewport.is_hidden(c) {
                order.push(c);
            }
        }

        // Assign x offsets (capture geometry for mouse hit-testing).
        let mut col_layout: Vec<(usize, u16, u16)> = Vec::new();
        let mut x = inner.x;
        for &c in &order {
            let w = widths.get(c).copied().unwrap_or(10);
            if (x - inner.x) as usize + w + 1 > available {
                break;
            }
            col_layout.push((c, x + 1, w as u16));
            x += w as u16 + 2;
        }

        // Header
        let header_style = Style::default()
            .fg(theme::color_primary())
            .add_modifier(Modifier::BOLD);
        for &(c, cx, w) in &col_layout {
            let name = self.model.header(c).unwrap_or("");
            let marker = match self.model.sort_state(c) {
                Some(true) => " ▼",
                Some(false) => " ▲",
                None => "",
            };
            let text = truncate_chars(&format!("{name}{marker}"), w as usize);
            buf.set_string(cx, inner.y, text, header_style);
        }

        // Separator
        if inner.height > 1 {
            let sep: String = "\u{2500}".repeat(available);
            buf.set_string(
                inner.x,
                inner.y + 1,
                &sep,
                Style::default().fg(theme::color_muted()),
            );
        }

        let data_y0 = inner.y + 2;
        let max_visible = (inner.height as usize).saturating_sub(3);
        let (cur_row, _cur_col) = self.selection.cur();
        let scroll_y = self.viewport.scroll_y;

        for vis in 0..max_visible {
            let display_row = scroll_y + vis;
            if display_row >= self.model.display_len() {
                break;
            }
            let y = data_y0 + vis as u16;
            for &(c, cx, w) in &col_layout {
                let cell = self.model.cell(display_row, c);
                let is_cursor = display_row == cur_row && c == self.selection.cur_col();
                let in_range = self.selection.contains(display_row, c);
                let is_null = cell.map(|x| x.is_null()).unwrap_or(false);

                let style = if is_cursor {
                    styles::selection_style(is_active)
                } else if in_range {
                    Style::default()
                        .fg(theme::color_bright())
                        .bg(theme::color_secondary())
                } else if is_null {
                    Style::default()
                        .fg(theme::color_muted())
                        .add_modifier(Modifier::ITALIC)
                } else if display_row % 2 == 1 {
                    Style::default().fg(theme::color_muted())
                } else {
                    Style::default().fg(theme::color_text())
                };

                let raw = cell.map(|x| x.render_text()).unwrap_or("");
                let text = truncate_chars(raw, w as usize);
                // Pad to the column width so the highlight covers the whole cell.
                let padded = format!("{text:<width$}", width = w as usize);
                let padded = truncate_chars(&padded, w as usize);
                buf.set_string(cx, y, padded, style);
            }
        }

        self.viewport
            .record(inner, data_y0, max_visible, col_layout);

        // Footer
        let footer_y = inner.y + inner.height.saturating_sub(1);
        let (r0, r1, c0, c1) = self.selection.bounds();
        let sel_info = if self.selection.has_range() {
            format!(" · sel {}×{}", r1 - r0 + 1, c1 - c0 + 1)
        } else {
            String::new()
        };
        let footer = format!(
            " {} rows ({}.{:03}s) · cell ({},{}){} ",
            self.model.display_len(),
            self.duration_ms / 1000,
            self.duration_ms % 1000,
            cur_row + 1,
            self.selection.cur_col() + 1,
            sel_info
        );
        buf.set_string(
            inner.x + 1,
            footer_y,
            truncate_chars(&footer, available.saturating_sub(1)),
            Style::default().fg(theme::color_muted()),
        );
    }

    fn render_value_viewer(&self, inner: Rect, buf: &mut Buffer) {
        let Some(v) = &self.value_view else {
            return;
        };
        // Centered popup covering most of the grid area.
        let w = inner.width.saturating_sub(4).max(10);
        let h = inner.height.saturating_sub(4).max(3);
        let x = inner.x + (inner.width - w) / 2;
        let y = inner.y + (inner.height - h) / 2;
        let area = Rect {
            x,
            y,
            width: w,
            height: h,
        };
        Clear.render(area, buf);

        let title = if v.is_null {
            format!(" {} = <null> ", v.column)
        } else {
            format!(" {} ", v.column)
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::color_border_focus()));
        let vinner = block.inner(area);
        block.render(area, buf);

        let body_style = Style::default().fg(theme::color_text());
        let rows = vinner.height as usize;
        for (i, line) in v.lines.iter().skip(v.scroll).take(rows).enumerate() {
            let text = truncate_chars(line, vinner.width as usize);
            buf.set_string(vinner.x, vinner.y + i as u16, text, body_style);
        }
    }
}
