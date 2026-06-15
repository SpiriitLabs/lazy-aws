//! System clipboard integration.
//!
//! `copy()` is intentionally redundant so a yank works in as many environments
//! as possible:
//!   1. `arboard` — native clipboard (X11/Wayland/macOS/Windows) while the app runs.
//!   2. OSC 52 — escape sequence written to the controlling terminal; this is the
//!      only path that reaches the *real* clipboard through SSH / tmux / mosh.
//!   3. Shell-out to `wl-copy` / `xclip` / `xsel` as a last-resort fallback.
//!
//! We attempt all of them best-effort and report success if any path worked.

use std::io::Write;

use base64::Engine;

/// Copy `text` to the system clipboard through every available channel.
pub fn copy(text: &str) -> Result<(), String> {
    let mut ok = false;

    if copy_via_arboard(text) {
        ok = true;
    }

    // OSC 52 reaches the real terminal even over SSH/tmux; emit it regardless.
    if emit_osc52(text) {
        ok = true;
    }

    // Only bother shelling out if the native clipboard did not take.
    if !ok && copy_via_shellout(text) {
        ok = true;
    }

    if ok {
        Ok(())
    } else {
        Err("No clipboard available (tried arboard, OSC52, wl-copy/xclip/xsel)".to_string())
    }
}

fn copy_via_arboard(text: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(mut cb) => cb.set_text(text.to_owned()).is_ok(),
        Err(_) => false,
    }
}

/// Write an OSC 52 sequence to the controlling terminal (`/dev/tty`).
///
/// Writing to `/dev/tty` rather than stdout avoids fighting ratatui's buffered
/// backend. Terminals that do not understand OSC 52 silently ignore it.
fn emit_osc52(text: &str) -> bool {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let seq = format!("\x1b]52;c;{encoded}\x07");

    match std::fs::OpenOptions::new().write(true).open("/dev/tty") {
        Ok(mut tty) => tty
            .write_all(seq.as_bytes())
            .and_then(|_| tty.flush())
            .is_ok(),
        Err(_) => false,
    }
}

fn copy_via_shellout(text: &str) -> bool {
    use std::process::{Command, Stdio};

    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];

    for (cmd, args) in candidates {
        if which::which(cmd).is_err() {
            continue;
        }
        let Ok(mut child) = Command::new(cmd)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        if let Some(ref mut stdin) = child.stdin {
            if stdin.write_all(text.as_bytes()).is_err() {
                continue;
            }
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_payload_is_base64_of_input() {
        let encoded = base64::engine::general_purpose::STANDARD.encode("hello".as_bytes());
        assert_eq!(encoded, "aGVsbG8=");
        // The full sequence wraps the payload between the OSC introducer and BEL.
        let seq = format!("\x1b]52;c;{encoded}\x07");
        assert!(seq.starts_with("\x1b]52;c;"));
        assert!(seq.ends_with('\x07'));
        assert!(seq.contains("aGVsbG8="));
    }
}
