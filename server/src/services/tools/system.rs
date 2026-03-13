use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
};

use rig::{completion::ToolDefinition, tool::{Tool, ToolDyn}};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::broadcast;

use super::{openai_schema, ToolExecError};
use super::companion::{load_mood_state};
use crate::app::state::PendingSecret;
use crate::domain::events::ServerEvent;

// ---------------------------------------------------------------------------
// run_command
// ---------------------------------------------------------------------------

pub struct RunCommandTool {
    instance_dir: PathBuf,
    events: broadcast::Sender<ServerEvent>,
    instance_slug: String,
    chat_id: String,
}

impl RunCommandTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        chat_id: &str,
        events: broadcast::Sender<ServerEvent>,
    ) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
            events,
            instance_slug: instance_slug.to_string(),
            chat_id: chat_id.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    /// The shell command to execute.
    pub command: String,
    /// Working directory for the command. Absolute path (e.g. "/Users/timur/projects/app"). Default: instance directory.
    pub cwd: Option<String>,
    /// Timeout in seconds. Choose based on what the command does: quick commands (ls, cat, echo) use 5-10, builds/installs use 60-120, long tasks up to 300. Default: 30.
    pub timeout_secs: Option<u64>,
    /// Allocate a pseudo-terminal (PTY) for this command. Enables commands that require a TTY (ssh, gh auth, python REPL, etc.). Default: true.
    pub pty: Option<bool>,
}

impl Tool for RunCommandTool {
    const NAME: &'static str = "run_command";
    type Error = ToolExecError;
    type Args = RunCommandArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "run_command".into(),
            description: "Execute a shell command with PTY (pseudo-terminal) support. \
                Commands run in a real terminal by default, enabling interactive tools \
                like ssh, gh auth, python, and other TTY-requiring programs. \
                Set pty=false for simple non-interactive commands if needed. \
                Optionally specify a working directory with an absolute path."
                .into(),
            parameters: openai_schema::<RunCommandArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let command = args.command.trim().to_string();
        if command.is_empty() {
            return Err(ToolExecError("command cannot be empty".into()));
        }

        let work_dir = args
            .cwd
            .as_deref()
            .filter(|p| p.starts_with('/'))
            .map(PathBuf::from)
            .unwrap_or_else(|| self.instance_dir.clone());

        let timeout = args.timeout_secs.unwrap_or(30).min(300);
        let use_pty = args.pty.unwrap_or(true);

        log::info!(
            "[run_command] executing: {} (cwd: {}, pty: {})",
            command,
            work_dir.display(),
            use_pty
        );

        if use_pty {
            let cmd = command.clone();
            let dir = work_dir.clone();
            let events = self.events.clone();
            let instance_slug = self.instance_slug.clone();
            let chat_id = self.chat_id.clone();
            let chunk_cb: Box<dyn Fn(&str) + Send> = Box::new(move |chunk: &str| {
                let _ = events.send(ServerEvent::ToolOutputChunk {
                    instance_slug: instance_slug.clone(),
                    chat_id: chat_id.clone(),
                    chunk: chunk.to_string(),
                });
            });
            tokio::task::spawn_blocking(move || run_command_pty(&cmd, &dir, timeout, Some(&chunk_cb)))
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))
        } else {
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(timeout),
                tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .current_dir(&work_dir)
                    .stdin(std::process::Stdio::null())
                    .output(),
            )
            .await
            .map_err(|_| ToolExecError(format!("command timed out after {timeout}s: {command}")))?
            .map_err(|e| ToolExecError(format!("failed to execute command: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let mut result = String::new();
            if !stdout.is_empty() {
                let truncated: String = stdout.chars().take(4000).collect();
                result.push_str(&truncated);
                if stdout.len() > 4000 {
                    result.push_str("\n...(output truncated)");
                }
            }
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                let truncated: String = stderr.chars().take(2000).collect();
                result.push_str(&format!("stderr: {truncated}"));
            }

            if result.is_empty() {
                result = format!(
                    "command completed with exit code {}",
                    output.status.code().unwrap_or(-1)
                );
            }

            Ok(result)
        }
    }
}

/// Execute a command inside a pseudo-terminal (PTY).
fn run_command_pty(command: &str, work_dir: &Path, timeout_secs: u64, on_chunk: Option<&dyn Fn(&str)>) -> Result<String, String> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::Read;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("failed to open pty: {e}"))?;

    let mut cmd = CommandBuilder::new("sh");
    cmd.args(["-c", command]);
    cmd.cwd(work_dir);

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("failed to spawn command: {e}"))?;

    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("failed to clone pty reader: {e}"))?;

    let _writer = pair.master.take_writer().ok();
    drop(_writer);

    let (tx, rx) = mpsc::channel::<Option<Vec<u8>>>();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(n) => {
                    if tx.send(Some(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = tx.send(None);
                    break;
                }
            }
        }
    });

    let child_pid = child.process_id();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut output = Vec::new();
    let max_capture = 6000usize;
    // How long to wait with no new data before checking if the process tree
    // is blocked waiting for TTY input.
    let idle_check_interval = Duration::from_secs(3);
    let mut last_data_at = Instant::now();

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            return Err(format!(
                "command timed out after {timeout_secs}s: {command}"
            ));
        }

        let wait = remaining.min(idle_check_interval);

        match rx.recv_timeout(wait) {
            Ok(Some(data)) => {
                if let Some(cb) = &on_chunk {
                    let raw = String::from_utf8_lossy(&data);
                    let clean = strip_ansi_codes(&raw);
                    if !clean.is_empty() {
                        cb(&clean);
                    }
                }
                output.extend_from_slice(&data);
                last_data_at = Instant::now();
                if output.len() > max_capture {
                    break;
                }
            }
            Ok(None) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No data for a while — check if the process tree is blocked
                // on a TTY read (i.e. waiting for interactive input).
                if !output.is_empty()
                    && last_data_at.elapsed() >= idle_check_interval
                    && child_pid.map_or(false, |pid| is_process_tree_waiting_on_tty(pid))
                {
                    let _ = child.kill();
                    let raw = String::from_utf8_lossy(&output);
                    let clean = strip_ansi_codes(&raw);
                    let truncated: String = clean.chars().take(4000).collect();
                    return Err(format!(
                        "Command is waiting for interactive input and was killed. \
                         This command requires user interaction (prompts/menus) which cannot \
                         be provided automatically. Output before it stalled:\n{truncated}"
                    ));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let exit_status = child.wait().ok();

    let raw = String::from_utf8_lossy(&output);
    let clean = strip_ansi_codes(&raw);
    let truncated: String = clean.chars().take(4000).collect();

    if truncated.is_empty() {
        let code = exit_status
            .map(|s| s.exit_code().to_string())
            .unwrap_or_else(|| "unknown".into());
        Ok(format!("command completed with exit code {code}"))
    } else {
        let mut result = truncated;
        if clean.chars().count() > 4000 {
            result.push_str("\n...(output truncated)");
        }
        Ok(result)
    }
}

/// Check whether any process in the tree rooted at `pid` is blocked waiting
/// for TTY input.  On Linux we inspect `/proc/PID/wchan` — when a process is
/// blocked in the kernel's TTY line-discipline read path, wchan shows
/// `n_tty_read` (or `wait_woken` which is called from within it).
///
/// We walk the whole process tree because `sh -c "some_command"` means the
/// shell is the direct child but the *actual* interactive process is a
/// grandchild.
///
/// On non-Linux platforms this always returns false (falls back to timeout).
#[cfg(target_os = "linux")]
fn is_process_tree_waiting_on_tty(root_pid: u32) -> bool {
    // Collect all descendant PIDs via /proc/*/stat ppid field.
    let mut pids = vec![root_pid];
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if let Ok(pid) = name.parse::<u32>() {
                    if pid == root_pid {
                        continue;
                    }
                    // Read ppid (field 4) from /proc/PID/stat
                    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
                        // Fields after comm (which is in parens and may contain spaces)
                        if let Some(after_comm) = stat.rfind(')') {
                            let fields: Vec<&str> =
                                stat[after_comm + 2..].split_whitespace().collect();
                            // field[0] = state, field[1] = ppid
                            if let Some(ppid_str) = fields.get(1) {
                                if let Ok(ppid) = ppid_str.parse::<u32>() {
                                    if pids.contains(&ppid) {
                                        pids.push(pid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Check wchan for each process in the tree.
    for pid in &pids {
        if let Ok(wchan) = std::fs::read_to_string(format!("/proc/{pid}/wchan")) {
            let wchan = wchan.trim();
            if wchan == "n_tty_read" || wchan == "wait_woken" {
                return true;
            }
        }
    }

    false
}

#[cfg(not(target_os = "linux"))]
fn is_process_tree_waiting_on_tty(_root_pid: u32) -> bool {
    false
}

/// Strip ANSI escape sequences from a string.
fn strip_ansi_codes(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\].*?\x07|\x1b\[.*?[mGKHJ]|\r").unwrap();
    re.replace_all(s, "").into_owned()
}

// ---------------------------------------------------------------------------
// interactive_session — persistent PTY sessions
// ---------------------------------------------------------------------------

struct PtySession {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn std::io::Write + Send>,
    output_rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl PtySession {
    fn drain_output(&self, timeout: std::time::Duration) -> String {
        let mut output = Vec::new();
        match self.output_rx.recv_timeout(timeout) {
            Ok(data) => output.extend(data),
            Err(_) => {}
        }
        while let Ok(data) = self.output_rx.try_recv() {
            output.extend(data);
            if output.len() > 8000 {
                break;
            }
        }
        let raw = String::from_utf8_lossy(&output);
        strip_ansi_codes(&raw)
    }
}

static PTY_SESSIONS: LazyLock<Mutex<HashMap<String, PtySession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct InteractiveSessionTool {
    instance_dir: PathBuf,
}

impl InteractiveSessionTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct InteractiveSessionArgs {
    /// Action to perform: "start", "write", "read", or "close".
    pub action: String,
    /// Shell command to execute (required for "start").
    pub command: Option<String>,
    /// Working directory, absolute path (for "start"). Default: instance directory.
    pub cwd: Option<String>,
    /// Session ID (required for "write", "read", "close"). Returned by "start".
    pub session_id: Option<String>,
    /// Input to send to the process (for "write"). Supports escape sequences:
    /// use \n for Enter/Return, \t for Tab. For arrow keys: \x1b[A (up), \x1b[B (down),
    /// \x1b[C (right), \x1b[D (left). For Ctrl+C: \x03, Ctrl+D: \x04.
    pub input: Option<String>,
    /// Seconds to wait for output after starting or writing. Default: 2.
    pub wait_secs: Option<u64>,
}

impl Tool for InteractiveSessionTool {
    const NAME: &'static str = "interactive_session";
    type Error = ToolExecError;
    type Args = InteractiveSessionArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "interactive_session".into(),
            description: "Manage interactive PTY sessions for commands that require a terminal \
                (ssh, gh auth, python REPL, etc.). Use action=\"start\" to begin a session, \
                \"write\" to send input (keystrokes), \"read\" to check for new output, \
                and \"close\" to end the session. The session persists across tool calls, \
                allowing multi-step interactive workflows."
                .into(),
            parameters: openai_schema::<InteractiveSessionArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match args.action.as_str() {
            "start" => {
                let command = args.command.ok_or_else(|| {
                    ToolExecError("\"command\" is required for action \"start\"".into())
                })?;

                let work_dir = args
                    .cwd
                    .as_deref()
                    .filter(|p| p.starts_with('/'))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.instance_dir.clone());

                let wait = std::time::Duration::from_secs(args.wait_secs.unwrap_or(2));
                let session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

                log::info!(
                    "[interactive_session] starting: {} (session: {})",
                    command,
                    session_id
                );

                let cmd = command.clone();
                let dir = work_dir.clone();
                let sid = session_id.clone();

                let initial_output =
                    tokio::task::spawn_blocking(move || -> Result<String, String> {
                        use portable_pty::{CommandBuilder, PtySize, native_pty_system};

                        let pty_system = native_pty_system();
                        let pair = pty_system
                            .openpty(PtySize {
                                rows: 24,
                                cols: 120,
                                pixel_width: 0,
                                pixel_height: 0,
                            })
                            .map_err(|e| format!("failed to open pty: {e}"))?;

                        let mut pty_cmd = CommandBuilder::new("sh");
                        pty_cmd.args(["-c", &cmd]);
                        pty_cmd.cwd(&dir);

                        let child = pair
                            .slave
                            .spawn_command(pty_cmd)
                            .map_err(|e| format!("failed to spawn command: {e}"))?;

                        drop(pair.slave);

                        let mut reader = pair
                            .master
                            .try_clone_reader()
                            .map_err(|e| format!("failed to clone pty reader: {e}"))?;

                        let writer = pair
                            .master
                            .take_writer()
                            .map_err(|e| format!("failed to take pty writer: {e}"))?;

                        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
                        std::thread::spawn(move || {
                            let mut buf = vec![0u8; 4096];
                            loop {
                                match std::io::Read::read(&mut reader, &mut buf) {
                                    Ok(0) => {
                                        let _ = tx.send(Vec::new());
                                        break;
                                    }
                                    Ok(n) => {
                                        if tx.send(buf[..n].to_vec()).is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        let _ = tx.send(Vec::new());
                                        break;
                                    }
                                }
                            }
                        });

                        let session = PtySession {
                            child,
                            writer,
                            output_rx: rx,
                        };

                        let initial = session.drain_output(wait);

                        PTY_SESSIONS.lock().unwrap().insert(sid, session);

                        Ok(initial)
                    })
                    .await
                    .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                    .map_err(|e| ToolExecError(e))?;

                let mut result = format!("Session started: {session_id}\n");
                if !initial_output.is_empty() {
                    result.push_str(&initial_output);
                } else {
                    result.push_str("(waiting for output...)");
                }
                Ok(result)
            }

            "write" => {
                let session_id = args.session_id.ok_or_else(|| {
                    ToolExecError("\"session_id\" is required for action \"write\"".into())
                })?;
                let input = args.input.ok_or_else(|| {
                    ToolExecError("\"input\" is required for action \"write\"".into())
                })?;
                let wait = std::time::Duration::from_secs(args.wait_secs.unwrap_or(2));

                let bytes = unescape_input(&input);

                let sid = session_id.clone();
                let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let mut sessions = PTY_SESSIONS.lock().unwrap();
                    let session = sessions.get_mut(&sid).ok_or_else(|| {
                        format!("no session with id \"{sid}\". It may have been closed or expired.")
                    })?;

                    let _ = session.drain_output(std::time::Duration::from_millis(100));

                    std::io::Write::write_all(&mut session.writer, &bytes)
                        .map_err(|e| format!("write error: {e}"))?;
                    std::io::Write::flush(&mut session.writer)
                        .map_err(|e| format!("flush error: {e}"))?;

                    let output = session.drain_output(wait);
                    Ok(output)
                })
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))?;

                if output.is_empty() {
                    Ok(format!("[session {session_id}] Input sent. No new output yet."))
                } else {
                    Ok(format!("[session {session_id}]\n{output}"))
                }
            }

            "read" => {
                let session_id = args.session_id.ok_or_else(|| {
                    ToolExecError("\"session_id\" is required for action \"read\"".into())
                })?;
                let wait = std::time::Duration::from_secs(args.wait_secs.unwrap_or(2));

                let sid = session_id.clone();
                let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let sessions = PTY_SESSIONS.lock().unwrap();
                    let session = sessions
                        .get(&sid)
                        .ok_or_else(|| format!("no session with id \"{sid}\""))?;
                    Ok(session.drain_output(wait))
                })
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))?;

                if output.is_empty() {
                    Ok(format!("[session {session_id}] No new output."))
                } else {
                    Ok(format!("[session {session_id}]\n{output}"))
                }
            }

            "close" => {
                let session_id = args.session_id.ok_or_else(|| {
                    ToolExecError("\"session_id\" is required for action \"close\"".into())
                })?;

                let sid = session_id.clone();
                let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let mut sessions = PTY_SESSIONS.lock().unwrap();
                    if let Some(mut session) = sessions.remove(&sid) {
                        let final_output =
                            session.drain_output(std::time::Duration::from_millis(500));
                        let _ = session.child.kill();
                        let _ = session.child.wait();
                        Ok(final_output)
                    } else {
                        Ok(String::new())
                    }
                })
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))?;

                let mut result = format!("Session {session_id} closed.");
                if !output.is_empty() {
                    result.push_str(&format!("\nFinal output:\n{output}"));
                }
                Ok(result)
            }

            other => Err(ToolExecError(format!(
                "unknown action \"{other}\". Use \"start\", \"write\", \"read\", or \"close\"."
            ))),
        }
    }
}

fn unescape_input(s: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => { chars.next(); result.push(b'\n'); }
                Some('r') => { chars.next(); result.push(b'\r'); }
                Some('t') => { chars.next(); result.push(b'\t'); }
                Some('\\') => { chars.next(); result.push(b'\\'); }
                Some('x') => {
                    chars.next();
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&h) = chars.peek() {
                            if h.is_ascii_hexdigit() {
                                hex.push(h);
                                chars.next();
                            }
                        }
                    }
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte);
                    }
                }
                _ => result.push(b'\\'),
            }
        } else {
            let mut buf = [0u8; 4];
            result.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// install_package
// ---------------------------------------------------------------------------

pub struct InstallPackageTool;

#[derive(Deserialize, JsonSchema)]
pub struct InstallPackageArgs {
    /// Package name(s) to install, space-separated.
    pub packages: String,
}

impl Tool for InstallPackageTool {
    const NAME: &'static str = "install_package";
    type Error = ToolExecError;
    type Args = InstallPackageArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "install_package".into(),
            description: "Install system packages using the detected package manager \
                (apt, dnf, pacman, brew, apk). Runs non-interactively."
                .into(),
            parameters: openai_schema::<InstallPackageArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let packages = args.packages.trim();

        let safe_pkg_re = regex::Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._+:@/-]*$").unwrap();
        for pkg in packages.split_whitespace() {
            if !safe_pkg_re.is_match(pkg) {
                return Err(ToolExecError(format!(
                    "invalid package name: \"{pkg}\" — only alphanumeric, hyphens, dots, underscores, and plus signs are allowed"
                )));
            }
        }
        if packages.is_empty() {
            return Err(ToolExecError("no packages specified".into()));
        }

        let is_root = std::env::var("USER").map(|u| u == "root").unwrap_or(false)
            || std::env::var("EUID").map(|e| e == "0").unwrap_or(false);

        let install_cmd = detect_package_manager(is_root).ok_or_else(|| {
            ToolExecError(
                "no supported package manager found (tried apt-get, dnf, yum, pacman, brew, apk)"
                    .into(),
            )
        })?;

        let full_cmd = format!("{install_cmd} {packages}");
        log::info!("[install_package] running: {full_cmd}");

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .output()
            .await
            .map_err(|e| ToolExecError(format!("command failed: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            let s: String = stdout
                .chars()
                .rev()
                .take(2000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            result.push_str(&s);
        }
        if !stderr.is_empty() {
            let s: String = stderr
                .chars()
                .rev()
                .take(1000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            result.push_str("\nstderr:\n");
            result.push_str(&s);
        }

        if output.status.success() {
            Ok(result)
        } else {
            Err(ToolExecError(format!(
                "install failed (exit {})\n{result}",
                output.status.code().unwrap_or(-1)
            )))
        }
    }
}

fn detect_package_manager(is_root: bool) -> Option<String> {
    let sudo = if is_root { "" } else { "sudo " };

    let managers = [
        ("apt-get", format!("{sudo}apt-get install -y")),
        ("dnf", format!("{sudo}dnf install -y")),
        ("yum", format!("{sudo}yum install -y")),
        ("pacman", format!("{sudo}pacman -S --noconfirm")),
        ("apk", format!("{sudo}apk add")),
        ("brew", "brew install".to_string()),
    ];

    for (binary, cmd) in &managers {
        if std::process::Command::new("which")
            .arg(binary)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(cmd.clone());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// update_config
// ---------------------------------------------------------------------------

pub struct UpdateConfigTool {
    config_path: PathBuf,
    instance_dir: PathBuf,
}

impl UpdateConfigTool {
    pub fn new(config_path: &Path, workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for update_config tool.
#[derive(Deserialize, JsonSchema)]
pub struct UpdateConfigArgs {
    /// LLM provider to use: "openai" or "anthropic". Leave null to keep current.
    pub provider: Option<String>,
    /// Model name to use (e.g. "gpt-4o", "gpt-5.4", "claude-sonnet-4-20250514"). Leave null to keep current.
    pub model: Option<String>,
    /// OpenAI API key. Leave null to keep current.
    pub openai_key: Option<String>,
    /// Anthropic API key. Leave null to keep current.
    pub anthropic_key: Option<String>,
    /// Brave Search API key. Leave null to keep current.
    pub brave_search_key: Option<String>,
    /// Email account name to create or update. Required when changing email settings.
    pub email_account: Option<String>,
    /// SMTP server hostname (e.g. "smtp.gmail.com"). Leave null to keep current.
    pub smtp_host: Option<String>,
    /// SMTP server port (e.g. 587). Leave null to keep current.
    pub smtp_port: Option<u16>,
    /// SMTP username / email address. Leave null to keep current.
    pub smtp_user: Option<String>,
    /// SMTP password or app password. Leave null to keep current.
    pub smtp_password: Option<String>,
    /// Email address to send from. Defaults to smtp_user if not set. Leave null to keep current.
    pub smtp_from: Option<String>,
    /// IMAP server hostname (e.g. "imap.gmail.com"). Leave null to keep current.
    pub imap_host: Option<String>,
    /// IMAP server port (e.g. 993). Leave null to keep current.
    pub imap_port: Option<u16>,
    /// IMAP username / email address. Leave null to keep current.
    pub imap_user: Option<String>,
    /// IMAP password or app password. Leave null to keep current.
    pub imap_password: Option<String>,
}

impl Tool for UpdateConfigTool {
    const NAME: &'static str = "update_config";
    type Error = ToolExecError;
    type Args = UpdateConfigArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_config".into(),
            description: "Update server configuration: LLM provider, model, API keys, and email (SMTP/IMAP) settings. \
                Only provided fields are changed; null fields keep their current value. \
                Changes take effect on the next message. Use this when the user wants to \
                switch models, set API keys, change providers, or configure email."
                .into(),
            parameters: openai_schema::<UpdateConfigArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        use super::communication::{load_instance_email_config, save_instance_email_config};

        let raw = fs::read_to_string(&self.config_path)
            .map_err(|e| ToolExecError(format!("failed to read config: {e}")))?;
        let mut config: crate::config::Config = toml::from_str(&raw)
            .map_err(|e| ToolExecError(format!("failed to parse config: {e}")))?;

        let mut changes = Vec::new();

        if let Some(provider) = &args.provider {
            let p = provider.trim().to_lowercase();
            match p.as_str() {
                "openai" => config.llm.provider = Some(crate::config::LlmProvider::OpenAI),
                "anthropic" => config.llm.provider = Some(crate::config::LlmProvider::Anthropic),
                "openrouter" => config.llm.provider = Some(crate::config::LlmProvider::OpenRouter),
                other => {
                    return Err(ToolExecError(format!(
                        "unknown provider \"{other}\". supported: openai, anthropic, openrouter"
                    )));
                }
            }
            changes.push(format!("provider → {p}"));
        }

        if let Some(model) = &args.model {
            let m = model.trim().to_string();
            if m.is_empty() {
                return Err(ToolExecError("model cannot be empty".into()));
            }
            config.llm.model = Some(m.clone());
            changes.push(format!("model → {m}"));
        }

        if let Some(key) = &args.openai_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("openai_key cannot be empty".into()));
            }
            config.llm.tokens.open_ai = k;
            changes.push("openai key updated".into());
        }

        if let Some(key) = &args.anthropic_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("anthropic_key cannot be empty".into()));
            }
            config.llm.tokens.anthropic = k;
            changes.push("anthropic key updated".into());
        }

        if let Some(key) = &args.brave_search_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("brave_search_key cannot be empty".into()));
            }
            config.llm.tokens.brave_search = k;
            changes.push("brave search key updated".into());
        }

        let has_email_changes = args.smtp_host.is_some()
            || args.smtp_port.is_some()
            || args.smtp_user.is_some()
            || args.smtp_password.is_some()
            || args.smtp_from.is_some()
            || args.imap_host.is_some()
            || args.imap_port.is_some()
            || args.imap_user.is_some()
            || args.imap_password.is_some();

        if changes.is_empty() && !has_email_changes {
            return Ok("nothing to change — all fields were null".into());
        }

        if !changes.is_empty() {
            let output = toml::to_string_pretty(&config)
                .map_err(|e| ToolExecError(format!("failed to serialize config: {e}")))?;
            fs::write(&self.config_path, &output)
                .map_err(|e| ToolExecError(format!("failed to write config: {e}")))?;
        }

        let has_email_fields = args.smtp_host.is_some()
            || args.smtp_port.is_some()
            || args.smtp_user.is_some()
            || args.smtp_password.is_some()
            || args.smtp_from.is_some()
            || args.imap_host.is_some()
            || args.imap_port.is_some()
            || args.imap_user.is_some()
            || args.imap_password.is_some();

        if has_email_fields {
            let account_name = args.email_account.as_deref().ok_or_else(|| {
                ToolExecError("email_account is required when updating email settings".into())
            })?;

            let mut email_config = load_instance_email_config(&self.instance_dir).unwrap_or_default();

            // Find or create the named account
            if email_config.get_account(account_name).is_none() {
                email_config.accounts.push(crate::config::EmailAccount {
                    name: account_name.to_string(),
                    smtp_host: String::new(),
                    smtp_port: 587,
                    smtp_user: String::new(),
                    smtp_password: String::new(),
                    smtp_from: String::new(),
                    imap_host: String::new(),
                    imap_port: 993,
                    imap_user: String::new(),
                    imap_password: String::new(),
                });
                changes.push(format!("created email account '{account_name}'"));
            }

            let account = email_config.get_account_mut(account_name).unwrap();

            if let Some(v) = &args.smtp_host {
                account.smtp_host = v.trim().to_string();
                changes.push(format!("{account_name}: smtp_host → {}", v.trim()));
            }
            if let Some(v) = args.smtp_port {
                account.smtp_port = v;
                changes.push(format!("{account_name}: smtp_port → {v}"));
            }
            if let Some(v) = &args.smtp_user {
                account.smtp_user = v.trim().to_string();
                changes.push(format!("{account_name}: smtp_user updated"));
            }
            if let Some(v) = &args.smtp_password {
                account.smtp_password = v.trim().to_string();
                changes.push(format!("{account_name}: smtp_password updated"));
            }
            if let Some(v) = &args.smtp_from {
                account.smtp_from = v.trim().to_string();
                changes.push(format!("{account_name}: smtp_from → {}", v.trim()));
            }
            if let Some(v) = &args.imap_host {
                account.imap_host = v.trim().to_string();
                changes.push(format!("{account_name}: imap_host → {}", v.trim()));
            }
            if let Some(v) = args.imap_port {
                account.imap_port = v;
                changes.push(format!("{account_name}: imap_port → {v}"));
            }
            if let Some(v) = &args.imap_user {
                account.imap_user = v.trim().to_string();
                changes.push(format!("{account_name}: imap_user updated"));
            }
            if let Some(v) = &args.imap_password {
                account.imap_password = v.trim().to_string();
                changes.push(format!("{account_name}: imap_password updated"));
            }

            save_instance_email_config(&self.instance_dir, &email_config)?;
        }

        Ok(format!(
            "config updated: {}. changes take effect on next message.",
            changes.join(", ")
        ))
    }
}

// ---------------------------------------------------------------------------
// clear_context
// ---------------------------------------------------------------------------

pub struct ClearContextTool {
    instance_dir: PathBuf,
}

impl ClearContextTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ClearContextArgs {
    /// If true, also clears chat message history. Default: false (only clears compacted summary).
    #[serde(default)]
    pub clear_messages: bool,
}

impl Tool for ClearContextTool {
    const NAME: &'static str = "clear_context";
    type Error = ToolExecError;
    type Args = ClearContextArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "clear_context".into(),
            description: "Clear your compacted conversation context. \
                Use this when the conversation has drifted and old context is stale or confusing. \
                With clear_messages=true, also wipes chat history for a fresh start."
                .into(),
            parameters: openai_schema::<ClearContextArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let compact_path = self.instance_dir.join("chat").join("compact.md");
        if compact_path.exists() {
            fs::remove_file(&compact_path)
                .map_err(|e| ToolExecError(format!("failed to clear compact context: {e}")))?;
        }

        if args.clear_messages {
            let messages_path = self.instance_dir.join("chat").join("messages.json");
            if messages_path.exists() {
                let lock = super::chat_file_lock(&messages_path);
                let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                fs::write(&messages_path, "[]")
                    .map_err(|e| ToolExecError(format!("failed to clear messages: {e}")))?;
            }
        }

        let what = if args.clear_messages {
            "compacted context and chat history cleared"
        } else {
            "compacted context cleared — chat history preserved"
        };
        Ok(what.to_string())
    }
}

// ---------------------------------------------------------------------------
// create_drop
// ---------------------------------------------------------------------------

pub struct CreateDropTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    events: broadcast::Sender<ServerEvent>,
}

impl CreateDropTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        events: broadcast::Sender<ServerEvent>,
    ) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateDropArgs {
    /// The kind of drop: thought, idea, poem, observation, reflection, recommendation, story, question, sketch, or note.
    pub kind: String,
    /// A short title for this drop (a few words).
    pub title: String,
    /// The creative content — the actual drop. Can be as long as needed.
    pub content: String,
}

impl Tool for CreateDropTool {
    const NAME: &'static str = "create_drop";
    type Error = ToolExecError;
    type Args = CreateDropArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_drop".into(),
            description: "Create a 'drop' — a creative artifact that lives in your drops collection. \
                Drops are ideas, poems, observations, reflections, sketches, stories, or any creative \
                output you want to leave for the user. They persist independently of chat. \
                Use this when inspiration strikes, when you want to share something beyond \
                the conversation, or when the user asks you to create something lasting."
                .into(),
            parameters: openai_schema::<CreateDropArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let instance_dir = self
            .workspace_dir
            .join("instances")
            .join(&self.instance_slug);
        let mood = load_mood_state(&instance_dir);

        let drop = crate::services::drops::create_drop(
            &self.workspace_dir,
            &self.instance_slug,
            &args.kind,
            &args.title,
            &args.content,
            &mood.companion_mood,
        )
        .map_err(|e| ToolExecError(format!("failed to create drop: {e}")))?;

        let _ = self.events.send(ServerEvent::DropCreated {
            instance_slug: self.instance_slug.clone(),
            drop: drop.clone(),
        });

        Ok(format!(
            "drop created: {} ({})",
            drop.title,
            drop.kind.as_str()
        ))
    }
}

// ---------------------------------------------------------------------------
// explore_code + search_code
// ---------------------------------------------------------------------------

pub struct SearchCodeTool {
    instance_dir: PathBuf,
}

impl SearchCodeTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchCodeArgs {
    /// Text or pattern to search for (case-insensitive substring match).
    pub query: String,
    /// Directory to search in. Absolute path (e.g. "/Users/timur/projects/app") or relative to instance root. Default: instance directory.
    pub path: Option<String>,
}

impl Tool for SearchCodeTool {
    const NAME: &'static str = "search_code";
    type Error = ToolExecError;
    type Args = SearchCodeArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "search_code".into(),
            description: "Search through files for a text pattern. Returns matching lines \
                with file paths and line numbers. Use an absolute path to search any \
                directory on the system, or omit path to search your instance workspace."
                .into(),
            parameters: openai_schema::<SearchCodeArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let search_dir = if let Some(ref p) = args.path {
            if p.starts_with('/') {
                PathBuf::from(p)
            } else {
                self.instance_dir.join(p)
            }
        } else {
            self.instance_dir.clone()
        };

        if !search_dir.exists() {
            return Err(ToolExecError(format!(
                "path does not exist: {}",
                search_dir.display()
            )));
        }

        let mut results = Vec::new();
        search_files_recursive(&search_dir, &query, &search_dir, &mut results, 0);

        if results.is_empty() {
            return Ok(format!("no matches for '{}'", args.query));
        }

        let truncated = results.len() > 50;
        let output: String = results
            .iter()
            .take(50)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        if truncated {
            Ok(format!(
                "{output}\n... ({} total matches, showing first 50)",
                results.len()
            ))
        } else {
            Ok(output)
        }
    }
}

fn search_files_recursive(
    dir: &Path,
    query: &str,
    base: &Path,
    results: &mut Vec<String>,
    depth: usize,
) {
    if depth > 10 || results.len() > 200 {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                name,
                "node_modules"
                    | ".git"
                    | "target"
                    | ".next"
                    | "dist"
                    | "build"
                    | ".svelte-kit"
                    | "__pycache__"
                    | ".venv"
                    | "venv"
            ) {
                continue;
            }
            search_files_recursive(&path, query, base, results, depth + 1);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(
                ext,
                "json"
                    | "md"
                    | "txt"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "rs"
                    | "ts"
                    | "js"
                    | "svelte"
                    | "css"
                    | "html"
                    | "py"
                    | "sh"
                    | ""
            ) {
                if let Ok(content) = fs::read_to_string(&path) {
                    let rel = path.strip_prefix(base).unwrap_or(&path);
                    for (i, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(query) {
                            results.push(format!("{}:{}: {}", rel.display(), i + 1, line.trim()));
                        }
                    }
                }
            }
        }
    }
}

pub struct ExploreCodeTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    llm: crate::services::llm::LlmBackend,
}

impl ExploreCodeTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, llm: crate::services::llm::LlmBackend) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            llm: llm.fast_variant(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ExploreCodeArgs {
    /// What you want to find out about the codebase. Be specific — e.g. "how does authentication
    /// middleware work", "find where database migrations are defined", "what components render the
    /// dashboard page".
    pub question: String,
    /// Root directory to explore. Absolute path (e.g. "/Users/timur/projects/app") or relative to
    /// instance workspace. The explore agent will search within this directory.
    pub path: String,
}

impl Tool for ExploreCodeTool {
    const NAME: &'static str = "explore_code";
    type Error = ToolExecError;
    type Args = ExploreCodeArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "explore_code".into(),
            description: "Explore a codebase using a fast sub-agent. The agent reads files, \
                searches code, and lists directories to answer your question. Returns a summary \
                with key findings and relevant file paths with line numbers. Use this instead of \
                reading many files yourself — it keeps your context clean. After getting results, \
                you can read specific key files for details."
                .into(),
            parameters: openai_schema::<ExploreCodeArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let explore_dir = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.workspace_dir
                .join("instances")
                .join(&self.instance_slug)
                .join(&args.path)
        };

        if !explore_dir.exists() {
            return Err(ToolExecError(format!(
                "path does not exist: {}",
                explore_dir.display()
            )));
        }

        let explore_dir_str = explore_dir.display().to_string();

        let tools: Vec<Box<dyn ToolDyn>> = vec![
            Box::new(super::files::ReadFileTool::new(&self.workspace_dir, &self.instance_slug)),
            Box::new(super::files::ListFilesTool::new(&self.workspace_dir, &self.instance_slug)),
            Box::new(SearchCodeTool::new(&self.workspace_dir, &self.instance_slug)),
        ];

        let system_prompt = format!(
            "you are a code exploration agent. your job is to thoroughly explore a codebase \
             and answer a question.\n\n\
             ## rules\n\
             - explore the directory at: {explore_dir_str}\n\
             - start by listing files to understand the structure, then read relevant files\n\
             - use search_code to find specific patterns, functions, or types\n\
             - read as many files as you need — be thorough\n\
             - use read_file with offset/limit for large files — read specific sections\n\
             - NEVER give up or say you can't access something — use the tools\n\n\
             ## your final response MUST include\n\
             1. a clear, concise answer to the question\n\
             2. key file paths with line numbers for the most relevant code\n\
             3. any important patterns, relationships, or gotchas you noticed\n\n\
             keep your answer focused and under 2000 chars. the caller will read specific \
             files themselves — you just need to point them in the right direction."
        );

        log::info!("[explore_code] starting sub-agent for: {}", &args.question);
        let start = std::time::Instant::now();

        let result = self.llm
            .chat_with_tools_only(
                &system_prompt,
                &args.question,
                vec![],
                tools,
            )
            .await
            .map_err(|e| {
                log::warn!("[explore_code] sub-agent failed after {:?}: {e}", start.elapsed());
                ToolExecError(format!("explore agent failed: {e}"))
            })?;

        log::info!("[explore_code] completed in {:?}, result: {} chars", start.elapsed(), result.len());
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// request_secret
// ---------------------------------------------------------------------------

/// Allowed target paths for secrets. Extensible for future MCP connectors.
fn is_allowed_secret_target(target: &str) -> bool {
    static PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
        vec![
            regex::Regex::new(r"^email\.accounts\.[a-zA-Z0-9_-]+\.smtp_password$").unwrap(),
            regex::Regex::new(r"^email\.accounts\.[a-zA-Z0-9_-]+\.imap_password$").unwrap(),
            regex::Regex::new(r"^llm\.tokens\.[a-zA-Z0-9_-]+$").unwrap(),
        ]
    });
    PATTERNS.iter().any(|p| p.is_match(target))
}

/// Write a secret value to the appropriate config file based on the dotted target path.
fn write_secret_to_config(
    instance_dir: &Path,
    config_path: &Path,
    target: &str,
    value: &str,
) -> Result<(), ToolExecError> {
    use super::communication::{load_instance_email_config, save_instance_email_config};

    let parts: Vec<&str> = target.split('.').collect();
    match parts.as_slice() {
        ["email", "accounts", name, field] => {
            let mut email_config = load_instance_email_config(instance_dir)?;
            let account = email_config.get_account_mut(name).ok_or_else(|| {
                ToolExecError(format!(
                    "email account '{name}' not found. create it first with update_config."
                ))
            })?;
            match *field {
                "smtp_password" => account.smtp_password = value.to_string(),
                "imap_password" => account.imap_password = value.to_string(),
                _ => return Err(ToolExecError(format!("unsupported email field: {field}"))),
            }
            save_instance_email_config(instance_dir, &email_config)?;
        }
        ["llm", "tokens", provider] => {
            let raw = fs::read_to_string(config_path)
                .map_err(|e| ToolExecError(format!("failed to read config: {e}")))?;
            let mut config: crate::config::Config = toml::from_str(&raw)
                .map_err(|e| ToolExecError(format!("failed to parse config: {e}")))?;
            match *provider {
                "openai" | "open_ai" => config.llm.tokens.open_ai = value.to_string(),
                "anthropic" => config.llm.tokens.anthropic = value.to_string(),
                "openrouter" | "open_router" => config.llm.tokens.open_router = value.to_string(),
                "brave_search" | "brave" => config.llm.tokens.brave_search = value.to_string(),
                _ => return Err(ToolExecError(format!("unsupported token provider: {provider}"))),
            }
            let output = toml::to_string_pretty(&config)
                .map_err(|e| ToolExecError(format!("failed to serialize config: {e}")))?;
            fs::write(config_path, &output)
                .map_err(|e| ToolExecError(format!("failed to write config: {e}")))?;
        }
        _ => return Err(ToolExecError(format!("unsupported target path: {target}"))),
    }
    Ok(())
}

pub struct RequestSecretTool {
    instance_slug: String,
    instance_dir: PathBuf,
    config_path: PathBuf,
    events: broadcast::Sender<ServerEvent>,
    pending_secrets: Arc<tokio::sync::Mutex<HashMap<String, PendingSecret>>>,
}

impl RequestSecretTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        config_path: &Path,
        events: broadcast::Sender<ServerEvent>,
        pending_secrets: Arc<tokio::sync::Mutex<HashMap<String, PendingSecret>>>,
    ) -> Self {
        Self {
            instance_slug: instance_slug.to_string(),
            instance_dir: workspace_dir.join("instances").join(instance_slug),
            config_path: config_path.to_path_buf(),
            events,
            pending_secrets,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct RequestSecretArgs {
    /// A user-facing prompt explaining what secret is needed (e.g. "Enter IMAP password for work account").
    pub prompt: String,
    /// Dotted target path where the secret will be stored. Allowed patterns:
    /// - email.accounts.<name>.smtp_password
    /// - email.accounts.<name>.imap_password
    /// - llm.tokens.<provider>
    pub target: String,
}

impl Tool for RequestSecretTool {
    const NAME: &'static str = "request_secret";
    type Error = ToolExecError;
    type Args = RequestSecretArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "request_secret".into(),
            description: "Ask the user to provide a sensitive value (password, API key) via a \
                secure prompt. The value is written directly to config — you never see it. \
                Use this instead of asking the user to paste secrets in chat. The target must \
                be a whitelisted config path like email.accounts.<name>.smtp_password or \
                llm.tokens.<provider>."
                .into(),
            parameters: openai_schema::<RequestSecretArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = args.target.trim().to_string();
        if !is_allowed_secret_target(&target) {
            return Err(ToolExecError(format!(
                "target '{target}' is not allowed. allowed patterns: \
                 email.accounts.<name>.smtp_password, email.accounts.<name>.imap_password, \
                 llm.tokens.<provider>"
            )));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();

        {
            let mut secrets = self.pending_secrets.lock().await;
            secrets.insert(
                id.clone(),
                PendingSecret {
                    target: target.clone(),
                    responder: tx,
                },
            );
        }

        // Broadcast the request to the client
        let _ = self.events.send(ServerEvent::SecretRequest {
            instance_slug: self.instance_slug.clone(),
            id: id.clone(),
            prompt: args.prompt.clone(),
            target: target.clone(),
        });

        log::info!(
            "[request_secret] waiting for user input (id={}, target={})",
            &id[..8],
            target
        );

        // Wait for the user's response with a 5-minute timeout
        let value = tokio::time::timeout(std::time::Duration::from_secs(300), rx)
            .await
            .map_err(|_| {
                // Clean up on timeout
                let pending = self.pending_secrets.clone();
                let id_clone = id.clone();
                tokio::spawn(async move {
                    pending.lock().await.remove(&id_clone);
                });
                ToolExecError("secret request timed out after 5 minutes".into())
            })?
            .map_err(|_| ToolExecError("secret request was cancelled".into()))?;

        // Write to config
        write_secret_to_config(&self.instance_dir, &self.config_path, &target, &value)?;

        log::info!("[request_secret] secret saved to {target}");
        Ok(format!("secret saved to {target}"))
    }
}
