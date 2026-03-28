use std::sync::Mutex;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use crate::computer_use;
use crate::overlay;

/// Whether the bridge is active.
static BRIDGE_ACTIVE: Mutex<bool> = Mutex::new(false);

/// Start the machine agent — connects to the server's machine WebSocket,
/// registers this machine, then listens for toolcalls and executes them.
#[tauri::command]
pub async fn connect_computer_use(
    app: tauri::AppHandle,
    instance_url: String,
    auth_token: String,
) -> Result<(), String> {
    {
        let mut active = BRIDGE_ACTIVE.lock().map_err(|e| e.to_string())?;
        *active = true;
    }

    tokio::spawn(async move {
        loop {
            // Check if still active
            {
                let active = BRIDGE_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
                if !*active {
                    break;
                }
            }

            match run_agent(&app, &instance_url, &auth_token).await {
                Ok(_) => {
                    eprintln!("[agent] connection closed, reconnecting in 3s...");
                }
                Err(e) => {
                    eprintln!("[agent] error: {e}, reconnecting in 3s...");
                }
            }

            // Check if still active before reconnecting
            {
                let active = BRIDGE_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
                if !*active {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
        eprintln!("[agent] bridge stopped");
    });

    Ok(())
}

#[tauri::command]
pub fn disconnect_computer_use() -> Result<(), String> {
    let mut active = BRIDGE_ACTIVE.lock().map_err(|e| e.to_string())?;
    *active = false;
    Ok(())
}

async fn run_agent(app: &tauri::AppHandle, instance_url: &str, auth_token: &str) -> Result<(), String> {
    let ws_proto = if instance_url.starts_with("https") {
        "wss"
    } else {
        "ws"
    };
    let host = instance_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let ws_url = format!(
        "{ws_proto}://{host}/api/agents/ws/machine?token={}",
        urlencoding::encode(auth_token)
    );

    eprintln!("[agent] connecting to {ws_url}");

    let (ws, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| format!("ws connect: {e}"))?;

    let (mut write, mut read) = ws.split();

    // Register this machine
    let machine_id = hostname();
    let os = std::env::consts::OS.to_string();

    // Get screen dimensions
    let screen = screenshots::Screen::all()
        .ok()
        .and_then(|s| s.into_iter().next());
    let (sw, sh) = screen
        .map(|s| {
            let info = s.display_info;
            (info.width, info.height)
        })
        .unwrap_or((1920, 1080));

    let register = serde_json::json!({
        "type": "register",
        "machine_id": machine_id,
        "os": os,
        "hostname": machine_id,
        "screen_width": sw,
        "screen_height": sh,
    });
    write
        .send(Message::Text(register.to_string().into()))
        .await
        .map_err(|e| format!("send register: {e}"))?;

    eprintln!("[agent] registered as '{machine_id}' ({os}, {sw}x{sh})");

    // Scale cache from last screenshot
    let mut cached_scale: f64 = 1.0;

    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(20));
    ping_interval.tick().await; // skip first immediate tick
    let mut last_pong = std::time::Instant::now();

    // Main loop: receive toolcalls, execute, send results
    loop {
        // Check if bridge is still active
        {
            let active = BRIDGE_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
            if !*active {
                break;
            }
        }

        let text;

        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        text = t.to_string();
                    }
                    Some(Ok(Message::Ping(d))) => {
                        let _ = write.send(Message::Pong(d)).await;
                        continue;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = std::time::Instant::now();
                        continue;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => {
                        eprintln!("[agent] ws error: {e}");
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                // Send ping
                if write.send(Message::Ping(vec![].into())).await.is_err() {
                    eprintln!("[agent] ping send failed, reconnecting...");
                    break;
                }
                // Check if we got a pong recently (within 45s)
                if last_pong.elapsed() > std::time::Duration::from_secs(45) {
                    eprintln!("[agent] no pong in 45s, reconnecting...");
                    break;
                }
                continue;
            }
        }

        let call: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Skip non-toolcall messages (e.g. "registered" ack)
        let request_id = match call.get("request_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };
        let action = call
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        eprintln!("[agent] toolcall: {action} (req={request_id})");

        // Hide overlay before screenshot so it doesn't appear in the capture
        if action == "screenshot" {
            overlay::hide(app);
            // Give the main thread time to close the overlay window
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        let result = execute_action(&call, &action, &mut cached_scale);

        // Show overlay after any action (including screenshot) to indicate activity
        overlay::show(app);
        overlay::emit_action(app, &action);

        let response = match &result {
            Ok(AgentResult::Screenshot {
                image,
                width,
                height,
                scale,
            }) => serde_json::json!({
                "type": "action_result",
                "request_id": request_id,
                "result_type": "screenshot",
                "image": image,
                "width": width,
                "height": height,
                "scale": scale,
                "success": true,
            }),
            Ok(AgentResult::Action) => serde_json::json!({
                "type": "action_result",
                "request_id": request_id,
                "result_type": "action",
                "success": true,
            }),
            Ok(AgentResult::Output(text)) => serde_json::json!({
                "type": "action_result",
                "request_id": request_id,
                "result_type": "output",
                "success": true,
                "error": text, // reuse error field for output text
            }),
            Err(e) => serde_json::json!({
                "type": "action_result",
                "request_id": request_id,
                "result_type": "action",
                "success": false,
                "error": e,
            }),
        };

        if write
            .send(Message::Text(response.to_string().into()))
            .await
            .is_err()
        {
            break;
        }
    }

    overlay::emit_idle(app);
    overlay::hide(app);

    Ok(())
}

enum AgentResult {
    Screenshot {
        image: String,
        width: u32,
        height: u32,
        scale: f64,
    },
    Action,
    /// Text output (bash stdout, file content, directory listing).
    Output(String),
}

fn execute_action(
    call: &serde_json::Value,
    action: &str,
    cached_scale: &mut f64,
) -> Result<AgentResult, String> {
    match action {
        "screenshot" => {
            let result = computer_use::computer_screenshot()?;
            *cached_scale = result.scale;
            Ok(AgentResult::Screenshot {
                image: result.image,
                width: result.width,
                height: result.height,
                scale: result.scale,
            })
        }
        "left_click" | "right_click" | "middle_click" => {
            let (x, y) = parse_coordinate(call);
            let button = action.trim_end_matches("_click").to_string();
            computer_use::computer_click(x, y, *cached_scale, button)?;
            Ok(AgentResult::Action)
        }
        "double_click" => {
            let (x, y) = parse_coordinate(call);
            computer_use::computer_double_click(x, y, *cached_scale)?;
            Ok(AgentResult::Action)
        }
        "mouse_move" => {
            let (x, y) = parse_coordinate(call);
            computer_use::computer_mouse_move(x, y, *cached_scale)?;
            Ok(AgentResult::Action)
        }
        "type" => {
            let text = call["text"].as_str().unwrap_or("").to_string();
            computer_use::computer_type(text)?;
            Ok(AgentResult::Action)
        }
        "key" => {
            let key = call["key"].as_str().unwrap_or("").to_string();
            computer_use::computer_key(key)?;
            Ok(AgentResult::Action)
        }
        "scroll" => {
            let (x, y) = parse_coordinate(call);
            let direction = call["scroll_direction"].as_str().unwrap_or("down");
            let amount = call["scroll_amount"].as_i64().unwrap_or(3) as i32;
            let (dx, dy) = match direction {
                "up" => (0, amount),
                "down" => (0, -amount),
                "left" => (-amount, 0),
                "right" => (amount, 0),
                _ => (0, -amount),
            };
            computer_use::computer_scroll(x, y, *cached_scale, dx, dy)?;
            Ok(AgentResult::Action)
        }
        // ── Switch desktop (macOS Spaces) ──
        "switch_desktop" => {
            let direction = call["scroll_direction"].as_str().unwrap_or("right");
            let key = match direction {
                "left" => "ctrl+left",
                "right" => "ctrl+right",
                _ => "ctrl+right",
            };
            computer_use::computer_key(key.to_string())?;
            // Wait for animation to complete
            std::thread::sleep(std::time::Duration::from_millis(700));
            Ok(AgentResult::Action)
        }
        // ── Bash ──
        "bash" => {
            let command = call["command"].as_str().unwrap_or("").to_string();
            let cwd = call["cwd"].as_str().map(|s| s.to_string());
            execute_bash(&command, cwd.as_deref())
        }
        // ── File operations ──
        "file_read" => {
            let path = expand_path(call["path"].as_str().unwrap_or(""));
            match std::fs::read_to_string(&path) {
                Ok(content) => Ok(AgentResult::Output(content)),
                Err(e) => Err(format!("read {path}: {e}")),
            }
        }
        "file_write" => {
            let path = expand_path(call["path"].as_str().unwrap_or(""));
            let content = call["content"].as_str().unwrap_or("");
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&path, content) {
                Ok(_) => Ok(AgentResult::Output(format!("written {} bytes to {path}", content.len()))),
                Err(e) => Err(format!("write {path}: {e}")),
            }
        }
        "file_list" => {
            let path = expand_path(call["path"].as_str().unwrap_or("."));
            match std::fs::read_dir(&path) {
                Ok(entries) => {
                    let mut lines = Vec::new();
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let meta = entry.metadata().ok();
                        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        if is_dir {
                            lines.push(format!("{name}/"));
                        } else {
                            lines.push(format!("{name}  ({size} bytes)"));
                        }
                    }
                    lines.sort();
                    Ok(AgentResult::Output(lines.join("\n")))
                }
                Err(e) => Err(format!("list {path}: {e}")),
            }
        }
        _ => Err(format!("unknown action: {action}")),
    }
}

fn execute_bash(command: &str, cwd: Option<&str>) -> Result<AgentResult, String> {
    use std::process::Command;

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };

    if let Some(dir) = cwd {
        cmd.current_dir(expand_path(dir));
    }

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut result = String::new();
            if !stdout.is_empty() {
                result.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !result.is_empty() { result.push('\n'); }
                result.push_str("[stderr] ");
                result.push_str(&stderr);
            }
            if result.is_empty() {
                result = format!("(exit code: {})", output.status.code().unwrap_or(-1));
            }
            if output.status.success() {
                Ok(AgentResult::Output(result))
            } else {
                Err(result)
            }
        }
        Err(e) => Err(format!("failed to run command: {e}")),
    }
}

fn expand_path(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

fn parse_coordinate(call: &serde_json::Value) -> (i32, i32) {
    let coord = &call["coordinate"];
    let x = coord.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y = coord.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    (x, y)
}

fn hostname() -> String {
    gethostname::gethostname()
        .to_string_lossy()
        .to_string()
}
