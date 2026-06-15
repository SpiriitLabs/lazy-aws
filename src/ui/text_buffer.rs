//! A small editable text buffer shared by `InputBox` (single line) and
//! `SqlEditor` (multi-line). It owns the text, the cursor and every editing
//! operation; widgets layer their own rendering and submit semantics on top.
//!
//! Single-line is just the degenerate case of multi-line (`newline()` is a
//! no-op), so there is exactly one implementation of cursor motion, word
//! deletion and the `Ctrl+Backspace → Ctrl+H` quirk (garde-fou 7).

pub struct TextBuffer {
    lines: Vec<Vec<char>>,
    cur_line: usize,
    cur_col: usize,
    multiline: bool,
}

impl TextBuffer {
    pub fn new(multiline: bool) -> Self {
        TextBuffer {
            lines: vec![Vec::new()],
            cur_line: 0,
            cur_col: 0,
            multiline,
        }
    }

    pub fn from_str(s: &str, multiline: bool) -> Self {
        let mut b = TextBuffer::new(multiline);
        b.set_text(s);
        b
    }

    pub fn set_text(&mut self, s: &str) {
        if self.multiline {
            self.lines = s.split('\n').map(|l| l.chars().collect()).collect();
            if self.lines.is_empty() {
                self.lines.push(Vec::new());
            }
        } else {
            // Collapse any newlines into a single line.
            self.lines = vec![s.chars().filter(|&c| c != '\n').collect()];
        }
        self.cur_line = self.lines.len() - 1;
        self.cur_col = self.lines[self.cur_line].len();
    }

    pub fn clear(&mut self) {
        self.lines = vec![Vec::new()];
        self.cur_line = 0;
        self.cur_col = 0;
    }

    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.is_empty())
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, idx: usize) -> Option<&[char]> {
        self.lines.get(idx).map(|l| l.as_slice())
    }

    /// `(line, col)` cursor position.
    pub fn cursor(&self) -> (usize, usize) {
        (self.cur_line, self.cur_col)
    }

    // ---- editing ---------------------------------------------------------

    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            self.newline();
            return;
        }
        self.lines[self.cur_line].insert(self.cur_col, c);
        self.cur_col += 1;
    }

    pub fn newline(&mut self) {
        if !self.multiline {
            return;
        }
        let tail = self.lines[self.cur_line].split_off(self.cur_col);
        self.lines.insert(self.cur_line + 1, tail);
        self.cur_line += 1;
        self.cur_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cur_col > 0 {
            self.cur_col -= 1;
            self.lines[self.cur_line].remove(self.cur_col);
        } else if self.cur_line > 0 {
            // Merge with the previous line.
            let cur = self.lines.remove(self.cur_line);
            self.cur_line -= 1;
            self.cur_col = self.lines[self.cur_line].len();
            self.lines[self.cur_line].extend(cur);
        }
    }

    pub fn delete(&mut self) {
        let len = self.lines[self.cur_line].len();
        if self.cur_col < len {
            self.lines[self.cur_line].remove(self.cur_col);
        } else if self.cur_line + 1 < self.lines.len() {
            let next = self.lines.remove(self.cur_line + 1);
            self.lines[self.cur_line].extend(next);
        }
    }

    pub fn delete_prev_word(&mut self) {
        let target = self.prev_word_boundary();
        self.lines[self.cur_line].drain(target..self.cur_col);
        self.cur_col = target;
    }

    pub fn delete_next_word(&mut self) {
        let target = self.next_word_boundary();
        self.lines[self.cur_line].drain(self.cur_col..target);
    }

    // ---- navigation ------------------------------------------------------

    pub fn move_left(&mut self) {
        if self.cur_col > 0 {
            self.cur_col -= 1;
        } else if self.cur_line > 0 {
            self.cur_line -= 1;
            self.cur_col = self.lines[self.cur_line].len();
        }
    }

    pub fn move_right(&mut self) {
        if self.cur_col < self.lines[self.cur_line].len() {
            self.cur_col += 1;
        } else if self.cur_line + 1 < self.lines.len() {
            self.cur_line += 1;
            self.cur_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cur_line > 0 {
            self.cur_line -= 1;
            self.cur_col = self.cur_col.min(self.lines[self.cur_line].len());
        }
    }

    pub fn move_down(&mut self) {
        if self.cur_line + 1 < self.lines.len() {
            self.cur_line += 1;
            self.cur_col = self.cur_col.min(self.lines[self.cur_line].len());
        }
    }

    pub fn home(&mut self) {
        self.cur_col = 0;
    }

    pub fn end(&mut self) {
        self.cur_col = self.lines[self.cur_line].len();
    }

    pub fn word_left(&mut self) {
        self.cur_col = self.prev_word_boundary();
    }

    pub fn word_right(&mut self) {
        self.cur_col = self.next_word_boundary();
    }

    fn prev_word_boundary(&self) -> usize {
        let line = &self.lines[self.cur_line];
        if self.cur_col == 0 {
            return 0;
        }
        let mut pos = self.cur_col - 1;
        while pos > 0 && line[pos] == ' ' {
            pos -= 1;
        }
        while pos > 0 && line[pos - 1] != ' ' {
            pos -= 1;
        }
        pos
    }

    fn next_word_boundary(&self) -> usize {
        let line = &self.lines[self.cur_line];
        let len = line.len();
        if self.cur_col >= len {
            return len;
        }
        let mut pos = self.cur_col;
        while pos < len && line[pos] != ' ' {
            pos += 1;
        }
        while pos < len && line[pos] == ' ' {
            pos += 1;
        }
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_ignores_newline() {
        let mut b = TextBuffer::new(false);
        b.insert_char('a');
        b.newline();
        b.insert_char('b');
        assert_eq!(b.text(), "ab");
        assert_eq!(b.line_count(), 1);
    }

    #[test]
    fn multiline_newline_splits() {
        let mut b = TextBuffer::from_str("hello world", true);
        // cursor at end; move home then 5 right to after "hello"
        b.home();
        for _ in 0..5 {
            b.move_right();
        }
        b.newline();
        assert_eq!(b.text(), "hello\n world");
        assert_eq!(b.line_count(), 2);
    }

    #[test]
    fn backspace_merges_lines() {
        let mut b = TextBuffer::from_str("ab\ncd", true);
        // cursor at end of "cd"; go to start of second line
        b.home();
        b.backspace(); // merge
        assert_eq!(b.text(), "abcd");
        assert_eq!(b.cursor(), (0, 2));
    }

    #[test]
    fn delete_prev_word_on_line() {
        let mut b = TextBuffer::from_str("foo bar baz", false);
        b.delete_prev_word();
        assert_eq!(b.text(), "foo bar ");
    }

    #[test]
    fn set_text_collapses_newlines_when_single_line() {
        let b = TextBuffer::from_str("a\nb\nc", false);
        assert_eq!(b.text(), "abc");
    }

    #[test]
    fn vertical_navigation_clamps_column() {
        let mut b = TextBuffer::from_str("longline\nhi", true);
        b.cursor(); // (1, 2)
        b.move_up(); // to line 0, col clamped... cursor was at end (1,2)
        let (l, c) = b.cursor();
        assert_eq!(l, 0);
        assert_eq!(c, 2);
    }
}
