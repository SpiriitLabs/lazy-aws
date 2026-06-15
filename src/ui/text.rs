use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::style::styles;

/// Truncate `s` to at most `max` characters (not bytes), appending `…` when cut.
///
/// UTF-8 safe: slicing by byte offset (`&s[..n]`) panics when `n` lands inside a
/// multibyte character. This counts `char`s instead, so it never panics.
pub fn truncate_chars(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        return s.to_string();
    }
    // Reserve one column for the ellipsis marker.
    let keep = max.saturating_sub(1).max(1);
    let mut out: String = s.chars().take(keep).collect();
    out.push('…');
    out
}

/// Render a labeled field with automatic word-wrapping.
pub fn wrap_field<'a>(
    label: &'a str,
    value: &'a str,
    value_style: Style,
    width: u16,
) -> Vec<Line<'a>> {
    let label_width = label.len() + 2;
    let available = (width as usize).saturating_sub(label_width);

    if available == 0 || value.len() <= available {
        return vec![Line::from(vec![
            Span::styled(label, styles::key_style()),
            Span::raw("  "),
            Span::styled(value, value_style),
        ])];
    }

    let mut lines = Vec::new();
    let mut remaining = value;
    let mut first = true;

    while !remaining.is_empty() {
        let chunk_end = if remaining.len() <= available {
            remaining.len()
        } else {
            remaining[..available]
                .rfind(' ')
                .map_or(available, |pos| pos)
        };

        let chunk = &remaining[..chunk_end];
        remaining = if chunk_end < remaining.len() {
            remaining[chunk_end..].trim_start()
        } else {
            ""
        };

        if first {
            lines.push(Line::from(vec![
                Span::styled(label, styles::key_style()),
                Span::raw("  "),
                Span::styled(chunk, value_style),
            ]));
            first = false;
        } else {
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(label_width)),
                Span::styled(chunk, value_style),
            ]));
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_value_no_wrap() {
        let lines = wrap_field("Label:", "short", Style::default(), 80);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn long_value_wraps() {
        let lines = wrap_field(
            "Desc:",
            "This is a very long description that should wrap onto multiple lines",
            Style::default(),
            30,
        );
        assert!(lines.len() > 1);
    }

    #[test]
    fn zero_width_no_panic() {
        let lines = wrap_field("Label:", "value", Style::default(), 0);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn truncate_shorter_than_max_unchanged() {
        assert_eq!(truncate_chars("hello", 10), "hello");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate_chars("hello world", 5), "hell…");
    }

    #[test]
    fn truncate_multibyte_does_not_panic() {
        // Each "é" is two bytes; a byte slice at 3 would split a char and panic.
        let s = "ééééé";
        let out = truncate_chars(s, 3);
        assert_eq!(out.chars().count(), 3);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn truncate_zero_is_empty() {
        assert_eq!(truncate_chars("hello", 0), "");
    }
}
