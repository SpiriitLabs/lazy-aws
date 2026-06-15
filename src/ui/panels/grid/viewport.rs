//! How the grid is projected onto the screen: scroll offsets, plus the geometry
//! captured at render time so the mouse handler can hit-test cells.
//!
//! Garde-fou 2: the viewport never reads the model. Column widths and the list
//! of visible columns are *passed in* by `DataGridPanel` at render time.

use std::collections::HashSet;

use ratatui::layout::Rect;

#[derive(Default)]
pub struct GridViewport {
    /// Index of the first visible (non-frozen) column.
    pub scroll_x: usize,
    /// Index of the first visible display row.
    pub scroll_y: usize,
    /// Number of leading columns kept pinned during horizontal scroll.
    pub frozen_cols: usize,
    /// Columns hidden from view (visual only; copy/export still include them).
    hidden: HashSet<usize>,

    // Geometry recorded on the last render (for mouse hit-testing).
    last_inner: Rect,
    last_data_y0: u16,
    last_visible_rows: usize,
    /// (col_idx, x, width) for every column drawn last frame.
    last_col_layout: Vec<(usize, u16, u16)>,
}

impl GridViewport {
    pub fn reset(&mut self) {
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.frozen_cols = 0;
        self.hidden.clear();
    }

    pub fn is_hidden(&self, col: usize) -> bool {
        self.hidden.contains(&col)
    }

    pub fn toggle_hidden(&mut self, col: usize) {
        if !self.hidden.remove(&col) {
            self.hidden.insert(col);
        }
    }

    pub fn unhide_all(&mut self) {
        self.hidden.clear();
    }

    /// Pin the first column (PK) so it stays visible while scrolling; toggles.
    pub fn toggle_freeze(&mut self) {
        self.frozen_cols = if self.frozen_cols == 0 { 1 } else { 0 };
    }

    /// Record the geometry produced by a render pass.
    pub fn record(
        &mut self,
        inner: Rect,
        data_y0: u16,
        visible_rows: usize,
        col_layout: Vec<(usize, u16, u16)>,
    ) {
        self.last_inner = inner;
        self.last_data_y0 = data_y0;
        self.last_visible_rows = visible_rows;
        self.last_col_layout = col_layout;
    }

    /// Keep the cursor cell visible after a move.
    pub fn ensure_visible(&mut self, cur_row: usize, cur_col: usize, row_count: usize) {
        // Vertical
        let h = self.last_visible_rows.max(1);
        if cur_row < self.scroll_y {
            self.scroll_y = cur_row;
        } else if cur_row >= self.scroll_y + h {
            self.scroll_y = cur_row + 1 - h;
        }
        let max_scroll_y = row_count.saturating_sub(h);
        self.scroll_y = self.scroll_y.min(max_scroll_y);

        // Horizontal: only scrollable columns (beyond the frozen prefix) move.
        if cur_col < self.frozen_cols {
            return;
        }
        if cur_col < self.scroll_x {
            self.scroll_x = cur_col;
        } else if let Some(&(_, _, _)) = self.last_col_layout.last() {
            // If the cursor column wasn't drawn last frame, nudge scroll right.
            let drawn = self.last_col_layout.iter().any(|&(c, _, _)| c == cur_col);
            if !drawn {
                self.scroll_x = cur_col;
            }
        }
    }

    /// Hit-test: which `(display_row, col)` is under screen coordinates, if any.
    pub fn cell_at(&self, col_x: u16, row_y: u16) -> Option<(usize, usize)> {
        if row_y < self.last_data_y0 {
            return None;
        }
        let rel = (row_y - self.last_data_y0) as usize;
        if rel >= self.last_visible_rows {
            return None;
        }
        let display_row = self.scroll_y + rel;
        let col = self.column_at(col_x)?;
        Some((display_row, col))
    }

    /// Hit-test the header row: which column index is under `col_x`, if any.
    pub fn header_at(&self, col_x: u16, row_y: u16) -> Option<usize> {
        // Header sits on the row just above the data area.
        if self.last_data_y0 < 2 || row_y != self.last_data_y0 - 2 {
            return None;
        }
        self.column_at(col_x)
    }

    fn column_at(&self, col_x: u16) -> Option<usize> {
        self.last_col_layout
            .iter()
            .find(|&&(_, x, w)| col_x >= x && col_x < x + w)
            .map(|&(c, _, _)| c)
    }
}
