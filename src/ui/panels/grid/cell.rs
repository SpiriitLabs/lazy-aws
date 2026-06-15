//! A single grid value.
//!
//! `Cell::Null` is deliberately distinct from `Cell::Text(String::new())` so the
//! grid can show `<null>` differently from an empty string and copy them
//! differently. This type is the *foundation* of the grid: model, selection,
//! rendering, copy and export are all built on `Cell`, never on raw `String`.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    Null,
    Text(String),
}

impl Cell {
    /// Build a cell from one field of `mysql --batch` output.
    ///
    /// Without `--raw`, the mysql client escapes a SQL NULL as the two-byte
    /// sequence `\N`, and escapes special characters inside values (`\t`, `\n`,
    /// `\\`, `\0`). A literal text "NULL" arrives as `NULL`, and a literal
    /// backslash-N arrives as `\\N`, so only an exact `\N` is a SQL NULL.
    pub fn from_batch(raw: &str) -> Self {
        if raw == "\\N" {
            Cell::Null
        } else {
            Cell::Text(unescape_batch(raw))
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Cell::Null)
    }

    /// Text used for in-grid display (NULL is shown as `<null>`).
    pub fn render_text(&self) -> &str {
        match self {
            Cell::Null => "<null>",
            Cell::Text(s) => s,
        }
    }

    /// Text used for fuzzy filtering / sorting comparisons.
    pub fn sort_key(&self) -> &str {
        match self {
            Cell::Null => "",
            Cell::Text(s) => s,
        }
    }

    /// Value for tab-separated copy: NULL becomes the literal `NULL`, an empty
    /// string stays empty — so the two are distinguishable on paste.
    pub fn tsv_value(&self) -> &str {
        match self {
            Cell::Null => "NULL",
            Cell::Text(s) => s,
        }
    }

    /// Raw text for CSV/JSON export. NULL maps to an empty string (callers that
    /// need the JSON `null` literal check [`Cell::is_null`] first).
    pub fn raw(&self) -> &str {
        match self {
            Cell::Null => "",
            Cell::Text(s) => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_null_is_distinct_from_text() {
        assert_eq!(Cell::from_batch("\\N"), Cell::Null);
        assert_eq!(Cell::from_batch("NULL"), Cell::Text("NULL".to_string()));
        assert_eq!(Cell::from_batch(""), Cell::Text(String::new()));
    }

    #[test]
    fn batch_unescapes_specials() {
        assert_eq!(Cell::from_batch("a\\tb").render_text(), "a\tb");
        assert_eq!(
            Cell::from_batch("line1\\nline2").render_text(),
            "line1\nline2"
        );
        // A literal backslash-N (escaped as \\N) is text, not NULL.
        assert_eq!(Cell::from_batch("\\\\N"), Cell::Text("\\N".to_string()));
    }

    #[test]
    fn tsv_distinguishes_null_from_empty() {
        assert_eq!(Cell::Null.tsv_value(), "NULL");
        assert_eq!(Cell::Text(String::new()).tsv_value(), "");
    }
}

/// Reverse the escaping `mysql --batch` applies to field values.
fn unescape_batch(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
