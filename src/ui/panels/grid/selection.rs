//! What the user has selected in the grid.
//!
//! Coordinates are `(display_row, col)` in **display space** (indices into the
//! model's filtered/sorted view), never source-row indices. Only [`GridModel`]
//! translates display → source. This struct is fed identically by the keyboard
//! (Shift+arrows) and the mouse (drag); neither duplicates the range logic.
//!
//! [`GridModel`]: super::model::GridModel

#[derive(Default)]
pub struct GridSelection {
    /// The active cell (cursor).
    cur: (usize, usize),
    /// Anchor of a range selection, if one is in progress.
    anchor: Option<(usize, usize)>,
}

impl GridSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cur(&self) -> (usize, usize) {
        self.cur
    }

    pub fn cur_row(&self) -> usize {
        self.cur.0
    }

    pub fn cur_col(&self) -> usize {
        self.cur.1
    }

    pub fn has_range(&self) -> bool {
        self.anchor.is_some()
    }

    /// Move the cursor to an absolute cell, dropping any range selection.
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.cur = (row, col);
        self.anchor = None;
    }

    /// Start (or continue) a range from the current cursor to `(row, col)`.
    /// Used by Shift+arrows and by mouse drag.
    pub fn extend_to(&mut self, row: usize, col: usize) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cur);
        }
        self.cur = (row, col);
    }

    /// Begin a fresh range anchored at `(row, col)` (mouse left-button down).
    pub fn begin(&mut self, row: usize, col: usize) {
        self.cur = (row, col);
        self.anchor = Some((row, col));
    }

    pub fn clear_range(&mut self) {
        self.anchor = None;
    }

    /// Inclusive bounds of the selection as `(row0, row1, col0, col1)`.
    /// With no range, this is the single cursor cell.
    pub fn bounds(&self) -> (usize, usize, usize, usize) {
        match self.anchor {
            None => (self.cur.0, self.cur.0, self.cur.1, self.cur.1),
            Some((ar, ac)) => (
                ar.min(self.cur.0),
                ar.max(self.cur.0),
                ac.min(self.cur.1),
                ac.max(self.cur.1),
            ),
        }
    }

    pub fn contains(&self, row: usize, col: usize) -> bool {
        let (r0, r1, c0, c1) = self.bounds();
        row >= r0 && row <= r1 && col >= c0 && col <= c1
    }

    /// Called when the model re-indexes (sort/filter changed): the safe choice
    /// is to drop the range and clamp the cursor so a sort never silently drags
    /// a stale selection. (See garde-fou 1 in the plan.)
    pub fn on_reindex(&mut self, row_count: usize, col_count: usize) {
        self.anchor = None;
        let max_row = row_count.saturating_sub(1);
        let max_col = col_count.saturating_sub(1);
        self.cur = (self.cur.0.min(max_row), self.cur.1.min(max_col));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_cell_bounds() {
        let mut s = GridSelection::new();
        s.set_cursor(3, 2);
        assert_eq!(s.bounds(), (3, 3, 2, 2));
        assert!(!s.has_range());
    }

    #[test]
    fn extend_creates_normalized_range() {
        let mut s = GridSelection::new();
        s.set_cursor(5, 5);
        s.extend_to(2, 1);
        assert!(s.has_range());
        assert_eq!(s.bounds(), (2, 5, 1, 5));
        assert!(s.contains(3, 3));
        assert!(!s.contains(6, 3));
    }

    #[test]
    fn begin_anchors_a_range() {
        let mut s = GridSelection::new();
        s.begin(1, 1);
        s.extend_to(4, 3);
        assert_eq!(s.bounds(), (1, 4, 1, 3));
    }

    #[test]
    fn on_reindex_drops_range_and_clamps() {
        let mut s = GridSelection::new();
        s.set_cursor(2, 2);
        s.extend_to(9, 9); // cursor now at (9,9) with a range
        assert!(s.has_range());
        s.on_reindex(5, 4); // 5 rows, 4 cols → max (4,3)
        assert!(!s.has_range());
        assert_eq!(s.cur(), (4, 3));
    }
}
