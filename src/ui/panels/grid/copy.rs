//! Copy & export strategies for the grid.
//!
//! Every *selection* strategy has the same contract — `fn(&GridModel,
//! &GridSelection) -> String` (garde-fou 4) — so callers just pick one. Export
//! strategies take only the model (no selection involved).

use super::cell::Cell;
use super::model::GridModel;
use super::selection::GridSelection;

/// The single cell under the cursor.
pub fn copy_cell(model: &GridModel, sel: &GridSelection) -> String {
    let (r, c) = sel.cur();
    model
        .cell(r, c)
        .map(|x| x.tsv_value().to_string())
        .unwrap_or_default()
}

/// The selected rectangle as tab-separated rows (DataGrip-style).
/// With no range, this is just the cursor cell.
pub fn copy_selection_tsv(model: &GridModel, sel: &GridSelection) -> String {
    let (r0, r1, c0, c1) = sel.bounds();
    let mut out = String::new();
    for r in r0..=r1 {
        let mut first = true;
        for c in c0..=c1 {
            if !first {
                out.push('\t');
            }
            first = false;
            if let Some(cell) = model.cell(r, c) {
                out.push_str(cell.tsv_value());
            }
        }
        if r < r1 {
            out.push('\n');
        }
    }
    out
}

/// The full row(s) spanned by the selection, all columns, tab-separated.
pub fn copy_rows(model: &GridModel, sel: &GridSelection) -> String {
    let (r0, r1, _, _) = sel.bounds();
    let mut out = String::new();
    for r in r0..=r1 {
        let mut first = true;
        for c in 0..model.col_count() {
            if !first {
                out.push('\t');
            }
            first = false;
            if let Some(cell) = model.cell(r, c) {
                out.push_str(cell.tsv_value());
            }
        }
        if r < r1 {
            out.push('\n');
        }
    }
    out
}

// ---- Export (whole grid) -------------------------------------------------

/// All visible rows as CSV (RFC-4180-ish quoting). NULL → empty unquoted field.
pub fn all_csv(model: &GridModel) -> String {
    let mut out = String::new();
    out.push_str(
        &model
            .columns
            .iter()
            .map(|c| csv_field(c))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for r in 0..model.display_len() {
        let cells: Vec<String> = (0..model.col_count())
            .map(|c| match model.cell(r, c) {
                Some(Cell::Null) | None => String::new(),
                Some(cell) => csv_field(cell.raw()),
            })
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

/// All visible rows as a JSON array of objects. NULL → JSON `null`.
pub fn all_json(model: &GridModel) -> String {
    let mut out = String::from("[\n");
    for r in 0..model.display_len() {
        out.push_str("  {");
        let mut first = true;
        for c in 0..model.col_count() {
            if !first {
                out.push_str(", ");
            }
            first = false;
            let key = json_string(model.header(c).unwrap_or(""));
            let val = match model.cell(r, c) {
                Some(Cell::Null) | None => "null".to_string(),
                Some(cell) => json_string(cell.raw()),
            };
            out.push_str(&format!("{key}: {val}"));
        }
        out.push('}');
        if r + 1 < model.display_len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

fn csv_field(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> GridModel {
        let mut m = GridModel::new();
        m.set(
            vec!["a".into(), "b".into()],
            vec![
                vec![Cell::Text("1".into()), Cell::Text("x".into())],
                vec![Cell::Text("2".into()), Cell::Null],
            ],
        );
        m
    }

    #[test]
    fn selection_tsv_rectangle() {
        let m = model();
        let mut s = GridSelection::new();
        s.set_cursor(0, 0);
        s.extend_to(1, 1);
        assert_eq!(copy_selection_tsv(&m, &s), "1\tx\n2\tNULL");
    }

    #[test]
    fn cell_copies_single_value() {
        let m = model();
        let mut s = GridSelection::new();
        s.set_cursor(0, 1);
        assert_eq!(copy_cell(&m, &s), "x");
    }

    #[test]
    fn csv_has_header_and_empty_null() {
        let m = model();
        let csv = all_csv(&m);
        assert!(csv.starts_with("\"a\",\"b\"\n"));
        // NULL row: second field empty (unquoted).
        assert!(csv.contains("\"2\",\n"));
    }

    #[test]
    fn json_null_literal() {
        let m = model();
        let json = all_json(&m);
        assert!(json.contains("\"b\": null"));
        assert!(json.contains("\"a\": \"1\""));
    }
}
