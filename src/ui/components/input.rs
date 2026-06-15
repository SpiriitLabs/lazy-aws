use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::ui::messages::Action;
use crate::ui::style::theme;
use crate::ui::text_buffer::TextBuffer;

/// InputBox is a single-line text input overlay. Text storage, cursor motion
/// and word deletion are delegated to a shared [`TextBuffer`]; the InputBox
/// owns only the overlay rendering, password masking and submit/cancel keys.
pub struct InputBox {
    buf: TextBuffer,
    label: String,
    placeholder: String,
    visible: bool,
    scroll_x: usize,     // horizontal scroll offset
    password_mode: bool, // display * instead of chars
}

impl Default for InputBox {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBox {
    pub fn new() -> Self {
        InputBox {
            buf: TextBuffer::new(false),
            label: String::new(),
            placeholder: String::new(),
            visible: false,
            scroll_x: 0,
            password_mode: false,
        }
    }

    pub fn show(&mut self, label: &str, placeholder: &str) {
        self.label = label.to_string();
        self.placeholder = placeholder.to_string();
        self.buf.clear();
        self.scroll_x = 0;
        self.visible = true;
        self.password_mode = false;
    }

    pub fn show_with_value(&mut self, label: &str, placeholder: &str, initial: &str) {
        self.label = label.to_string();
        self.placeholder = placeholder.to_string();
        self.buf.set_text(initial);
        self.scroll_x = 0;
        self.visible = true;
        self.password_mode = false;
    }

    pub fn show_password(&mut self, label: &str, placeholder: &str) {
        self.show(label, placeholder);
        self.password_mode = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn value(&self) -> String {
        self.buf.text()
    }

    fn cursor(&self) -> usize {
        self.buf.cursor().1
    }

    fn chars(&self) -> Vec<char> {
        self.buf.line(0).map(|l| l.to_vec()).unwrap_or_default()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if !self.visible {
            return None;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Enter => {
                let value = self.value();
                self.hide();
                Some(Action::InputSubmit(value))
            }
            KeyCode::Esc => {
                self.hide();
                Some(Action::InputCancel)
            }
            // Ctrl+H (Ctrl+Backspace on most terminals) / Ctrl+W: delete previous word
            KeyCode::Char('h' | 'w') if ctrl => {
                self.buf.delete_prev_word();
                None
            }
            KeyCode::Char(c) if !ctrl => {
                self.buf.insert_char(c);
                None
            }
            KeyCode::Backspace => {
                if ctrl {
                    self.buf.delete_prev_word();
                } else {
                    self.buf.backspace();
                }
                None
            }
            KeyCode::Delete => {
                if ctrl {
                    self.buf.delete_next_word();
                } else {
                    self.buf.delete();
                }
                None
            }
            KeyCode::Left => {
                if ctrl {
                    self.buf.word_left();
                } else {
                    self.buf.move_left();
                }
                None
            }
            KeyCode::Right => {
                if ctrl {
                    self.buf.word_right();
                } else {
                    self.buf.move_right();
                }
                None
            }
            KeyCode::Home => {
                self.buf.home();
                None
            }
            KeyCode::End => {
                self.buf.end();
                None
            }
            _ => None,
        }
    }

    /// Renders the input box directly into the buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if !self.visible {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default()
            .title(format!(" {} ", self.label))
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(theme::color_primary())
                    .add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width < 4 {
            return;
        }

        let field_w = inner.width.saturating_sub(2) as usize;
        let y_label = inner.y;
        let y_field = inner.y + 2;
        let y_hint = inner.y + inner.height.saturating_sub(1);

        // Label hint
        let hint_style = Style::default().fg(theme::color_muted());
        buf.set_string(
            inner.x + 1,
            y_hint,
            "Enter: submit  Esc: cancel  ←→: navigate  Ctrl+←→: word jump",
            hint_style,
        );

        let value: Vec<char> = self.chars();
        let cursor = self.cursor();

        // Show placeholder or value
        if value.is_empty() {
            let placeholder_style = Style::default().fg(theme::color_muted());
            buf.set_string(inner.x + 1, y_field, &self.placeholder, placeholder_style);
            // Cursor at start
            let cursor_style = Style::default()
                .fg(theme::color_bright())
                .add_modifier(Modifier::REVERSED);
            buf.set_string(inner.x + 1, y_field, " ", cursor_style);
            return;
        }

        // Adjust scroll to keep cursor visible
        let scroll = if cursor < self.scroll_x {
            cursor
        } else if cursor >= self.scroll_x + field_w {
            cursor - field_w + 1
        } else {
            self.scroll_x
        };

        // Render the value with cursor
        let text_style = Style::default().fg(theme::color_bright());
        let cursor_style = Style::default()
            .fg(theme::color_background())
            .bg(theme::color_primary());

        let visible_chars: Vec<char> = value.iter().skip(scroll).take(field_w).copied().collect();

        // Draw input field background
        let field_bg = Style::default()
            .fg(theme::color_text())
            .bg(theme::color_secondary());
        let bg_fill: String = " ".repeat(field_w);
        buf.set_string(inner.x + 1, y_field, &bg_fill, field_bg);

        // Draw characters
        for (i, &ch) in visible_chars.iter().enumerate() {
            let abs_pos = i + scroll;
            let x = inner.x + 1 + i as u16;
            let display_ch = if self.password_mode { '*' } else { ch };
            if abs_pos == cursor {
                buf.set_string(x, y_field, display_ch.to_string(), cursor_style);
            } else {
                buf.set_string(
                    x,
                    y_field,
                    display_ch.to_string(),
                    text_style.bg(theme::color_secondary()),
                );
            }
        }

        // Draw cursor at end if it's past the last char
        if cursor >= scroll + visible_chars.len() && cursor == value.len() {
            let x = inner.x + 1 + (cursor - scroll).min(field_w.saturating_sub(1)) as u16;
            buf.set_string(x, y_field, " ", cursor_style);
        }

        // Position indicator
        let pos_text = format!(" {}/{} ", cursor, value.len());
        let pos_style = Style::default().fg(theme::color_muted());
        let pos_x = inner.x + inner.width.saturating_sub(pos_text.len() as u16 + 1);
        buf.set_string(pos_x, y_label, &pos_text, pos_style);
    }

    // Keep view() for tests
    pub fn view(&self) -> String {
        if !self.visible {
            return String::new();
        }
        format!("{}\n\n{}", self.label, self.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn submit_returns_value() {
        let mut b = InputBox::new();
        b.show("Filter", "type here");
        b.handle_key(key(KeyCode::Char('h')));
        b.handle_key(key(KeyCode::Char('i')));
        let result = b.handle_key(key(KeyCode::Enter));
        match result {
            Some(Action::InputSubmit(v)) => assert_eq!(v, "hi"),
            other => panic!("expected InputSubmit, got {other:?}"),
        }
    }

    #[test]
    fn show_with_initial_value() {
        let mut b = InputBox::new();
        b.show_with_value("Query", "", "fields @timestamp");
        assert_eq!(b.value(), "fields @timestamp");
        assert_eq!(b.cursor(), 17); // cursor at end
    }

    #[test]
    fn cancel_returns_action() {
        let mut b = InputBox::new();
        b.show("Filter", "");
        let result = b.handle_key(key(KeyCode::Esc));
        match result {
            Some(Action::InputCancel) => {}
            other => panic!("expected InputCancel, got {other:?}"),
        }
    }

    #[test]
    fn cursor_navigation() {
        let mut b = InputBox::new();
        b.show_with_value("Test", "", "hello world");
        assert_eq!(b.cursor(), 11);

        b.handle_key(key(KeyCode::Home));
        assert_eq!(b.cursor(), 0);

        b.handle_key(key(KeyCode::End));
        assert_eq!(b.cursor(), 11);

        b.handle_key(key(KeyCode::Left));
        assert_eq!(b.cursor(), 10);

        b.handle_key(key(KeyCode::Right));
        assert_eq!(b.cursor(), 11);
    }

    #[test]
    fn insert_at_cursor() {
        let mut b = InputBox::new();
        b.show_with_value("Test", "", "hllo");
        b.handle_key(key(KeyCode::Home));
        b.handle_key(key(KeyCode::Right)); // after 'h'
        b.handle_key(key(KeyCode::Char('e')));
        assert_eq!(b.value(), "hello");
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn delete_at_cursor() {
        let mut b = InputBox::new();
        b.show_with_value("Test", "", "hello");
        b.handle_key(key(KeyCode::Home));
        b.handle_key(key(KeyCode::Delete));
        assert_eq!(b.value(), "ello");
    }

    #[test]
    fn backspace_at_cursor() {
        let mut b = InputBox::new();
        b.show_with_value("Test", "", "hello");
        b.handle_key(key(KeyCode::Home));
        b.handle_key(key(KeyCode::Right));
        b.handle_key(key(KeyCode::Right)); // cursor at 2
        b.handle_key(key(KeyCode::Backspace)); // delete 'e'
        assert_eq!(b.value(), "hllo");
        assert_eq!(b.cursor(), 1);
    }
}
