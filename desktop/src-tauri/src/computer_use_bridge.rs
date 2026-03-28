use std::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};

use crate::computer_use;

/// Whether the bridge is active (used to signal shutdown).
static BRIDGE_ACTIVE: Mutex<bool> = Mutex::new(false);

/// Start listening for computer_use_request events from the instance WebSocket.
/// Called from the frontend before navigating to the instance.
#[tauri::command]
pub async fn connect_computer_use(instance_url: String, auth_token: String) -> Result<(), String> {
    {
        let mut active = BRIDGE_ACTIVE.lock().map_err(|e| e.to_string())?;
        *active = true;
    }

    // Spawn background WebSocket listener
    tokio::spawn(async move {
        if let Err(e) = run_bridge(instance_url, auth_token).await {
            eprintln!("[computer-use] bridge error: {e}");
        }
    });

    Ok(())
}

#[tauri::command]
pub fn disconnect_computer_use() -> Result<(), String> {
    let mut active = BRIDGE_ACTIVE.lock().map_err(|e| e.to_string())?;
    *active = false;
    Ok(())
}

async fn run_bridge(instance_url: String, auth_token: String) -> Result<(), String> {
    let ws_proto = if instance_url.starts_with("https") { "wss" } else { "ws" };
    let host = instance_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let ws_url = format!(
        "{ws_proto}://{host}/api/ws?token={}",
        urlencoding::encode(&auth_token)
    );

    eprintln!("[computer-use] connecting to {}", ws_url);

    let (ws, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| format!("ws connect failed: {e}"))?;

    eprintln!("[computer-use] connected");

    let (mut write, mut read) = ws.split();
    let http = reqwest::Client::new();

    while let Some(msg) = read.next().await {
        // Check if bridge is still active
        {
            let active = BRIDGE_ACTIVE.lock().map_err(|e| e.to_string())?;
            if !*active {
                eprintln!("[computer-use] bridge disconnected, stopping");
                break;
            }
        }

        let msg = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Ping(d)) => {
                let _ = write.send(Message::Pong(d)).await;
                continue;
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => {
                eprintln!("[computer-use] ws error: {e}");
                break;
            }
        };

        // Parse server event
        let event: serde_json::Value = match serde_json::from_str(&msg) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if event.get("type").and_then(|v| v.as_str()) != Some("computer_use_request") {
            continue;
        }

        let request_id = event["request_id"].as_str().unwrap_or("").to_string();
        let action = event["action"].as_str().unwrap_or("").to_string();
        let instance_slug = event["instance_slug"].as_str().unwrap_or("").to_string();

        eprintln!("[computer-use] action={action} request_id={request_id}");

        let result = execute_action(&event, &action);

        // POST result back to server
        let result_url = format!(
            "{}/api/instances/{}/computer-use/{}",
            instance_url, instance_slug, request_id
        );

        let result_json = match &result {
            Ok(ComputerUseResult::Screenshot { image, width, height, scale }) => {
                serde_json::json!({
                    "type": "screenshot",
                    "image": image,
                    "width": width,
                    "height": height,
                    "scale": scale,
                })
            }
            Ok(ComputerUseResult::Action) => {
                serde_json::json!({ "type": "action", "success": true })
            }
            Err(e) => {
                serde_json::json!({ "type": "action", "success": false, "error": e })
            }
        };

        let _ = http
            .post(&result_url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .json(&result_json)
            .send()
            .await;
    }

    eprintln!("[computer-use] bridge stopped");
    Ok(())
}

enum ComputerUseResult {
    Screenshot {
        image: String,
        width: u32,
        height: u32,
        scale: f64,
    },
    Action,
}

fn execute_action(event: &serde_json::Value, action: &str) -> Result<ComputerUseResult, String> {
    // Cache scale from last screenshot
    static SCALE: Mutex<f64> = Mutex::new(1.0);

    match action {
        "screenshot" => {
            let result = computer_use::computer_screenshot()?;
            *SCALE.lock().map_err(|e| e.to_string())? = result.scale;
            Ok(ComputerUseResult::Screenshot {
                image: result.image,
                width: result.width,
                height: result.height,
                scale: result.scale,
            })
        }
        "left_click" | "right_click" | "middle_click" => {
            let (x, y) = parse_coordinate(event);
            let scale = *SCALE.lock().map_err(|e| e.to_string())?;
            let button = action.trim_end_matches("_click").to_string();
            computer_use::computer_click(x, y, scale, button)?;
            Ok(ComputerUseResult::Action)
        }
        "double_click" => {
            let (x, y) = parse_coordinate(event);
            let scale = *SCALE.lock().map_err(|e| e.to_string())?;
            computer_use::computer_double_click(x, y, scale)?;
            Ok(ComputerUseResult::Action)
        }
        "mouse_move" => {
            let (x, y) = parse_coordinate(event);
            let scale = *SCALE.lock().map_err(|e| e.to_string())?;
            computer_use::computer_mouse_move(x, y, scale)?;
            Ok(ComputerUseResult::Action)
        }
        "type" => {
            let text = event["text"].as_str().unwrap_or("").to_string();
            computer_use::computer_type(text)?;
            Ok(ComputerUseResult::Action)
        }
        "key" => {
            let key = event["key"].as_str().unwrap_or("").to_string();
            computer_use::computer_key(key)?;
            Ok(ComputerUseResult::Action)
        }
        "scroll" => {
            let (x, y) = parse_coordinate(event);
            let scale = *SCALE.lock().map_err(|e| e.to_string())?;
            let (dx, dy) = parse_scroll_delta(event);
            computer_use::computer_scroll(x, y, scale, dx, dy)?;
            Ok(ComputerUseResult::Action)
        }
        _ => Err(format!("unknown action: {action}")),
    }
}

fn parse_coordinate(event: &serde_json::Value) -> (i32, i32) {
    let coord = &event["coordinate"];
    let x = coord.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y = coord.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    (x, y)
}

fn parse_scroll_delta(event: &serde_json::Value) -> (i32, i32) {
    let delta = &event["scroll_delta"];
    let dx = delta.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let dy = delta.get(1).and_then(|v| v.as_i64()).unwrap_or(-3) as i32;
    (dx, dy)
}
