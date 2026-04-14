use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::aws::{S3ListResult, S3Object};
use crate::ui::fuzzy::fuzzy_match;
use crate::ui::style::theme;

/// An entry in the objects panel.
#[derive(Debug, Clone)]
pub enum S3ObjectItem {
    /// "../" shown at top when inside a prefix
    ParentDir,
    /// Simulated folder (full prefix string including trailing /)
    Prefix(String),
    /// Actual S3 object
    Object(S3Object),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectSort {
    Name,
    NameDesc,
    LastModified,
    LastModifiedDesc,
    Size,
    SizeDesc,
}

impl ObjectSort {
    fn label(&self) -> &str {
        match self {
            ObjectSort::Name => "↑name",
            ObjectSort::NameDesc => "↓name",
            ObjectSort::LastModified => "↑date",
            ObjectSort::LastModifiedDesc => "↓date",
            ObjectSort::Size => "↑size",
            ObjectSort::SizeDesc => "↓size",
        }
    }
}

pub struct ObjectsPanel {
    pub items: Vec<S3ObjectItem>,
    filtered: Vec<usize>,
    pub filter: String,
    pub cursor: usize,
    pub current_prefix: String,
    pub sort_by: ObjectSort,
    /// The bucket name, stored for the panel title
    pub bucket_name: String,
}

impl Default for ObjectsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectsPanel {
    pub fn new() -> Self {
        ObjectsPanel {
            items: vec![],
            filtered: vec![],
            filter: String::new(),
            cursor: 0,
            current_prefix: String::new(),
            sort_by: ObjectSort::Name,
            bucket_name: String::new(),
        }
    }

    /// Populate the panel from an S3 list-objects-v2 result.
    pub fn set_result(&mut self, result: S3ListResult) {
        self.items.clear();

        // Add parent dir entry if we're inside a prefix
        if !self.current_prefix.is_empty() {
            self.items.push(S3ObjectItem::ParentDir);
        }

        // Add common prefixes (folders)
        for cp in result.common_prefixes {
            self.items.push(S3ObjectItem::Prefix(cp.prefix));
        }

        // Add objects
        for obj in result.contents {
            // Skip the "directory marker" object that matches the current prefix exactly
            if obj.key == self.current_prefix {
                continue;
            }
            self.items.push(S3ObjectItem::Object(obj));
        }

        self.rebuild_filter();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.filtered.clear();
        self.filter.clear();
        self.cursor = 0;
        self.current_prefix.clear();
        self.sort_by = ObjectSort::Name;
        self.bucket_name.clear();
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
            ObjectSort::Name => ObjectSort::NameDesc,
            ObjectSort::NameDesc => ObjectSort::LastModified,
            ObjectSort::LastModified => ObjectSort::LastModifiedDesc,
            ObjectSort::LastModifiedDesc => ObjectSort::Size,
            ObjectSort::Size => ObjectSort::SizeDesc,
            ObjectSort::SizeDesc => ObjectSort::Name,
        };
        self.rebuild_filter();
    }

    /// Navigate into a prefix (like cd into a folder).
    pub fn navigate_into(&mut self, prefix: &str) {
        self.current_prefix = prefix.to_string();
        self.items.clear();
        self.filtered.clear();
        self.filter.clear();
        self.cursor = 0;
    }

    /// Go up one prefix level. Returns the new current_prefix.
    pub fn go_up(&mut self) -> String {
        if self.current_prefix.is_empty() {
            return String::new();
        }
        // Strip trailing / then find previous /
        let trimmed = self.current_prefix.trim_end_matches('/');
        let new_prefix = match trimmed.rfind('/') {
            Some(pos) => &trimmed[..=pos], // keep the trailing /
            None => "",
        };
        self.current_prefix = new_prefix.to_string();
        self.items.clear();
        self.filtered.clear();
        self.filter.clear();
        self.cursor = 0;
        self.current_prefix.clone()
    }

    /// Get the display name of an item relative to the current prefix.
    pub fn item_display_name(&self, item: &S3ObjectItem) -> String {
        match item {
            S3ObjectItem::ParentDir => "../".to_string(),
            S3ObjectItem::Prefix(p) => {
                let relative = p.strip_prefix(&self.current_prefix).unwrap_or(p);
                relative.to_string()
            }
            S3ObjectItem::Object(obj) => {
                let relative = obj
                    .key
                    .strip_prefix(&self.current_prefix)
                    .unwrap_or(&obj.key);
                relative.to_string()
            }
        }
    }

    fn rebuild_filter(&mut self) {
        // Separate parent dir index from the rest
        let parent_idx = self
            .items
            .iter()
            .position(|i| matches!(i, S3ObjectItem::ParentDir));

        let other_indices: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| !matches!(i, S3ObjectItem::ParentDir))
            .map(|(i, _)| i)
            .collect();

        if self.filter.is_empty() {
            // No filter: parent first, then sorted items
            self.filtered = Vec::new();
            if let Some(pi) = parent_idx {
                self.filtered.push(pi);
            }

            // Separate prefixes and objects for sorting
            let mut prefix_indices: Vec<usize> = Vec::new();
            let mut object_indices: Vec<usize> = Vec::new();
            for &i in &other_indices {
                match &self.items[i] {
                    S3ObjectItem::Prefix(_) => prefix_indices.push(i),
                    S3ObjectItem::Object(_) => object_indices.push(i),
                    S3ObjectItem::ParentDir => {}
                }
            }

            self.sort_indices(&mut prefix_indices);
            self.sort_indices(&mut object_indices);

            self.filtered.extend(prefix_indices);
            self.filtered.extend(object_indices);
        } else {
            // Fuzzy filter
            self.filtered = Vec::new();
            if let Some(pi) = parent_idx {
                self.filtered.push(pi);
            }

            let mut scored: Vec<(usize, i32)> = other_indices
                .into_iter()
                .filter_map(|i| {
                    let name = self.item_display_name(&self.items[i]);
                    fuzzy_match(&name, &self.filter).map(|s| (i, s))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));

            self.filtered.extend(scored.into_iter().map(|(i, _)| i));
        }

        let count = self.filtered.len();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        } else if count == 0 {
            self.cursor = 0;
        }
    }

    fn sort_indices(&self, indices: &mut [usize]) {
        let items = &self.items;
        let current_prefix = &self.current_prefix;
        let sort_by = &self.sort_by;

        indices.sort_by(|&a, &b| {
            let name_a = display_name_for_sort(items, a, current_prefix);
            let name_b = display_name_for_sort(items, b, current_prefix);

            match sort_by {
                ObjectSort::Name => name_a.cmp(&name_b),
                ObjectSort::NameDesc => name_b.cmp(&name_a),
                ObjectSort::LastModified => {
                    let date_a = item_last_modified(items, a);
                    let date_b = item_last_modified(items, b);
                    date_a.cmp(date_b)
                }
                ObjectSort::LastModifiedDesc => {
                    let date_a = item_last_modified(items, a);
                    let date_b = item_last_modified(items, b);
                    date_b.cmp(date_a)
                }
                ObjectSort::Size => {
                    let size_a = item_size(items, a);
                    let size_b = item_size(items, b);
                    size_a.cmp(&size_b)
                }
                ObjectSort::SizeDesc => {
                    let size_a = item_size(items, a);
                    let size_b = item_size(items, b);
                    size_b.cmp(&size_a)
                }
            }
        });
    }

    pub fn selected(&self) -> Option<&S3ObjectItem> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.items.get(i))
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

        // Count only non-ParentDir items
        let real_count = self
            .items
            .iter()
            .filter(|i| !matches!(i, S3ObjectItem::ParentDir))
            .count();
        let filtered_count = self
            .filtered
            .iter()
            .filter(|&&i| !matches!(self.items[i], S3ObjectItem::ParentDir))
            .count();

        let count_text = if self.filter.is_empty() {
            format!("{}", real_count)
        } else {
            format!("{}/{}", filtered_count, real_count)
        };

        let path_display = if self.bucket_name.is_empty() {
            "Objects".to_string()
        } else if self.current_prefix.is_empty() {
            self.bucket_name.to_string()
        } else {
            format!("{}/{}", self.bucket_name, self.current_prefix)
        };

        let sort_label = self.sort_by.label();
        let block = Block::default()
            .title(format!(
                " {} [{}]{} {} ",
                path_display, count_text, filter_text, sort_label
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let inner = block.inner(area);
        block.render(area, buf);

        let items_to_show: Vec<(usize, &S3ObjectItem)> = self
            .filtered
            .iter()
            .filter_map(|&i| self.items.get(i).map(|item| (i, item)))
            .collect();

        if loading {
            let style = Style::default().fg(theme::color_primary());
            buf.set_string(inner.x + 1, inner.y, "Loading...", style);
            return;
        }
        if items_to_show.is_empty() {
            let style = Style::default().fg(theme::color_muted());
            buf.set_string(
                inner.x + 1,
                inner.y,
                if self.filter.is_empty() {
                    "Empty"
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

        for (i, (_, item)) in items_to_show.iter().skip(offset).enumerate() {
            if i >= visible {
                break;
            }
            let y = inner.y + i as u16;
            let is_selected = (i + offset) == self.cursor;

            let display = self.item_display_name(item);

            let (text_style, size_str) = match item {
                S3ObjectItem::ParentDir => {
                    (Style::default().fg(theme::color_muted()), String::new())
                }
                S3ObjectItem::Prefix(_) => {
                    (Style::default().fg(theme::color_info()), String::new())
                }
                S3ObjectItem::Object(obj) => (
                    Style::default().fg(theme::color_text()),
                    format_size(obj.size),
                ),
            };

            let base_style = if is_selected && is_active {
                text_style.add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if is_selected {
                text_style.add_modifier(Modifier::REVERSED)
            } else {
                text_style
            };

            let line = if size_str.is_empty() {
                Line::from(Span::styled(format!(" {display}"), base_style))
            } else {
                Line::from(vec![
                    Span::styled(format!(" {display}"), base_style),
                    Span::styled(
                        format!(" {size_str}"),
                        Style::default().fg(theme::color_muted()),
                    ),
                ])
            };
            buf.set_line(inner.x, y, &line, inner.width);
        }
    }
}

fn display_name_for_sort(items: &[S3ObjectItem], idx: usize, current_prefix: &str) -> String {
    match &items[idx] {
        S3ObjectItem::ParentDir => String::new(),
        S3ObjectItem::Prefix(p) => p.strip_prefix(current_prefix).unwrap_or(p).to_string(),
        S3ObjectItem::Object(obj) => obj
            .key
            .strip_prefix(current_prefix)
            .unwrap_or(&obj.key)
            .to_string(),
    }
}

fn item_last_modified(items: &[S3ObjectItem], idx: usize) -> &str {
    match &items[idx] {
        S3ObjectItem::Object(obj) => &obj.last_modified,
        _ => "",
    }
}

fn item_size(items: &[S3ObjectItem], idx: usize) -> i64 {
    match &items[idx] {
        S3ObjectItem::Object(obj) => obj.size,
        _ => -1, // Prefixes sort before objects when ascending
    }
}

pub fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::{S3CommonPrefix, S3ListResult, S3Object};

    fn make_object(key: &str, size: i64, modified: &str) -> S3Object {
        S3Object {
            key: key.to_string(),
            size,
            last_modified: modified.to_string(),
            storage_class: "STANDARD".to_string(),
            e_tag: String::new(),
        }
    }

    fn make_result(prefixes: &[&str], objects: Vec<S3Object>) -> S3ListResult {
        S3ListResult {
            common_prefixes: prefixes
                .iter()
                .map(|p| S3CommonPrefix {
                    prefix: p.to_string(),
                })
                .collect(),
            contents: objects,
            is_truncated: false,
            key_count: 0,
            next_continuation_token: None,
        }
    }

    #[test]
    fn navigate_into_and_set_result() {
        let mut panel = ObjectsPanel::new();
        panel.bucket_name = "my-bucket".to_string();
        panel.navigate_into("");

        let result = make_result(
            &["logs/", "data/"],
            vec![make_object("readme.txt", 100, "2024-01-01")],
        );
        panel.set_result(result);

        // No ParentDir at root
        assert_eq!(panel.items.len(), 3); // 2 prefixes + 1 object
        assert!(matches!(panel.items[0], S3ObjectItem::Prefix(_)));
    }

    #[test]
    fn parent_dir_shown_in_subprefix() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("logs/");

        let result = make_result(
            &["logs/2024/"],
            vec![make_object("logs/app.log", 500, "2024-01-01")],
        );
        panel.set_result(result);

        assert!(matches!(panel.items[0], S3ObjectItem::ParentDir));
        assert_eq!(panel.items.len(), 3);
    }

    #[test]
    fn go_up_from_nested() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("logs/2024/");

        let new = panel.go_up();
        assert_eq!(new, "logs/");
        assert_eq!(panel.current_prefix, "logs/");
    }

    #[test]
    fn go_up_from_first_level() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("logs/");

        let new = panel.go_up();
        assert_eq!(new, "");
        assert_eq!(panel.current_prefix, "");
    }

    #[test]
    fn go_up_from_root() {
        let mut panel = ObjectsPanel::new();
        let new = panel.go_up();
        assert_eq!(new, "");
    }

    #[test]
    fn display_name_strips_prefix() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("logs/");

        let result = make_result(
            &["logs/2024/"],
            vec![make_object("logs/app.log", 500, "2024-01-01")],
        );
        panel.set_result(result);

        // ParentDir
        assert_eq!(panel.item_display_name(&panel.items[0]), "../");
        // Prefix
        assert_eq!(panel.item_display_name(&panel.items[1]), "2024/");
        // Object
        assert_eq!(panel.item_display_name(&panel.items[2]), "app.log");
    }

    #[test]
    fn fuzzy_filter_objects() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("");

        let result = make_result(
            &["logs/", "data/"],
            vec![
                make_object("readme.txt", 100, "2024-01-01"),
                make_object("config.json", 200, "2024-01-02"),
            ],
        );
        panel.set_result(result);

        panel.set_filter("lg");
        // Should match "logs/" prefix
        let names: Vec<String> = panel
            .filtered
            .iter()
            .map(|&i| panel.item_display_name(&panel.items[i]))
            .collect();
        assert!(names.contains(&"logs/".to_string()));
        assert!(!names.contains(&"data/".to_string()));
    }

    #[test]
    fn sort_by_size() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("");

        let result = make_result(
            &[],
            vec![
                make_object("big.bin", 10000, "2024-01-01"),
                make_object("small.txt", 10, "2024-01-02"),
                make_object("medium.dat", 5000, "2024-01-03"),
            ],
        );
        panel.set_result(result);

        panel.sort_by = ObjectSort::SizeDesc;
        panel.rebuild_filter();

        let visible: Vec<String> = panel
            .filtered
            .iter()
            .map(|&i| panel.item_display_name(&panel.items[i]))
            .collect();
        assert_eq!(visible[0], "big.bin");
        assert_eq!(visible[1], "medium.dat");
        assert_eq!(visible[2], "small.txt");
    }

    #[test]
    fn sort_by_date() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("");

        let result = make_result(
            &[],
            vec![
                make_object("old.txt", 100, "2023-01-01"),
                make_object("new.txt", 100, "2024-06-01"),
                make_object("mid.txt", 100, "2024-01-01"),
            ],
        );
        panel.set_result(result);

        panel.sort_by = ObjectSort::LastModifiedDesc;
        panel.rebuild_filter();

        let visible: Vec<String> = panel
            .filtered
            .iter()
            .map(|&i| panel.item_display_name(&panel.items[i]))
            .collect();
        assert_eq!(visible[0], "new.txt");
        assert_eq!(visible[1], "mid.txt");
        assert_eq!(visible[2], "old.txt");
    }

    #[test]
    fn format_size_values() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn skips_directory_marker_object() {
        let mut panel = ObjectsPanel::new();
        panel.navigate_into("logs/");

        let result = make_result(
            &[],
            vec![
                make_object("logs/", 0, "2024-01-01"), // directory marker
                make_object("logs/app.log", 500, "2024-01-01"),
            ],
        );
        panel.set_result(result);

        // Should have ParentDir + app.log, NOT the "logs/" marker
        let names: Vec<String> = panel
            .items
            .iter()
            .map(|i| panel.item_display_name(i))
            .collect();
        assert_eq!(names.len(), 2); // ParentDir + app.log
        assert!(names.contains(&"../".to_string()));
        assert!(names.contains(&"app.log".to_string()));
    }
}
