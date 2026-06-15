use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::aws::Cluster;
use crate::ui::fuzzy::fuzzy_match;
use crate::ui::style::{styles, theme};

pub struct ClustersPanel {
    pub clusters: Vec<Cluster>,
    filtered: Vec<usize>,
    pub filter: String,
    pub cursor: usize,
}

impl Default for ClustersPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ClustersPanel {
    pub fn new() -> Self {
        ClustersPanel {
            clusters: vec![],
            filtered: vec![],
            filter: String::new(),
            cursor: 0,
        }
    }

    pub fn set_clusters(&mut self, clusters: Vec<Cluster>) {
        self.clusters = clusters;
        self.rebuild_filter();
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.rebuild_filter();
    }

    pub fn clear_filter(&mut self) {
        self.set_filter("");
    }

    fn rebuild_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered = (0..self.clusters.len()).collect();
        } else {
            let mut scored: Vec<(usize, i32)> = self
                .clusters
                .iter()
                .enumerate()
                .filter_map(|(i, c)| fuzzy_match(&c.cluster_name, &self.filter).map(|s| (i, s)))
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }
        let count = self.filtered.len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        } else if count == 0 {
            self.cursor = 0;
        }
    }

    fn visible(&self) -> Vec<&Cluster> {
        self.filtered
            .iter()
            .filter_map(|&i| self.clusters.get(i))
            .collect()
    }

    pub fn selected(&self) -> Option<&Cluster> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.clusters.get(i))
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
            format!("{}/{}", self.filtered.len(), self.clusters.len())
        };
        let block = Block::default()
            .title(format!(" Clusters [{}]{} ", count_text, filter_text))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        let items = self.visible();
        if loading {
            crate::ui::panels::render_placeholder(buf, inner, "Loading...", true);
            return;
        }
        if items.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            buf.set_string(
                inner.x + 1,
                inner.y,
                if self.filter.is_empty() {
                    "No clusters found"
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

        for (i, cluster) in items.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let is_selected = (i + offset) == self.cursor;
            let counts = format!(
                " {} running / {} pending / {} svc",
                cluster.running_tasks_count,
                cluster.pending_tasks_count,
                cluster.active_services_count
            );
            let status_color = theme::status_color(&cluster.status);

            let style = if is_selected {
                styles::selection_style(is_active)
            } else {
                Style::default().fg(theme::color_text())
            };

            let line = Line::from(vec![
                Span::styled(format!(" {}", cluster.cluster_name), style),
                Span::styled(&counts, Style::default().fg(status_color)),
            ]);
            buf.set_line(inner.x, y, &line, inner.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cl(name: &str) -> Cluster {
        Cluster {
            cluster_name: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn fuzzy_filter_matches_subsequence() {
        let mut p = ClustersPanel::new();
        p.set_clusters(vec![cl("staging-api"), cl("prod-api"), cl("prod-worker")]);
        p.set_filter("prdapi");
        let names: Vec<&str> = p
            .visible()
            .iter()
            .map(|c| c.cluster_name.as_str())
            .collect();
        // only "prod-api" contains p,r,d,a,p,i in order
        assert_eq!(names, vec!["prod-api"]);
    }

    #[test]
    fn fuzzy_filter_ranks_better_match_first() {
        let mut p = ClustersPanel::new();
        p.set_clusters(vec![cl("aXpXi"), cl("api")]);
        p.set_filter("api");
        let names: Vec<&str> = p
            .visible()
            .iter()
            .map(|c| c.cluster_name.as_str())
            .collect();
        // consecutive + word-boundary match ranks ahead of the spread-out one
        assert_eq!(names.first(), Some(&"api"));
    }

    #[test]
    fn empty_filter_keeps_all_in_order() {
        let mut p = ClustersPanel::new();
        p.set_clusters(vec![cl("a"), cl("b"), cl("c")]);
        p.set_filter("");
        let names: Vec<&str> = p
            .visible()
            .iter()
            .map(|c| c.cluster_name.as_str())
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }
}
