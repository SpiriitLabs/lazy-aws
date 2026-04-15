use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::ui::style::theme;

/// Hint is a key/description pair.
#[derive(Debug, Clone)]
pub struct Hint {
    pub key: String,
    pub desc: String,
}

/// StatusBar renders contextual keyboard hints at the bottom.
pub struct StatusBar {
    pub width: u16,
    pub hints: Vec<Hint>,
    pub aws_info: String,
    pub loading_msg: String,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBar {
    pub fn new() -> Self {
        StatusBar {
            width: 0,
            hints: vec![],
            aws_info: String::new(),
            loading_msg: String::new(),
        }
    }

    pub fn set_aws_info(&mut self, info: &str) {
        self.aws_info = info.to_string();
    }

    pub fn set_loading(&mut self, msg: &str) {
        self.loading_msg = msg.to_string();
    }

    pub fn set_width(&mut self, width: u16) {
        self.width = width;
    }

    pub fn set_hints(&mut self, hints: Vec<Hint>) {
        self.hints = hints;
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let bar_bg = if theme::mode() == theme::ThemeMode::Light {
            Color::Rgb(0xE0, 0xE0, 0xE0)
        } else {
            Color::Rgb(0x1A, 0x1A, 0x1A)
        };
        let bg = Style::default().fg(theme::color_text()).bg(bar_bg);
        for x in area.x..area.x + area.width {
            buf.set_string(x, area.y, " ", bg);
        }

        let mut x = area.x + 1;

        // Loading message
        if !self.loading_msg.is_empty() {
            let msg = format!("⟳ {}", self.loading_msg);
            let style = Style::default()
                .fg(Color::Rgb(0x00, 0x00, 0x00))
                .bg(theme::color_warning());
            buf.set_string(x, area.y, &msg, style);
            x += msg.len() as u16 + 2;
        }

        // Hints
        for hint in &self.hints {
            let key_style = Style::default().fg(theme::color_primary()).bg(bar_bg);
            buf.set_string(x, area.y, &hint.key, key_style);
            x += hint.key.len() as u16 + 1;
            buf.set_string(x, area.y, &hint.desc, bg);
            x += hint.desc.len() as u16 + 2;
        }

        // AWS info on the right
        if !self.aws_info.is_empty() {
            let info_len = self.aws_info.len() as u16;
            if area.width > info_len + 2 {
                let info_x = area.x + area.width - info_len - 1;
                let info_style = Style::default().fg(theme::color_primary()).bg(bar_bg);
                buf.set_string(info_x, area.y, &self.aws_info, info_style);
            }
        }
    }
}

/// Returns the base hints shown on the status bar for a given tab.
pub fn default_hints(tab: usize) -> Vec<Hint> {
    let mut hints = vec![
        Hint {
            key: "j/k".to_string(),
            desc: "navigate".to_string(),
        },
        Hint {
            key: "tab".to_string(),
            desc: "switch panel".to_string(),
        },
    ];

    match tab {
        0 => {
            // ECS
            hints.push(Hint {
                key: "enter".to_string(),
                desc: "select".to_string(),
            });
            hints.push(Hint {
                key: "f".to_string(),
                desc: "force deploy".to_string(),
            });
        }
        1 => {
            // Tasks
            hints.push(Hint {
                key: "e".to_string(),
                desc: "exec shell".to_string(),
            });
            hints.push(Hint {
                key: "l".to_string(),
                desc: "view logs".to_string(),
            });
            hints.push(Hint {
                key: "x".to_string(),
                desc: "stop task".to_string(),
            });
        }
        2 => {
            // SSM
            hints.push(Hint {
                key: "s".to_string(),
                desc: "start session".to_string(),
            });
        }
        3 => {
            // Logs
            hints.push(Hint {
                key: "f".to_string(),
                desc: "live tail".to_string(),
            });
            hints.push(Hint {
                key: "i".to_string(),
                desc: "Insights".to_string(),
            });
            hints.push(Hint {
                key: "S".to_string(),
                desc: "export".to_string(),
            });
        }
        4 => {
            // RDS
            hints.push(Hint {
                key: "c".to_string(),
                desc: "connect".to_string(),
            });
            hints.push(Hint {
                key: "s".to_string(),
                desc: "SQL query".to_string(),
            });
            hints.push(Hint {
                key: "d".to_string(),
                desc: "disconnect".to_string(),
            });
            hints.push(Hint {
                key: "e".to_string(),
                desc: "export CSV".to_string(),
            });
            hints.push(Hint {
                key: "i".to_string(),
                desc: "import SQL".to_string(),
            });
        }
        5 => {
            // S3
            hints.push(Hint {
                key: "enter".to_string(),
                desc: "browse".to_string(),
            });
            hints.push(Hint {
                key: "d".to_string(),
                desc: "download".to_string(),
            });
            hints.push(Hint {
                key: "u".to_string(),
                desc: "upload".to_string(),
            });
            hints.push(Hint {
                key: "x".to_string(),
                desc: "delete".to_string(),
            });
            hints.push(Hint {
                key: "s".to_string(),
                desc: "sort".to_string(),
            });
        }
        _ => {}
    }

    hints.push(Hint {
        key: "y".to_string(),
        desc: "copy".to_string(),
    });
    hints.push(Hint {
        key: "p".to_string(),
        desc: "profile".to_string(),
    });
    hints.push(Hint {
        key: "?".to_string(),
        desc: "help".to_string(),
    });
    hints.push(Hint {
        key: "q".to_string(),
        desc: "quit".to_string(),
    });

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hints_ecs() {
        let hints = default_hints(0);
        assert!(hints
            .iter()
            .any(|h| h.key == "f" && h.desc == "force deploy"));
        assert!(hints.iter().any(|h| h.key == "q"));
    }

    #[test]
    fn default_hints_tasks() {
        let hints = default_hints(1);
        assert!(hints.iter().any(|h| h.key == "e" && h.desc == "exec shell"));
        assert!(hints.iter().any(|h| h.key == "l" && h.desc == "view logs"));
    }

    #[test]
    fn default_hints_ssm() {
        let hints = default_hints(2);
        assert!(hints
            .iter()
            .any(|h| h.key == "s" && h.desc == "start session"));
    }

    #[test]
    fn default_hints_logs() {
        let hints = default_hints(3);
        assert!(hints.iter().any(|h| h.key == "f" && h.desc == "live tail"));
        assert!(hints.iter().any(|h| h.key == "i" && h.desc == "Insights"));
    }
}
