use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

/// Strips carriage returns from PTY output while preserving ANSI color codes.
fn strip_cr(s: &str) -> String {
    s.replace('\r', "")
}

/// StreamLine carries either a line of output or a final signal.
#[derive(Debug)]
pub struct StreamLine {
    pub text: String,
    pub err: Option<String>,
    pub done: bool,
}

/// StreamHandle holds a stream receiver and the child PID for kill support.
pub struct StreamHandle {
    pub rx: mpsc::Receiver<StreamLine>,
    pub child_pid: Option<u32>,
}

/// Kills a process by PID using SIGTERM.
pub fn kill_process(pid: u32) {
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// RunResult holds the output of a synchronous command.
pub struct RunResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

/// Executor abstracts running AWS CLI commands for testability.
pub trait Executor: Send + Sync {
    fn run(&self, args: &[&str]) -> Result<RunResult, String>;
    fn stream(&self, args: &[&str]) -> Result<StreamHandle, String>;
    fn bin(&self) -> String;
    fn look_path(&self) -> String;
    fn profile(&self) -> String;
    fn region(&self) -> String;
}

/// RealExecutor runs actual AWS CLI commands via std::process::Command.
pub struct RealExecutor {
    cmd: String,
    prefix: Vec<String>,
    aws_profile: String,
    aws_region: String,
}

impl RealExecutor {
    pub fn new(bin: &str, profile: &str, region: &str) -> Self {
        let parts = split_command(bin);
        let cmd = parts[0].clone();
        let prefix = parts[1..].to_vec();
        RealExecutor {
            cmd,
            prefix,
            aws_profile: profile.to_string(),
            aws_region: region.to_string(),
        }
    }

    fn build_args(&self, args: &[&str]) -> Vec<String> {
        let mut result: Vec<String> = self.prefix.clone();
        result.extend(args.iter().map(|s| s.to_string()));
        result.push("--output".to_string());
        result.push("json".to_string());
        result.push("--profile".to_string());
        result.push(self.aws_profile.clone());
        result.push("--region".to_string());
        result.push(self.aws_region.clone());
        result.push("--no-paginate".to_string());
        result
    }

    fn build_stream_args(&self, args: &[&str]) -> Vec<String> {
        let mut result: Vec<String> = self.prefix.clone();
        result.extend(args.iter().map(|s| s.to_string()));
        result.push("--profile".to_string());
        result.push(self.aws_profile.clone());
        result.push("--region".to_string());
        result.push(self.aws_region.clone());
        result
    }
}

impl Executor for RealExecutor {
    fn run(&self, args: &[&str]) -> Result<RunResult, String> {
        let full_args = self.build_args(args);
        log::debug!("run: {} {}", self.cmd, full_args.join(" "));
        let output = Command::new(&self.cmd)
            .args(&full_args)
            .output()
            .map_err(|e| e.to_string())?;

        let exit_code = output.status.code().unwrap_or(-1);

        Ok(RunResult {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code,
        })
    }

    fn stream(&self, args: &[&str]) -> Result<StreamHandle, String> {
        let full_args = self.build_stream_args(args);
        log::debug!("stream: {} {}", self.cmd, full_args.join(" "));

        // Use `script` to allocate a PTY so AWS CLI flushes output in real-time.
        let inner_cmd = std::iter::once(self.cmd.as_str())
            .chain(full_args.iter().map(|s| s.as_str()))
            .map(shell_escape)
            .collect::<Vec<_>>()
            .join(" ");

        let mut child = Command::new("script")
            .args(["-qefc", &inner_cmd, "/dev/null"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;

        let child_pid = child.id();
        let stdout = child.stdout.take().ok_or("failed to capture stdout")?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        let clean = strip_cr(&text);
                        if clean.is_empty() {
                            continue;
                        }
                        let _ = tx.send(StreamLine {
                            text: clean,
                            err: None,
                            done: false,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(StreamLine {
                            text: String::new(),
                            err: Some(e.to_string()),
                            done: true,
                        });
                        return;
                    }
                }
            }

            let status = child.wait();
            match status {
                Ok(s) if s.success() || s.code() == Some(1) => {
                    let _ = tx.send(StreamLine {
                        text: String::new(),
                        err: None,
                        done: true,
                    });
                }
                Ok(s) => {
                    let _ = tx.send(StreamLine {
                        text: String::new(),
                        err: Some(format!("exit code {}", s.code().unwrap_or(-1))),
                        done: true,
                    });
                }
                Err(e) => {
                    let _ = tx.send(StreamLine {
                        text: String::new(),
                        err: Some(e.to_string()),
                        done: true,
                    });
                }
            }
        });

        Ok(StreamHandle {
            rx,
            child_pid: Some(child_pid),
        })
    }

    fn bin(&self) -> String {
        let mut parts = vec![self.cmd.clone()];
        parts.extend(self.prefix.clone());
        parts.join(" ")
    }

    fn look_path(&self) -> String {
        which::which(&self.cmd)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| self.cmd.clone())
    }

    fn profile(&self) -> String {
        self.aws_profile.clone()
    }

    fn region(&self) -> String {
        self.aws_region.clone()
    }
}

/// Escapes a string for safe use in a shell command.
fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || "-_./=:@^".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Splits a command string into parts, respecting quoted strings.
pub fn split_command(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for c in s.chars() {
        match in_quote {
            Some(q) => {
                if c == q {
                    in_quote = None;
                } else {
                    current.push(c);
                }
            }
            None => match c {
                '\'' | '"' => {
                    in_quote = Some(c);
                }
                ' ' | '\t' => {
                    if !current.is_empty() {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                _ => {
                    current.push(c);
                }
            },
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        vec!["aws".to_string()]
    } else {
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_simple() {
        let parts = split_command("aws");
        assert_eq!(parts, vec!["aws"]);
    }

    #[test]
    fn split_with_prefix() {
        let parts = split_command("docker exec php aws");
        assert_eq!(parts, vec!["docker", "exec", "php", "aws"]);
    }

    #[test]
    fn split_quoted() {
        let parts = split_command("ssh 'remote host' aws");
        assert_eq!(parts, vec!["ssh", "remote host", "aws"]);
    }

    #[test]
    fn split_empty() {
        let parts = split_command("");
        assert_eq!(parts, vec!["aws"]);
    }

    #[test]
    fn strip_cr_removes_carriage_returns() {
        assert_eq!(strip_cr("hello\r\nworld"), "hello\nworld");
        assert_eq!(strip_cr("no cr"), "no cr");
    }
}
