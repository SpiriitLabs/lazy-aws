//! Multi-line SQL console (DataGrip-style). Owns text editing via a shared
//! [`TextBuffer`]; it never references the results grid — it only produces SQL
//! text and lets `app.rs` run it (Hollywood principle, garde-fou 3).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use ratatui::widgets::{Block, Borders, Clear};

use crate::ui::style::theme;
use crate::ui::text::truncate_chars;
use crate::ui::text_buffer::TextBuffer;

/// Completion popup state (Tab/↑↓ navigate, Enter accepts).
struct Completion {
    /// Column where the typed prefix starts (the word is `anchor..cursor`).
    anchor: usize,
    matches: Vec<String>,
    idx: usize,
}

pub struct SqlEditor {
    buf: TextBuffer,
    scroll_y: usize,
    completion: Option<Completion>,
}

impl Default for SqlEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlEditor {
    pub fn new() -> Self {
        SqlEditor {
            buf: TextBuffer::new(true),
            scroll_y: 0,
            completion: None,
        }
    }

    pub fn set_text(&mut self, s: &str) {
        self.buf.set_text(s);
        self.scroll_y = 0;
        self.completion = None;
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.scroll_y = 0;
        self.completion = None;
    }

    pub fn text(&self) -> String {
        self.buf.text()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn completion_active(&self) -> bool {
        self.completion.is_some()
    }

    /// Inspect the word under the cursor for completion.
    ///
    /// Returns `(anchor, qualifier, prefix)` where `anchor` is the column where
    /// the partial word starts, `qualifier` is the identifier before a `.`
    /// (e.g. the `o` in `o.cust`), and `prefix` is the partial word typed so
    /// far. Schema-aware candidate building lives in `app.rs`, which then calls
    /// [`SqlEditor::open_completion`].
    pub fn completion_context(&self) -> (usize, Option<String>, String) {
        let (line, col) = self.buf.cursor();
        let chars: Vec<char> = self.buf.line(line).map(|c| c.to_vec()).unwrap_or_default();
        let mut anchor = col;
        while anchor > 0 && is_word_char(chars[anchor - 1]) {
            anchor -= 1;
        }
        let prefix: String = chars[anchor..col].iter().collect();

        // A qualifier is the identifier immediately before a '.' just before
        // the partial word (e.g. the `o` in `o.cust`).
        let mut qualifier = None;
        if anchor > 0 && chars[anchor - 1] == '.' {
            let mut qstart = anchor - 1;
            while qstart > 0 && is_word_char(chars[qstart - 1]) {
                qstart -= 1;
            }
            let q: String = chars[qstart..anchor - 1].iter().collect();
            if !q.is_empty() {
                qualifier = Some(q);
            }
        }
        (anchor, qualifier, prefix)
    }

    /// Open the completion popup with `matches`, replacing `anchor..cursor` on
    /// accept. No-op if `matches` is empty.
    pub fn open_completion(&mut self, anchor: usize, matches: Vec<String>) {
        if !matches.is_empty() {
            self.completion = Some(Completion {
                anchor,
                matches,
                idx: 0,
            });
        }
    }

    pub fn completion_next(&mut self) {
        if let Some(c) = &mut self.completion {
            if !c.matches.is_empty() {
                c.idx = (c.idx + 1) % c.matches.len();
            }
        }
    }

    pub fn completion_prev(&mut self) {
        if let Some(c) = &mut self.completion {
            if !c.matches.is_empty() {
                c.idx = (c.idx + c.matches.len() - 1) % c.matches.len();
            }
        }
    }

    pub fn cancel_completion(&mut self) {
        self.completion = None;
    }

    /// Insert the highlighted completion, replacing the typed prefix.
    pub fn accept_completion(&mut self) {
        let Some(comp) = self.completion.take() else {
            return;
        };
        let (_, col) = self.buf.cursor();
        let word_len = col.saturating_sub(comp.anchor);
        for _ in 0..word_len {
            self.buf.backspace();
        }
        for ch in comp.matches[comp.idx].chars() {
            self.buf.insert_char(ch);
        }
    }

    /// Handle a text-editing key. Returns `true` if it was consumed. Control
    /// combos (execute, save, open) are left for `app.rs` to interpret.
    pub fn handle_edit_key(&mut self, key: KeyEvent) -> bool {
        // Any edit/navigation invalidates the completion cycle.
        self.completion = None;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('h' | 'w') if ctrl => self.buf.delete_prev_word(),
            KeyCode::Char(c) if !ctrl => self.buf.insert_char(c),
            KeyCode::Enter if !ctrl => self.buf.newline(),
            KeyCode::Backspace if ctrl => self.buf.delete_prev_word(),
            KeyCode::Backspace => self.buf.backspace(),
            KeyCode::Delete if ctrl => self.buf.delete_next_word(),
            KeyCode::Delete => self.buf.delete(),
            KeyCode::Left if ctrl => self.buf.word_left(),
            KeyCode::Left => self.buf.move_left(),
            KeyCode::Right if ctrl => self.buf.word_right(),
            KeyCode::Right => self.buf.move_right(),
            KeyCode::Up => self.buf.move_up(),
            KeyCode::Down => self.buf.move_down(),
            KeyCode::Home => self.buf.home(),
            KeyCode::End => self.buf.end(),
            _ => return false,
        }
        true
    }

    /// The `;`-delimited statement that the cursor currently sits in.
    pub fn statement_at_cursor(&self) -> Option<String> {
        let text = self.buf.text();
        let offset = self.cursor_offset();
        statement_at(&text, offset)
    }

    fn cursor_offset(&self) -> usize {
        let (line, col) = self.buf.cursor();
        let mut offset = 0;
        for l in 0..line {
            offset += self.buf.line(l).map(|c| c.len()).unwrap_or(0) + 1; // +1 for '\n'
        }
        offset + col
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, is_active: bool) {
        let border_color = if is_active {
            theme::color_border_focus()
        } else {
            theme::color_border()
        };
        let hint = if is_active {
            " Console — F5/Alt+↵/Ctrl+R run · Tab complete · Ctrl+s/o save/open · Esc leave "
        } else {
            " Console "
        };
        let block = Block::default()
            .title(hint)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height == 0 || inner.width < 2 {
            return;
        }

        let (cur_line, cur_col) = self.buf.cursor();
        let height = inner.height as usize;
        // Keep the cursor line visible.
        if cur_line < self.scroll_y {
            self.scroll_y = cur_line;
        } else if cur_line >= self.scroll_y + height {
            self.scroll_y = cur_line + 1 - height;
        }

        let max_w = inner.width as usize;
        for vis in 0..height {
            let li = self.scroll_y + vis;
            let Some(chars) = self.buf.line(li) else {
                break;
            };
            let line: String = chars.iter().collect();
            let y = inner.y + vis as u16;

            // Render syntax-highlighted segments left to right.
            let mut x = inner.x;
            let mut drawn = 0usize;
            for (text, style) in highlight_line(&line) {
                if drawn >= max_w {
                    break;
                }
                let chunk = truncate_chars(&text, max_w - drawn);
                let chunk_w = chunk.chars().count();
                buf.set_string(x, y, &chunk, style);
                x += chunk_w as u16;
                drawn += chunk_w;
            }

            // Draw the cursor as a reversed cell.
            if is_active && li == cur_line {
                let cx = inner.x + (cur_col.min(inner.width as usize - 1)) as u16;
                let under = chars.get(cur_col).copied().unwrap_or(' ');
                buf.set_string(
                    cx,
                    y,
                    under.to_string(),
                    Style::default()
                        .fg(theme::color_background())
                        .bg(theme::color_primary())
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // Completion popup, anchored just below the cursor.
        if is_active {
            self.render_completion_popup(inner, buf, cur_line, cur_col);
        }
    }

    fn render_completion_popup(
        &self,
        inner: Rect,
        buf: &mut Buffer,
        cur_line: usize,
        cur_col: usize,
    ) {
        let Some(comp) = &self.completion else {
            return;
        };
        if comp.matches.is_empty() {
            return;
        }
        const MAX_ROWS: usize = 8;
        let rows = comp.matches.len().min(MAX_ROWS);
        let width = comp
            .matches
            .iter()
            .map(|m| m.chars().count())
            .max()
            .unwrap_or(4)
            .clamp(4, 30) as u16
            + 2;

        let anchor_x = inner.x + comp.anchor.min(inner.width as usize) as u16;
        let cursor_y = inner.y + (cur_line.saturating_sub(self.scroll_y)) as u16;
        // Prefer below the cursor; flip above if no room.
        let below = cursor_y + 1 + rows as u16 <= inner.y + inner.height;
        let y = if below {
            cursor_y + 1
        } else {
            cursor_y.saturating_sub(rows as u16)
        };
        let x = anchor_x.min(inner.x + inner.width.saturating_sub(width));
        let area = Rect {
            x,
            y,
            width: width.min(inner.width),
            height: rows as u16,
        };
        Clear.render(area, buf);

        // Scroll the list so the highlighted entry is visible.
        let start = if comp.idx >= rows {
            comp.idx + 1 - rows
        } else {
            0
        };
        let _ = cur_col;
        for (vis, m) in comp.matches.iter().skip(start).take(rows).enumerate() {
            let i = start + vis;
            let style = if i == comp.idx {
                Style::default()
                    .fg(theme::color_background())
                    .bg(theme::color_primary())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme::color_text())
                    .bg(theme::color_secondary())
            };
            let label = truncate_chars(&format!(" {m}"), area.width as usize);
            let padded = format!("{label:<w$}", w = area.width as usize);
            buf.set_string(area.x, area.y + vis as u16, &padded, style);
        }
    }
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "INSERT",
    "INTO",
    "UPDATE",
    "DELETE",
    "SET",
    "VALUES",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "OUTER",
    "FULL",
    "CROSS",
    "ON",
    "AND",
    "OR",
    "NOT",
    "NULL",
    "ORDER",
    "BY",
    "GROUP",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "AS",
    "IN",
    "LIKE",
    "BETWEEN",
    "IS",
    "DISTINCT",
    "CREATE",
    "TABLE",
    "DROP",
    "ALTER",
    "ADD",
    "INDEX",
    "PRIMARY",
    "KEY",
    "FOREIGN",
    "REFERENCES",
    "TRUNCATE",
    "RENAME",
    "VIEW",
    "UNION",
    "ALL",
    "ASC",
    "DESC",
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "SHOW",
    "COLUMNS",
    "DESCRIBE",
    "EXPLAIN",
    "USE",
    "DEFAULT",
    "INT",
    "VARCHAR",
    "TEXT",
    "DATETIME",
    "TIMESTAMP",
    "BOOLEAN",
    "DECIMAL",
];

/// Split a console line into syntax-highlighted segments.
fn highlight_line(line: &str) -> Vec<(String, Style)> {
    let kw_style = Style::default()
        .fg(theme::color_primary())
        .add_modifier(Modifier::BOLD);
    let str_style = Style::default().fg(theme::color_success());
    let comment_style = Style::default().fg(theme::color_muted());
    let normal = Style::default().fg(theme::color_text());

    // Whole-line comment.
    if let Some(pos) = line.find("--") {
        if line[..pos].trim().is_empty() {
            return vec![(line.to_string(), comment_style)];
        }
    }

    let mut out: Vec<(String, Style)> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut buf = String::new();
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            if !buf.is_empty() {
                out.push((std::mem::take(&mut buf), normal));
            }
            // String literal until the closing quote.
            let mut s = String::from(c);
            i += 1;
            while i < chars.len() {
                s.push(chars[i]);
                if chars[i] == '\'' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push((s, str_style));
        } else if c.is_alphanumeric() || c == '_' {
            buf.push(c);
            i += 1;
        } else {
            if !buf.is_empty() {
                let word = std::mem::take(&mut buf);
                out.push((word.clone(), word_style(&word, kw_style, normal)));
            }
            out.push((c.to_string(), normal));
            i += 1;
        }
    }
    if !buf.is_empty() {
        out.push((buf.clone(), word_style(&buf, kw_style, normal)));
    }
    out
}

fn word_style(word: &str, kw: Style, normal: Style) -> Style {
    if SQL_KEYWORDS.contains(&word.to_uppercase().as_str()) {
        kw
    } else {
        normal
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The SQL keyword list, exposed so `app.rs` can include keywords as completion
/// candidates and resolve schema context in one place.
pub fn sql_keywords() -> &'static [&'static str] {
    SQL_KEYWORDS
}

/// Extract the `;`-separated statement containing byte/char `offset`.
fn statement_at(text: &str, offset: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.iter().all(|c| c.is_whitespace() || *c == ';') {
        return None;
    }
    let clamped = offset.min(chars.len());

    // Split into `;`-delimited segments with their char ranges.
    let mut segments: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for (i, &ch) in chars.iter().enumerate() {
        if ch == ';' {
            segments.push((start, i));
            start = i + 1;
        }
    }
    segments.push((start, chars.len()));

    // The cursor sits in the last segment that begins at/before it.
    let mut chosen = 0;
    for (idx, &(s, _)) in segments.iter().enumerate() {
        if s <= clamped {
            chosen = idx;
        }
    }

    // If that segment is empty (e.g. the cursor is just after a trailing `;`),
    // fall back to the nearest preceding non-empty statement.
    loop {
        let (s, e) = segments[chosen];
        let seg: String = chars[s..e].iter().collect();
        let trimmed = seg.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
        if chosen == 0 {
            return None;
        }
        chosen -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_picks_segment_under_cursor() {
        let text = "SELECT 1;\nSELECT 2;\nSELECT 3";
        // offset inside "SELECT 2"
        let off = text.find("SELECT 2").unwrap() + 2;
        assert_eq!(statement_at(text, off).as_deref(), Some("SELECT 2"));
    }

    #[test]
    fn statement_first_segment() {
        let text = "SELECT a FROM t; SELECT b";
        assert_eq!(statement_at(text, 3).as_deref(), Some("SELECT a FROM t"));
    }

    #[test]
    fn statement_none_when_empty() {
        assert_eq!(statement_at("   \n  ", 1), None);
    }

    #[test]
    fn statement_falls_back_when_cursor_after_trailing_semicolon() {
        let text = "SELECT * FROM users;";
        // Cursor right after the trailing ';' (empty segment) → previous stmt.
        assert_eq!(
            statement_at(text, text.chars().count()).as_deref(),
            Some("SELECT * FROM users")
        );
    }

    #[test]
    fn statement_falls_back_across_trailing_newline() {
        let text = "SELECT 1;\n";
        assert_eq!(
            statement_at(text, text.chars().count()).as_deref(),
            Some("SELECT 1")
        );
    }

    #[test]
    fn completion_context_plain_word() {
        let mut e = SqlEditor::new();
        e.set_text("SELECT * FROM us");
        let (anchor, qual, prefix) = e.completion_context();
        assert_eq!(prefix, "us");
        assert_eq!(qual, None);
        assert_eq!(anchor, "SELECT * FROM ".len());
    }

    #[test]
    fn completion_context_qualified() {
        let mut e = SqlEditor::new();
        e.set_text("SELECT * FROM orders o WHERE o.cust");
        let (_, qual, prefix) = e.completion_context();
        assert_eq!(qual.as_deref(), Some("o"));
        assert_eq!(prefix, "cust");
    }

    #[test]
    fn completion_context_qualified_empty_prefix() {
        let mut e = SqlEditor::new();
        e.set_text("SELECT * FROM orders o WHERE o.");
        let (_, qual, prefix) = e.completion_context();
        assert_eq!(qual.as_deref(), Some("o"));
        assert_eq!(prefix, "");
    }

    #[test]
    fn open_and_accept_completion() {
        let mut e = SqlEditor::new();
        e.set_text("SELECT * FROM us");
        let (anchor, _, _) = e.completion_context();
        e.open_completion(anchor, vec!["users".to_string()]);
        assert!(e.completion_active());
        assert_eq!(e.text(), "SELECT * FROM us"); // unchanged until accept
        e.accept_completion();
        assert_eq!(e.text(), "SELECT * FROM users");
    }

    #[test]
    fn editor_cursor_offset_tracks_lines() {
        let mut e = SqlEditor::new();
        e.set_text("ab\ncd");
        // cursor at end (line 1, col 2) → offset = 3 (ab\n) + 2 = 5
        assert_eq!(e.cursor_offset(), 5);
        assert_eq!(e.statement_at_cursor().as_deref(), Some("ab\ncd"));
    }
}
