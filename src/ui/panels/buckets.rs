use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::aws::Bucket;
use crate::ui::fuzzy::fuzzy_match;
use crate::ui::style::theme;

#[derive(Debug, Clone, PartialEq)]
pub enum BucketSort {
    Name,
    NameDesc,
    CreationDate,
    CreationDateDesc,
}

impl BucketSort {
    fn label(&self) -> &str {
        match self {
            BucketSort::Name => "↑name",
            BucketSort::NameDesc => "↓name",
            BucketSort::CreationDate => "↑date",
            BucketSort::CreationDateDesc => "↓date",
        }
    }
}

pub struct BucketsPanel {
    pub buckets: Vec<Bucket>,
    filtered: Vec<usize>,
    pub filter: String,
    pub cursor: usize,
    pub sort_by: BucketSort,
}

impl Default for BucketsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl BucketsPanel {
    pub fn new() -> Self {
        BucketsPanel {
            buckets: vec![],
            filtered: vec![],
            filter: String::new(),
            cursor: 0,
            sort_by: BucketSort::Name,
        }
    }

    pub fn set_buckets(&mut self, buckets: Vec<Bucket>) {
        self.buckets = buckets;
        self.rebuild_filter();
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.rebuild_filter();
    }

    pub fn clear_filter(&mut self) {
        self.set_filter("");
    }

    pub fn cycle_sort(&mut self) {
        self.sort_by = match self.sort_by {
            BucketSort::Name => BucketSort::NameDesc,
            BucketSort::NameDesc => BucketSort::CreationDate,
            BucketSort::CreationDate => BucketSort::CreationDateDesc,
            BucketSort::CreationDateDesc => BucketSort::Name,
        };
        self.rebuild_filter();
    }

    fn rebuild_filter(&mut self) {
        if self.filter.is_empty() {
            // No filter: include all, sorted by sort_by
            self.filtered = (0..self.buckets.len()).collect();
            self.sort_filtered(&[]);
        } else {
            // Fuzzy filter with scores
            let mut scored: Vec<(usize, i32)> = self
                .buckets
                .iter()
                .enumerate()
                .filter_map(|(i, b)| fuzzy_match(&b.name, &self.filter).map(|s| (i, s)))
                .collect();
            // Sort by fuzzy score descending, then by sort_by as tiebreaker
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            let scores: Vec<i32> = scored.iter().map(|s| s.1).collect();
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
            self.sort_filtered(&scores);
        }

        let count = self.filtered.len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        } else if count == 0 {
            self.cursor = 0;
        }
    }

    fn sort_filtered(&mut self, _scores: &[i32]) {
        if !self.filter.is_empty() {
            // When filtering, fuzzy score is already the primary sort
            return;
        }
        let buckets = &self.buckets;
        let sort_by = &self.sort_by;
        self.filtered.sort_by(|&a, &b| match sort_by {
            BucketSort::Name => buckets[a].name.cmp(&buckets[b].name),
            BucketSort::NameDesc => buckets[b].name.cmp(&buckets[a].name),
            BucketSort::CreationDate => buckets[a].creation_date.cmp(&buckets[b].creation_date),
            BucketSort::CreationDateDesc => buckets[b].creation_date.cmp(&buckets[a].creation_date),
        });
    }

    fn visible(&self) -> Vec<&Bucket> {
        self.filtered
            .iter()
            .filter_map(|&i| self.buckets.get(i))
            .collect()
    }

    pub fn selected(&self) -> Option<&Bucket> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.buckets.get(i))
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let count = self.filtered.len();
        if count > 0 && self.cursor < count - 1 {
            self.cursor += 1;
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, is_active: bool, loading: bool) {
        let border_color = if is_active {
            theme::color_border_focus()
        } else {
            theme::color_border()
        };
        let filter_text = if self.filter.is_empty() {
            String::new()
        } else {
            format!(" /{}", self.filter)
        };
        let count_text = if self.filter.is_empty() {
            format!("{}", self.filtered.len())
        } else {
            format!("{}/{}", self.filtered.len(), self.buckets.len())
        };
        let sort_label = self.sort_by.label();
        let block = Block::default()
            .title(format!(
                " Buckets [{}]{} {} ",
                count_text, filter_text, sort_label
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        let items = self.visible();
        if loading {
            let style = Style::default().fg(theme::color_primary());
            buf.set_string(inner.x + 1, inner.y, "Loading...", style);
            return;
        }
        if items.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            buf.set_string(
                inner.x + 1,
                inner.y,
                if self.filter.is_empty() {
                    "No buckets found"
                } else {
                    "No match"
                },
                style,
            );
            return;
        }

        let visible = inner.height as usize;
        let offset = if self.cursor >= visible {
            self.cursor - visible + 1
        } else {
            0
        };

        for (i, bucket) in items.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let is_selected = (i + offset) == self.cursor;

            let style = if is_selected && is_active {
                Style::default()
                    .fg(theme::color_bright())
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if is_selected {
                Style::default()
                    .fg(theme::color_text())
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(theme::color_text())
            };

            let date_display = if bucket.creation_date.len() > 10 {
                &bucket.creation_date[..10]
            } else {
                &bucket.creation_date
            };

            let line = Line::from(vec![
                Span::styled(format!(" {}", bucket.name), style),
                Span::styled(
                    format!(" {}", date_display),
                    Style::default().fg(theme::color_muted()),
                ),
            ]);
            buf.set_line(inner.x, y, &line, inner.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bucket(name: &str, date: &str) -> Bucket {
        Bucket {
            name: name.to_string(),
            creation_date: date.to_string(),
        }
    }

    #[test]
    fn set_buckets_and_select() {
        let mut panel = BucketsPanel::new();
        panel.set_buckets(vec![
            make_bucket("alpha", "2023-01-01"),
            make_bucket("beta", "2023-06-01"),
        ]);
        assert_eq!(panel.selected().unwrap().name, "alpha");
    }

    #[test]
    fn move_up_down() {
        let mut panel = BucketsPanel::new();
        panel.set_buckets(vec![
            make_bucket("a", "2023-01-01"),
            make_bucket("b", "2023-01-02"),
            make_bucket("c", "2023-01-03"),
        ]);
        panel.move_down();
        assert_eq!(panel.selected().unwrap().name, "b");
        panel.move_down();
        assert_eq!(panel.selected().unwrap().name, "c");
        panel.move_down(); // at end, should not move
        assert_eq!(panel.selected().unwrap().name, "c");
        panel.move_up();
        assert_eq!(panel.selected().unwrap().name, "b");
    }

    #[test]
    fn fuzzy_filter() {
        let mut panel = BucketsPanel::new();
        panel.set_buckets(vec![
            make_bucket("my-logs-bucket", "2023-01-01"),
            make_bucket("data-bucket", "2023-01-02"),
            make_bucket("log-archive", "2023-01-03"),
        ]);
        panel.set_filter("lg");
        // "lg" should match "my-logs-bucket" and "log-archive"
        assert_eq!(panel.filtered.len(), 2);
    }

    #[test]
    fn sort_by_name() {
        let mut panel = BucketsPanel::new();
        panel.set_buckets(vec![
            make_bucket("charlie", "2023-03-01"),
            make_bucket("alpha", "2023-01-01"),
            make_bucket("bravo", "2023-02-01"),
        ]);
        // Default sort is Name ascending
        let items = panel.visible();
        assert_eq!(items[0].name, "alpha");
        assert_eq!(items[1].name, "bravo");
        assert_eq!(items[2].name, "charlie");
    }

    #[test]
    fn cycle_sort() {
        let mut panel = BucketsPanel::new();
        panel.set_buckets(vec![
            make_bucket("b", "2023-01-01"),
            make_bucket("a", "2023-06-01"),
        ]);
        // Name asc
        assert_eq!(panel.visible()[0].name, "a");
        // Name desc
        panel.cycle_sort();
        assert_eq!(panel.visible()[0].name, "b");
        // CreationDate asc
        panel.cycle_sort();
        assert_eq!(panel.visible()[0].creation_date, "2023-01-01");
        // CreationDate desc
        panel.cycle_sort();
        assert_eq!(panel.visible()[0].creation_date, "2023-06-01");
    }

    #[test]
    fn empty_panel() {
        let panel = BucketsPanel::new();
        assert!(panel.selected().is_none());
        assert_eq!(panel.filtered.len(), 0);
    }
}
