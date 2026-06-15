//! In-TUI file browser used to open and save `.sql` scripts.
//!
//! It knows nothing about SQL or the console (garde-fou 6): it navigates
//! directories and yields a [`PathBuf`]. `app.rs` reads/writes the file. It
//! always starts at the directory `lazy-aws` was launched from.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::ui::fuzzy::fuzzy_match;
use crate::ui::style::{styles, theme};
use crate::ui::text::truncate_chars;

#[derive(Clone, Copy, PartialEq)]
pub enum BrowserMode {
    Open,
    Save,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Parent,
    Dir,
    File,
    SaveHere,
}

struct Entry {
    label: String,
    path: PathBuf,
    kind: Kind,
}

/// What an Enter keypress produced.
pub enum BrowserOutcome {
    /// Still browsing (navigated into a directory).
    None,
    /// Open: this file was chosen. Save: an existing file to overwrite.
    PickFile(PathBuf),
    /// Save: write a new file into this directory (app prompts for a name).
    PickDir(PathBuf),
    Cancelled,
}

pub struct FileBrowser {
    visible: bool,
    mode: BrowserMode,
    cwd: PathBuf,
    entries: Vec<Entry>,
    filtered: Vec<usize>,
    filter: String,
    cursor: usize,
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBrowser {
    pub fn new() -> Self {
        FileBrowser {
            visible: false,
            mode: BrowserMode::Open,
            cwd: PathBuf::from("."),
            entries: Vec::new(),
            filtered: Vec::new(),
            filter: String::new(),
            cursor: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn mode(&self) -> BrowserMode {
        self.mode
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Open the browser rooted at `start` (typically the launch CWD).
    pub fn open(&mut self, mode: BrowserMode, start: &Path) {
        self.mode = mode;
        self.cwd = start.to_path_buf();
        self.filter.clear();
        self.cursor = 0;
        self.visible = true;
        self.refresh();
    }

    fn refresh(&mut self) {
        self.entries = build_entries(&self.cwd, self.mode);
        self.cursor = 0;
        self.rebuild_filter();
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.rebuild_filter();
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    fn rebuild_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered = (0..self.entries.len()).collect();
        } else {
            let mut scored: Vec<(usize, i32)> = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| fuzzy_match(&e.label, &self.filter).map(|s| (i, s)))
                .collect();
            scored.sort_by_key(|b| std::cmp::Reverse(b.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }
        if self.cursor >= self.filtered.len() {
            self.cursor = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.filtered.len() {
            self.cursor += 1;
        }
    }

    fn selected(&self) -> Option<&Entry> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.entries.get(i))
    }

    /// Handle a key. Returns an outcome; on `None` keep browsing.
    pub fn handle_key(&mut self, key: KeyEvent) -> BrowserOutcome {
        match key.code {
            KeyCode::Esc => {
                self.visible = false;
                BrowserOutcome::Cancelled
            }
            KeyCode::Up => {
                self.move_up();
                BrowserOutcome::None
            }
            KeyCode::Down => {
                self.move_down();
                BrowserOutcome::None
            }
            KeyCode::Enter => self.activate(),
            _ => BrowserOutcome::None,
        }
    }

    /// Activate the selected entry (Enter). Public so `app.rs` can drive it.
    pub fn activate(&mut self) -> BrowserOutcome {
        let Some(entry) = self.selected() else {
            return BrowserOutcome::None;
        };
        match entry.kind {
            Kind::Parent | Kind::Dir => {
                self.cwd = entry.path.clone();
                self.filter.clear();
                self.refresh();
                BrowserOutcome::None
            }
            Kind::File => {
                let p = entry.path.clone();
                self.visible = false;
                BrowserOutcome::PickFile(p)
            }
            Kind::SaveHere => {
                let p = entry.path.clone();
                self.visible = false;
                BrowserOutcome::PickDir(p)
            }
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if !self.visible {
            return;
        }
        Clear.render(area, buf);

        let verb = match self.mode {
            BrowserMode::Open => "Open .sql",
            BrowserMode::Save => "Save .sql",
        };
        let title = format!(" {verb} — {} ", self.cwd.display());
        let block = Block::default()
            .title(truncate_chars(
                &title,
                area.width.saturating_sub(2) as usize,
            ))
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(theme::color_primary())
                    .add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height < 2 {
            return;
        }

        let list_h = inner.height.saturating_sub(1) as usize;
        let offset = self.cursor.saturating_sub(list_h.saturating_sub(1));
        for (vis, &ei) in self.filtered.iter().skip(offset).take(list_h).enumerate() {
            let entry = &self.entries[ei];
            let is_sel = offset + vis == self.cursor;
            let y = inner.y + vis as u16;
            let icon = match entry.kind {
                Kind::Parent => "⬆ ",
                Kind::Dir => "📁 ",
                Kind::File => "📄 ",
                Kind::SaveHere => "💾 ",
            };
            let style = if is_sel {
                styles::selection_style(true)
            } else if entry.kind == Kind::File {
                Style::default().fg(theme::color_text())
            } else {
                Style::default().fg(theme::color_info())
            };
            let line = truncate_chars(
                &format!("{icon}{}", entry.label),
                inner.width.saturating_sub(1) as usize,
            );
            buf.set_string(inner.x + 1, y, &line, style);
        }

        let hint = " ↑↓ move · Enter open/select · Esc cancel ";
        buf.set_string(
            inner.x + 1,
            inner.y + inner.height - 1,
            truncate_chars(hint, inner.width.saturating_sub(1) as usize),
            Style::default().fg(theme::color_muted()),
        );
    }
}

fn build_entries(dir: &Path, mode: BrowserMode) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();

    if mode == BrowserMode::Save {
        out.push(Entry {
            label: "‹ save in this folder ›".to_string(),
            path: dir.to_path_buf(),
            kind: Kind::SaveHere,
        });
    }
    if let Some(parent) = dir.parent() {
        out.push(Entry {
            label: "..".to_string(),
            path: parent.to_path_buf(),
            kind: Kind::Parent,
        });
    }

    let mut dirs: Vec<Entry> = Vec::new();
    let mut files: Vec<Entry> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue; // skip hidden
            }
            if path.is_dir() {
                dirs.push(Entry {
                    label: name,
                    path,
                    kind: Kind::Dir,
                });
            } else if path.extension().and_then(|e| e.to_str()) == Some("sql") {
                files.push(Entry {
                    label: name,
                    path,
                    kind: Kind::File,
                });
            }
        }
    }
    dirs.sort_by_key(|e| e.label.to_lowercase());
    files.sort_by_key(|e| e.label.to_lowercase());
    out.extend(dirs);
    out.extend(files);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn open_lists_dirs_and_sql_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("a.sql"), "SELECT 1").unwrap();
        fs::write(tmp.path().join("note.txt"), "x").unwrap();

        let mut fb = FileBrowser::new();
        fb.open(BrowserMode::Open, tmp.path());
        let labels: Vec<&str> = fb.entries.iter().map(|e| e.label.as_str()).collect();
        assert!(labels.contains(&"sub"));
        assert!(labels.contains(&"a.sql"));
        assert!(!labels.contains(&"note.txt")); // non-sql hidden in open mode
    }

    #[test]
    fn save_mode_offers_save_here() {
        let tmp = tempfile::tempdir().unwrap();
        let mut fb = FileBrowser::new();
        fb.open(BrowserMode::Save, tmp.path());
        assert!(matches!(
            fb.entries.first().map(|e| e.kind),
            Some(Kind::SaveHere)
        ));
    }

    #[test]
    fn entering_directory_navigates() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub").join("q.sql"), "SELECT 1").unwrap();

        let mut fb = FileBrowser::new();
        fb.open(BrowserMode::Open, tmp.path());
        // Move cursor onto "sub" and activate.
        let sub_pos = fb
            .filtered
            .iter()
            .position(|&i| fb.entries[i].label == "sub")
            .unwrap();
        fb.cursor = sub_pos;
        matches!(fb.activate(), BrowserOutcome::None);
        // Now "q.sql" should be visible.
        assert!(fb.entries.iter().any(|e| e.label == "q.sql"));
    }
}
