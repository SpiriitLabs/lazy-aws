use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// KeyBinding holds a set of keys and their help text.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub keys: Vec<KeyEvent>,
    pub help_key: String,
    pub help_desc: String,
}

impl KeyBinding {
    pub fn matches(&self, key: &KeyEvent) -> bool {
        self.keys.iter().any(|k| k == key)
    }
}

/// KeyMap contains all key bindings.
#[derive(Debug, Clone)]
pub struct KeyMap {
    pub quit: KeyBinding,
    pub help: KeyBinding,
    pub tab_ecs: KeyBinding,
    pub tab_tasks: KeyBinding,
    pub tab_ssm: KeyBinding,
    pub tab_logs: KeyBinding,
    pub tab_rds: KeyBinding,
    pub next_tab: KeyBinding,
    pub prev_tab: KeyBinding,
    pub up: KeyBinding,
    pub down: KeyBinding,
    pub enter: KeyBinding,
    pub back: KeyBinding,
    pub search: KeyBinding,
    pub escape: KeyBinding,
    pub refresh: KeyBinding,
    pub exec: KeyBinding,
    pub logs: KeyBinding,
    pub force_deploy: KeyBinding,
    pub stop_task: KeyBinding,
    pub session: KeyBinding,
    pub follow: KeyBinding,
    pub insights: KeyBinding,
    pub profile: KeyBinding,
    pub sso_login: KeyBinding,
    pub yank: KeyBinding,
    pub top: KeyBinding,
    pub bottom: KeyBinding,
    pub export: KeyBinding,
    pub tab_s3: KeyBinding,
    pub download: KeyBinding,
    pub upload: KeyBinding,
    pub delete_object: KeyBinding,
    pub sort: KeyBinding,
    pub sql_modify: KeyBinding,
    pub resize_mode: KeyBinding,
    pub layout_toggle: KeyBinding,
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn key_ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

fn key_shift(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

/// Returns the default key bindings.
pub fn default_key_map() -> KeyMap {
    KeyMap {
        quit: KeyBinding {
            keys: vec![key(KeyCode::Char('q')), key_ctrl(KeyCode::Char('c'))],
            help_key: "q".to_string(),
            help_desc: "quit".to_string(),
        },
        help: KeyBinding {
            keys: vec![key(KeyCode::Char('?'))],
            help_key: "?".to_string(),
            help_desc: "help".to_string(),
        },
        tab_ecs: KeyBinding {
            keys: vec![key(KeyCode::Char('1'))],
            help_key: "1".to_string(),
            help_desc: "ECS".to_string(),
        },
        tab_tasks: KeyBinding {
            keys: vec![key(KeyCode::Char('2'))],
            help_key: "2".to_string(),
            help_desc: "Tasks".to_string(),
        },
        tab_ssm: KeyBinding {
            keys: vec![key(KeyCode::Char('3'))],
            help_key: "3".to_string(),
            help_desc: "SSM".to_string(),
        },
        tab_logs: KeyBinding {
            keys: vec![key(KeyCode::Char('4'))],
            help_key: "4".to_string(),
            help_desc: "Logs".to_string(),
        },
        tab_rds: KeyBinding {
            keys: vec![key(KeyCode::Char('5'))],
            help_key: "5".to_string(),
            help_desc: "RDS".to_string(),
        },
        next_tab: KeyBinding {
            keys: vec![key(KeyCode::Tab)],
            help_key: "tab".to_string(),
            help_desc: "next panel".to_string(),
        },
        prev_tab: KeyBinding {
            keys: vec![
                key(KeyCode::BackTab),
                key_shift(KeyCode::BackTab),
                key_shift(KeyCode::Tab),
            ],
            help_key: "shift+tab".to_string(),
            help_desc: "prev panel".to_string(),
        },
        up: KeyBinding {
            keys: vec![key(KeyCode::Up), key(KeyCode::Char('k'))],
            help_key: "k/↑".to_string(),
            help_desc: "up".to_string(),
        },
        down: KeyBinding {
            keys: vec![key(KeyCode::Down), key(KeyCode::Char('j'))],
            help_key: "j/↓".to_string(),
            help_desc: "down".to_string(),
        },
        enter: KeyBinding {
            keys: vec![key(KeyCode::Enter)],
            help_key: "enter".to_string(),
            help_desc: "select".to_string(),
        },
        back: KeyBinding {
            keys: vec![key(KeyCode::Backspace), key(KeyCode::Char('h'))],
            help_key: "backspace".to_string(),
            help_desc: "back".to_string(),
        },
        search: KeyBinding {
            keys: vec![key(KeyCode::Char('/'))],
            help_key: "/".to_string(),
            help_desc: "search".to_string(),
        },
        escape: KeyBinding {
            keys: vec![key(KeyCode::Esc)],
            help_key: "esc".to_string(),
            help_desc: "cancel".to_string(),
        },
        refresh: KeyBinding {
            keys: vec![key_shift(KeyCode::Char('R'))],
            help_key: "R".to_string(),
            help_desc: "refresh".to_string(),
        },
        exec: KeyBinding {
            keys: vec![key(KeyCode::Char('e'))],
            help_key: "e".to_string(),
            help_desc: "exec shell".to_string(),
        },
        logs: KeyBinding {
            keys: vec![key(KeyCode::Char('l'))],
            help_key: "l".to_string(),
            help_desc: "view logs".to_string(),
        },
        force_deploy: KeyBinding {
            keys: vec![key(KeyCode::Char('f'))],
            help_key: "f".to_string(),
            help_desc: "force deploy".to_string(),
        },
        stop_task: KeyBinding {
            keys: vec![key(KeyCode::Char('x'))],
            help_key: "x".to_string(),
            help_desc: "stop task".to_string(),
        },
        session: KeyBinding {
            keys: vec![key(KeyCode::Char('s'))],
            help_key: "s".to_string(),
            help_desc: "start session".to_string(),
        },
        follow: KeyBinding {
            keys: vec![key(KeyCode::Char('f'))],
            help_key: "f".to_string(),
            help_desc: "live tail".to_string(),
        },
        insights: KeyBinding {
            keys: vec![key(KeyCode::Char('i'))],
            help_key: "i".to_string(),
            help_desc: "Insights query".to_string(),
        },
        profile: KeyBinding {
            keys: vec![key(KeyCode::Char('p'))],
            help_key: "p".to_string(),
            help_desc: "switch profile".to_string(),
        },
        sso_login: KeyBinding {
            keys: vec![key_shift(KeyCode::Char('L'))],
            help_key: "L".to_string(),
            help_desc: "SSO login".to_string(),
        },
        yank: KeyBinding {
            keys: vec![key(KeyCode::Char('y'))],
            help_key: "y".to_string(),
            help_desc: "copy ARN".to_string(),
        },
        top: KeyBinding {
            keys: vec![key(KeyCode::Char('g'))],
            help_key: "g".to_string(),
            help_desc: "go to top".to_string(),
        },
        bottom: KeyBinding {
            keys: vec![key_shift(KeyCode::Char('G'))],
            help_key: "G".to_string(),
            help_desc: "go to bottom".to_string(),
        },
        export: KeyBinding {
            keys: vec![key_shift(KeyCode::Char('S'))],
            help_key: "S".to_string(),
            help_desc: "export logs".to_string(),
        },
        tab_s3: KeyBinding {
            keys: vec![key(KeyCode::Char('6'))],
            help_key: "6".to_string(),
            help_desc: "S3".to_string(),
        },
        download: KeyBinding {
            keys: vec![key(KeyCode::Char('d'))],
            help_key: "d".to_string(),
            help_desc: "download".to_string(),
        },
        upload: KeyBinding {
            keys: vec![key(KeyCode::Char('u'))],
            help_key: "u".to_string(),
            help_desc: "upload".to_string(),
        },
        delete_object: KeyBinding {
            keys: vec![key(KeyCode::Char('x'))],
            help_key: "x".to_string(),
            help_desc: "delete object".to_string(),
        },
        sort: KeyBinding {
            keys: vec![key(KeyCode::Char('s'))],
            help_key: "s".to_string(),
            help_desc: "cycle sort".to_string(),
        },
        sql_modify: KeyBinding {
            keys: vec![key(KeyCode::Char('e'))],
            help_key: "e".to_string(),
            help_desc: "execute modify".to_string(),
        },
        resize_mode: KeyBinding {
            keys: vec![key_ctrl(KeyCode::Char('r'))],
            help_key: "Ctrl+r".to_string(),
            help_desc: "resize mode".to_string(),
        },
        layout_toggle: KeyBinding {
            keys: vec![key_ctrl(KeyCode::Char('v'))],
            help_key: "Ctrl+v".to_string(),
            help_desc: "toggle layout".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_key_map_all_bindings_set() {
        let km = default_key_map();

        let bindings: Vec<(&str, &KeyBinding)> = vec![
            ("Quit", &km.quit),
            ("Help", &km.help),
            ("TabEcs", &km.tab_ecs),
            ("TabTasks", &km.tab_tasks),
            ("TabSsm", &km.tab_ssm),
            ("TabLogs", &km.tab_logs),
            ("TabRds", &km.tab_rds),
            ("NextTab", &km.next_tab),
            ("PrevTab", &km.prev_tab),
            ("Up", &km.up),
            ("Down", &km.down),
            ("Enter", &km.enter),
            ("Back", &km.back),
            ("Search", &km.search),
            ("Escape", &km.escape),
            ("Refresh", &km.refresh),
            ("Exec", &km.exec),
            ("Logs", &km.logs),
            ("ForceDeploy", &km.force_deploy),
            ("StopTask", &km.stop_task),
            ("Session", &km.session),
            ("Follow", &km.follow),
            ("Insights", &km.insights),
            ("Profile", &km.profile),
            ("SsoLogin", &km.sso_login),
            ("Yank", &km.yank),
            ("Top", &km.top),
            ("Bottom", &km.bottom),
            ("Export", &km.export),
            ("TabS3", &km.tab_s3),
            ("Download", &km.download),
            ("Upload", &km.upload),
            ("DeleteObject", &km.delete_object),
            ("Sort", &km.sort),
            ("SqlModify", &km.sql_modify),
            ("ResizeMode", &km.resize_mode),
            ("LayoutToggle", &km.layout_toggle),
        ];

        for (name, binding) in bindings {
            assert!(
                !binding.keys.is_empty(),
                "binding {name} should have at least one key"
            );
        }
    }

    #[test]
    fn default_key_map_quit_keys() {
        let km = default_key_map();
        assert!(km.quit.matches(&key(KeyCode::Char('q'))));
        assert!(km.quit.matches(&key_ctrl(KeyCode::Char('c'))));
    }
}
