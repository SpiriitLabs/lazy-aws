use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::aws::Task;
use crate::ui::style::theme;

pub struct TasksPanel {
    pub tasks: Vec<Task>,
    filtered: Vec<usize>,
    pub filter: String,
    pub cursor: usize,
}

impl Default for TasksPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TasksPanel {
    pub fn new() -> Self {
        TasksPanel {
            tasks: vec![],
            filtered: vec![],
            filter: String::new(),
            cursor: 0,
        }
    }

    pub fn set_tasks(&mut self, tasks: Vec<Task>) {
        self.tasks = tasks;
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
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                if lower.is_empty() {
                    return true;
                }
                let id = t.task_arn.rsplit('/').next().unwrap_or("");
                id.to_lowercase().contains(&lower) || t.last_status.to_lowercase().contains(&lower)
            })
            .map(|(i, _)| i)
            .collect();
        let count = self.filtered.len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        } else if count == 0 {
            self.cursor = 0;
        }
    }

    fn visible(&self) -> Vec<&Task> {
        self.filtered
            .iter()
            .filter_map(|&i| self.tasks.get(i))
            .collect()
    }

    pub fn selected(&self) -> Option<&Task> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.tasks.get(i))
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
        let block = Block::default()
            .title(format!(
                " Tasks [{}]{} ",
                if self.filter.is_empty() {
                    format!("{}", self.filtered.len())
                } else {
                    format!("{}/{}", self.filtered.len(), self.tasks.len())
                },
                filter_text
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
                    "Select a service"
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

        for (i, task) in items.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let is_selected = (i + offset) == self.cursor;

            let short_id = task.task_arn.rsplit('/').next().unwrap_or(&task.task_arn);
            let short_id = if short_id.len() > 12 {
                &short_id[..12]
            } else {
                short_id
            };
            let status_color = theme::status_color(&task.last_status);
            let container_count = format!(" {} containers", task.containers.len());

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
                Span::styled(format!(" {short_id}"), style),
                Span::styled(
                    format!(" {}", task.last_status),
                    Style::default().fg(status_color),
                ),
                Span::styled(&container_count, Style::default().fg(theme::color_muted())),
            ]);
            buf.set_line(inner.x, y, &line, inner.width);
        }
    }
}
