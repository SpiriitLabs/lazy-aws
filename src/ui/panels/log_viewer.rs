use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

use crate::ui::style::theme;

pub struct LogViewerPanel {
    pub lines: Vec<String>,
    filtered_indices: Vec<usize>, // indices into `lines` that match the filter
    pub filter: String,
    pub cursor: usize, // position in the filtered list
    pub scroll_y: usize,
    pub follow: bool,
    pub show_timestamps: bool,
    last_visible_height: usize,
}

/// Largeur de la gouttière timestamp : "HH:MM:SS" + 1 espace.
const TS_GUTTER_W: usize = 9;

impl Default for LogViewerPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl LogViewerPanel {
    pub fn new() -> Self {
        LogViewerPanel {
            lines: vec![],
            filtered_indices: vec![],
            filter: String::new(),
            cursor: 0,
            scroll_y: 0,
            follow: true,
            show_timestamps: true,
            last_visible_height: 20,
        }
    }

    /// Bascule l'affichage de la gouttière timestamp (touche `t`).
    pub fn toggle_timestamps(&mut self) {
        self.show_timestamps = !self.show_timestamps;
    }

    /// Returns the visible lines (filtered or all).
    pub fn visible_lines(&self) -> Vec<&str> {
        if self.filter.is_empty() {
            self.lines.iter().map(|s| s.as_str()).collect()
        } else {
            self.filtered_indices
                .iter()
                .filter_map(|&i| self.lines.get(i).map(|s| s.as_str()))
                .collect()
        }
    }

    fn visible_count(&self) -> usize {
        if self.filter.is_empty() {
            self.lines.len()
        } else {
            self.filtered_indices.len()
        }
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.rebuild_filter();
        // Le suivi live est préservé : si on suivait, on se recale au bas de la
        // liste filtrée pour continuer à voir arriver les lignes qui matchent.
        // Sinon on revient en haut de la nouvelle liste filtrée.
        if self.follow {
            self.go_to_bottom();
        } else {
            self.cursor = 0;
            self.scroll_y = 0;
        }
    }

    pub fn clear_filter(&mut self) {
        self.set_filter("");
    }

    fn rebuild_filter(&mut self) {
        let lower = self.filter.to_lowercase();
        self.filtered_indices = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.to_lowercase().contains(&lower))
            .map(|(i, _)| i)
            .collect();
    }

    pub fn append_line(&mut self, line: &str) {
        let idx = self.lines.len();
        self.lines.push(line.to_string());
        // Update filter index if line matches
        if !self.filter.is_empty() && line.to_lowercase().contains(&self.filter.to_lowercase()) {
            self.filtered_indices.push(idx);
        }
        if self.follow {
            let count = self.visible_count();
            self.cursor = count.saturating_sub(1);
            if count > self.last_visible_height {
                self.scroll_y = count - self.last_visible_height;
            }
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.filtered_indices.clear();
        self.cursor = 0;
        self.scroll_y = 0;
    }

    pub fn move_up(&mut self) {
        self.follow = false;
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        if self.cursor < self.scroll_y {
            self.scroll_y = self.cursor;
        }
    }

    pub fn move_down(&mut self) {
        self.follow = false;
        let count = self.visible_count();
        if count > 0 && self.cursor < count - 1 {
            self.cursor += 1;
        }
        if self.cursor >= self.scroll_y + self.last_visible_height {
            self.scroll_y = self.cursor - self.last_visible_height + 1;
        }
        if self.cursor == self.visible_count().saturating_sub(1) {
            self.follow = true;
        }
    }

    pub fn go_to_top(&mut self) {
        self.follow = false;
        self.cursor = 0;
        self.scroll_y = 0;
    }

    pub fn go_to_bottom(&mut self) {
        let count = self.visible_count();
        if count > 0 {
            self.cursor = count - 1;
            self.follow = true;
            if count > self.last_visible_height {
                self.scroll_y = count - self.last_visible_height;
            }
        }
    }

    pub fn page_up(&mut self) {
        let page = self.last_visible_height.saturating_sub(2);
        self.follow = false;
        self.cursor = self.cursor.saturating_sub(page);
        if self.cursor < self.scroll_y {
            self.scroll_y = self.cursor;
        }
    }

    pub fn page_down(&mut self) {
        let page = self.last_visible_height.saturating_sub(2);
        self.follow = false;
        let count = self.visible_count();
        if count > 0 {
            self.cursor = (self.cursor + page).min(count - 1);
            if self.cursor >= self.scroll_y + self.last_visible_height {
                self.scroll_y = self.cursor - self.last_visible_height + 1;
            }
            if self.cursor == count - 1 {
                self.follow = true;
            }
        }
    }

    /// Returns the currently selected log line.
    pub fn selected_line(&self) -> Option<&str> {
        let lines = self.visible_lines();
        lines.get(self.cursor).copied()
    }

    /// Renders the log list with a cursor highlight.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, is_active: bool) {
        let border_color = if is_active {
            theme::color_border_focus()
        } else {
            theme::color_border()
        };

        let count = self.visible_count();
        let filter_indicator = if self.filter.is_empty() {
            String::new()
        } else {
            format!(" filter:\"{}\"", self.filter)
        };
        // Indicateur d'état du suivi : ● LIVE (vert) quand on suit, sinon
        // ⏸ PAUSED (jaune). Avec un filtre actif on précise la cause.
        let (follow_label, follow_color) = if self.follow {
            ("● LIVE", theme::color_success())
        } else if self.filter.is_empty() {
            ("⏸ PAUSED", theme::color_warning())
        } else {
            ("⏸ PAUSED (filter)", theme::color_warning())
        };
        let title = format!(
            " Logs [{}/{}]{} ",
            if count == 0 { 0 } else { self.cursor + 1 },
            count,
            filter_indicator,
        );

        let block = Block::default()
            .title(title)
            .title_top(
                ratatui::text::Line::from(ratatui::text::Span::styled(
                    format!(" {follow_label} "),
                    Style::default()
                        .fg(follow_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .right_aligned(),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        let visible_h = inner.height as usize;
        let visible_w = inner.width.saturating_sub(1) as usize;
        self.last_visible_height = visible_h;

        let display_lines = self.visible_lines();

        if display_lines.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            let msg = if self.filter.is_empty() {
                "No logs"
            } else {
                "No logs match filter"
            };
            buf.set_string(inner.x + 1, inner.y, msg, style);
            return;
        }

        let offset = if self.follow && display_lines.len() > visible_h {
            display_lines.len() - visible_h
        } else {
            self.scroll_y
        };

        let normal_style = Style::default().fg(theme::color_text());
        let selected_style = crate::ui::style::styles::selection_style(true);
        let selected_inactive = crate::ui::style::styles::selection_style(false);
        let highlight_style = Style::default()
            .fg(theme::color_primary())
            .add_modifier(Modifier::BOLD);
        let muted_style = Style::default().fg(theme::color_muted());

        // Quand les timestamps sont affichés, on réserve une gouttière de
        // largeur fixe à gauche pour que les messages restent alignés (même
        // pour les lignes sans timestamp, comme "-- tail ended --").
        let gutter_w = if self.show_timestamps { TS_GUTTER_W } else { 0 };
        let msg_x = inner.x + 1 + gutter_w as u16;
        let msg_w = visible_w.saturating_sub(gutter_w);

        for (i, line) in display_lines.iter().skip(offset).enumerate() {
            if i >= visible_h {
                break;
            }
            let y = inner.y + i as u16;
            let line_idx = i + offset;
            let is_selected = line_idx == self.cursor;

            let (ts, msg) = if self.show_timestamps {
                split_timestamp(line)
            } else {
                (None, *line)
            };

            // Gouttière timestamp (toujours en muted, y compris ligne sélectionnée).
            if self.show_timestamps {
                let ts_text = ts.as_deref().unwrap_or("");
                let padded = format!("{:<width$}", ts_text, width = gutter_w);
                buf.set_string(inner.x + 1, y, &padded, muted_style);
            }

            // Style du message selon le niveau de log détecté.
            let level_style = match theme::log_level_color(line) {
                Some(c) => Style::default().fg(c),
                None => normal_style,
            };
            let base_style = if is_selected && is_active {
                selected_style
            } else if is_selected {
                selected_inactive
            } else {
                level_style
            };

            let truncated: String = msg.chars().take(msg_w).collect();

            if is_selected {
                let padded = format!("{:<width$}", truncated, width = msg_w);
                buf.set_string(msg_x, y, &padded, base_style);
            } else if !self.filter.is_empty() {
                // Highlight matching text in non-selected lines
                render_highlighted_line(
                    buf,
                    msg_x,
                    y,
                    &truncated,
                    &self.filter,
                    level_style,
                    highlight_style,
                    msg_w,
                );
            } else {
                buf.set_string(msg_x, y, &truncated, base_style);
            }

            if msg.chars().count() > msg_w && !is_selected {
                let arrow_x = inner.x + inner.width.saturating_sub(1);
                buf.set_string(arrow_x, y, "→", muted_style);
            }
        }
    }
}

/// Si la ligne commence par un timestamp ISO8601 (format émis par
/// `aws logs tail`), renvoie `(Some("HH:MM:SS"), reste_du_message)`.
/// Sinon renvoie `(None, ligne_complète)`.
fn split_timestamp(line: &str) -> (Option<String>, &str) {
    if let Some((first, rest)) = line.split_once(' ') {
        if let Some(hms) = iso_to_hms(first) {
            return (Some(hms), rest);
        }
    }
    (None, line)
}

/// Extrait `HH:MM:SS` d'un token type `2026-06-15T12:00:00.000+00:00`.
fn iso_to_hms(token: &str) -> Option<String> {
    let t_pos = token.find('T')?;
    let after_t = &token[t_pos + 1..];
    if after_t.len() >= 8 {
        let b = after_t.as_bytes();
        if b[2] == b':'
            && b[5] == b':'
            && b[0].is_ascii_digit()
            && b[1].is_ascii_digit()
            && b[3].is_ascii_digit()
            && b[4].is_ascii_digit()
            && b[6].is_ascii_digit()
            && b[7].is_ascii_digit()
        {
            return Some(after_t[..8].to_string());
        }
    }
    None
}

/// Renders a line with filter matches highlighted.
#[allow(clippy::too_many_arguments)]
fn render_highlighted_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    text: &str,
    filter: &str,
    normal: Style,
    highlight: Style,
    max_w: usize,
) {
    let lower_text = text.to_lowercase();
    let lower_filter = filter.to_lowercase();
    let mut col = 0u16;
    let mut pos = 0;

    while pos < text.len() && (col as usize) < max_w {
        if let Some(match_start) = lower_text[pos..].find(&lower_filter) {
            let abs_start = pos + match_start;
            // Render text before match
            if abs_start > pos {
                let before: String = text[pos..abs_start]
                    .chars()
                    .take(max_w - col as usize)
                    .collect();
                buf.set_string(x + col, y, &before, normal);
                col += before.len() as u16;
            }
            // Render match
            let match_end = abs_start + filter.len();
            let matched: String = text[abs_start..match_end.min(text.len())]
                .chars()
                .take(max_w - col as usize)
                .collect();
            buf.set_string(x + col, y, &matched, highlight);
            col += matched.len() as u16;
            pos = match_end;
        } else {
            // No more matches, render rest
            let rest: String = text[pos..].chars().take(max_w - col as usize).collect();
            buf.set_string(x + col, y, &rest, normal);
            break;
        }
    }
}

/// Renders the full content of a selected log line with word-wrap.
/// `@ptr` fields are separated and displayed at the bottom in muted style.
pub fn render_log_detail(line: &str, area: Rect, buf: &mut Buffer, is_active: bool) {
    let border_color = if is_active {
        theme::color_border_focus()
    } else {
        theme::color_border()
    };
    let block = Block::default()
        .title(" Log Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    block.render(area, buf);

    if line.is_empty() {
        let style = Style::default().fg(theme::color_muted());
        buf.set_string(inner.x + 1, inner.y, "Select a log line", style);
        return;
    }

    let visible_w = inner.width.saturating_sub(2) as usize;
    let visible_h = inner.height as usize;

    if visible_w == 0 || visible_h == 0 {
        return;
    }

    // Separate @ptr from the rest of the content
    // Format from Insights: "key=value | key=value | @ptr=value"
    let mut main_parts: Vec<&str> = Vec::new();
    let mut ptr_value: Option<&str> = None;

    for part in line.split(" | ") {
        if let Some(val) = part.strip_prefix("@ptr=") {
            ptr_value = Some(val);
        } else {
            main_parts.push(part);
        }
    }

    let main_content = main_parts.join(" | ");

    // Couleur selon le niveau de log détecté (rouge/jaune/gris), repli sur
    // bright pour les lignes "normales" — cohérent avec la liste.
    let content_color = theme::log_level_color(&main_content).unwrap_or_else(theme::color_bright);

    // Reserve space for @ptr at the bottom (separator line + ptr line(s))
    let ptr_lines = if let Some(ptr) = ptr_value {
        let ptr_text = format!("@ptr {ptr}");
        let needed = ptr_text.len().div_ceil(visible_w); // ceil div
        needed + 1 // +1 for separator
    } else {
        0
    };
    let main_visible_h = visible_h.saturating_sub(ptr_lines);

    // Render main content with word-wrap
    let style = Style::default().fg(content_color);
    let mut y = 0usize;

    for text_line in main_content.split('\n') {
        if y >= main_visible_h {
            break;
        }
        if text_line.is_empty() {
            y += 1;
            continue;
        }
        let chars: Vec<char> = text_line.chars().collect();
        for chunk in chars.chunks(visible_w) {
            if y >= main_visible_h {
                break;
            }
            let s: String = chunk.iter().collect();
            buf.set_string(inner.x + 1, inner.y + y as u16, &s, style);
            y += 1;
        }
    }

    // Render @ptr at the bottom
    if let Some(ptr) = ptr_value {
        let ptr_style = Style::default().fg(theme::color_muted());
        let separator_y = inner.y + inner.height.saturating_sub(ptr_lines as u16);

        // Separator line
        let sep: String = "─".repeat(visible_w);
        buf.set_string(inner.x + 1, separator_y, &sep, ptr_style);

        // @ptr value
        let ptr_text = format!("@ptr {ptr}");
        let chars: Vec<char> = ptr_text.chars().collect();
        for (i, chunk) in chars.chunks(visible_w).enumerate() {
            let py = separator_y + 1 + i as u16;
            if py >= inner.y + inner.height {
                break;
            }
            let s: String = chunk.iter().collect();
            buf.set_string(inner.x + 1, py, &s, ptr_style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_timestamp_iso() {
        let (ts, rest) = split_timestamp("2026-06-15T12:34:56.000+00:00 hello world");
        assert_eq!(ts.as_deref(), Some("12:34:56"));
        assert_eq!(rest, "hello world");
    }

    #[test]
    fn split_timestamp_none_for_plain_line() {
        let (ts, rest) = split_timestamp("-- tail ended --");
        assert_eq!(ts, None);
        assert_eq!(rest, "-- tail ended --");

        let (ts, rest) = split_timestamp("Error: boom");
        assert_eq!(ts, None);
        assert_eq!(rest, "Error: boom");
    }

    #[test]
    fn follow_preserved_under_filter() {
        let mut p = LogViewerPanel::new();
        for i in 0..10 {
            p.append_line(&format!("line {i} INFO"));
        }
        p.append_line("line 10 ERROR boom");
        assert!(p.follow, "should follow by default");

        // Poser un filtre ne doit pas couper le suivi : on reste calé en bas.
        p.set_filter("ERROR");
        assert!(p.follow, "follow must survive a filter change");
        assert_eq!(p.visible_lines().len(), 1);
        assert_eq!(p.selected_line(), Some("line 10 ERROR boom"));
    }

    #[test]
    fn manual_scroll_pauses_follow() {
        let mut p = LogViewerPanel::new();
        for i in 0..10 {
            p.append_line(&format!("line {i}"));
        }
        p.move_up();
        assert!(!p.follow, "manual navigation pauses follow");
    }
}
