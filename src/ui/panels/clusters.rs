use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::aws::Cluster;
use crate::ui::style::theme;

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
        let lower = self.filter.to_lowercase();
        self.filtered = self
            .clusters
            .iter()
            .enumerate()
            .filter(|(_, c)| lower.is_empty() || c.cluster_name.to_lowercase().contains(&lower))
            .map(|(i, _)| i)
            .collect();
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

            let line = Line::from(vec![
                Span::styled(format!(" {}", cluster.cluster_name), style),
                Span::styled(&counts, Style::default().fg(status_color)),
            ]);
            buf.set_line(inner.x, y, &line, inner.width);
        }
    }
}
