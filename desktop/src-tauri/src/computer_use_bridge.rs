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
                    eprintln!("[agent] connection closed, reconnecting in 5s...");
                }
                Err(e) => {
                    eprintln!("[agent] error: {e}, reconnecting in 5s...");
                }
            }

            // Check if still active before reconnecting
            {
                let active = BRIDGE_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
                if !*active {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
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

    // Main loop: receive toolcalls, execute, send results
    while let Some(msg) = read.next().await {
        // Check if bridge is still active
        {
            let active = BRIDGE_ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
            if !*active {
                break;
            }
        }

        let text = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Ping(d)) => {
                let _ = write.send(Message::Pong(d)).await;
                continue;
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => {
                eprintln!("[agent] ws error: {e}");
                break;
            }
        };

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
        _ => Err(format!("unknown action: {action}")),
    }
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
