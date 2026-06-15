//! What the grid shows: columns, rows, the active filter and sort. No screen
//! concept lives here. Rows are stored in source order; `filtered` holds the
//! display order as indices back into the source rows.

use super::cell::Cell;
use crate::ui::fuzzy::fuzzy_match;

const MAX_COL_WIDTH: usize = 40;

#[derive(Default)]
pub struct GridModel {
    pub columns: Vec<String>,
    rows: Vec<Vec<Cell>>,
    /// Display order: indices into `rows`.
    filtered: Vec<usize>,
    filter: String,
    sort_col: Option<usize>,
    sort_desc: bool,
}

impl GridModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, columns: Vec<String>, rows: Vec<Vec<Cell>>) {
        self.columns = columns;
        self.rows = rows;
        self.filter.clear();
        self.sort_col = None;
        self.sort_desc = false;
        self.rebuild();
    }

    pub fn clear(&mut self) {
        self.columns.clear();
        self.rows.clear();
        self.filtered.clear();
        self.filter.clear();
        self.sort_col = None;
        self.sort_desc = false;
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn col_count(&self) -> usize {
        self.columns.len()
    }

    /// Number of rows currently visible (after filtering).
    pub fn display_len(&self) -> usize {
        self.filtered.len()
    }

    /// Total number of source rows (ignores the filter).
    pub fn total_rows(&self) -> usize {
        self.rows.len()
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.rebuild();
    }

    pub fn header(&self, col: usize) -> Option<&str> {
        self.columns.get(col).map(|s| s.as_str())
    }

    pub fn sort_state(&self, col: usize) -> Option<bool> {
        if self.sort_col == Some(col) {
            Some(self.sort_desc)
        } else {
            None
        }
    }

    /// Toggle sorting on `col`: unsorted → asc → desc → asc …
    pub fn toggle_sort(&mut self, col: usize) {
        if col >= self.columns.len() {
            return;
        }
        match self.sort_col {
            Some(c) if c == col => self.sort_desc = !self.sort_desc,
            _ => {
                self.sort_col = Some(col);
                self.sort_desc = false;
            }
        }
        self.rebuild();
    }

    /// Translate a display-row index into its source-row index. The single
    /// authority for this mapping (garde-fou 1).
    pub fn display_to_source(&self, display_idx: usize) -> Option<usize> {
        self.filtered.get(display_idx).copied()
    }

    /// Cell at a display position.
    pub fn cell(&self, display_row: usize, col: usize) -> Option<&Cell> {
        let src = self.display_to_source(display_row)?;
        self.rows.get(src).and_then(|r| r.get(col))
    }

    /// Per-column display widths (header vs widest cell), capped.
    pub fn column_widths(&self) -> Vec<usize> {
        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.chars().count()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if let Some(w) = widths.get_mut(i) {
                    *w = (*w).max(cell.render_text().chars().count());
                }
            }
        }
        for w in &mut widths {
            *w = (*w).clamp(1, MAX_COL_WIDTH);
        }
        widths
    }

    fn rebuild(&mut self) {
        // Filter (fuzzy over the joined row text).
        if self.filter.is_empty() {
            self.filtered = (0..self.rows.len()).collect();
        } else {
            let mut scored: Vec<(usize, i32)> = self
                .rows
                .iter()
                .enumerate()
                .filter_map(|(i, row)| {
                    let hay = row
                        .iter()
                        .map(|c| c.sort_key())
                        .collect::<Vec<_>>()
                        .join(" ");
                    fuzzy_match(&hay, &self.filter).map(|s| (i, s))
                })
                .collect();
            scored.sort_by_key(|b| std::cmp::Reverse(b.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }

        // Sort (stable, by the chosen column's text).
        if let Some(col) = self.sort_col {
            let rows = &self.rows;
            let desc = self.sort_desc;
            self.filtered.sort_by(|&a, &b| {
                let ka = rows.get(a).and_then(|r| r.get(col)).map(|c| c.sort_key());
                let kb = rows.get(b).and_then(|r| r.get(col)).map(|c| c.sort_key());
                let ord = natural_cmp(ka.unwrap_or(""), kb.unwrap_or(""));
                if desc {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }
    }
}

/// Compare two cell strings numerically when both parse as numbers, else
/// lexicographically. Keeps `2` before `10` in a numeric column.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(na), Ok(nb)) => na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GridModel {
        let mut m = GridModel::new();
        m.set(
            vec!["id".into(), "name".into()],
            vec![
                vec![Cell::Text("10".into()), Cell::Text("bob".into())],
                vec![Cell::Text("2".into()), Cell::Text("alice".into())],
                vec![Cell::Text("3".into()), Cell::Null],
            ],
        );
        m
    }

    #[test]
    fn display_to_source_identity_unsorted() {
        let m = sample();
        assert_eq!(m.display_len(), 3);
        assert_eq!(m.display_to_source(0), Some(0));
        assert_eq!(m.display_to_source(2), Some(2));
    }

    #[test]
    fn numeric_sort_orders_naturally() {
        let mut m = sample();
        m.toggle_sort(0); // asc on id
                          // 2, 3, 10 — not lexicographic 10, 2, 3.
        assert_eq!(m.cell(0, 0).unwrap().raw(), "2");
        assert_eq!(m.cell(1, 0).unwrap().raw(), "3");
        assert_eq!(m.cell(2, 0).unwrap().raw(), "10");
        m.toggle_sort(0); // desc
        assert_eq!(m.cell(0, 0).unwrap().raw(), "10");
    }

    #[test]
    fn filter_reduces_rows() {
        let mut m = sample();
        m.set_filter("alice");
        assert_eq!(m.display_len(), 1);
        assert_eq!(m.cell(0, 1).unwrap().raw(), "alice");
    }

    #[test]
    fn widths_capped() {
        let mut m = GridModel::new();
        m.set(vec!["c".into()], vec![vec![Cell::Text("x".repeat(200))]]);
        assert_eq!(m.column_widths(), vec![MAX_COL_WIDTH]);
    }
}
