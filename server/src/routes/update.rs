use axum::{Json, Router, extract::State, routing::{get, post}};
use crate::app::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/update/check", get(check_update))
        .route("/api/update/apply", post(apply_update))
}

#[derive(serde::Serialize)]
struct UpdateCheck {
    current: String,
    latest: String,
    update_available: bool,
}

async fn check_update(State(_state): State<AppState>) -> Json<UpdateCheck> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let current_tag = format!("v{current}");

    let latest = match fetch_latest_tag().await {
        Some(tag) => tag,
        None => {
            return Json(UpdateCheck {
                current: current_tag.clone(),
                latest: current_tag,
                update_available: false,
            });
        }
    };

    let update_available = latest != current_tag && latest != "nightly";

    Json(UpdateCheck {
        current: current_tag,
        latest,
        update_available,
    })
}

async fn apply_update(State(_state): State<AppState>) -> Json<serde_json::Value> {
    // Run update script
    let update_script = std::path::Path::new("/opt/bolly/scripts/update-bolly.sh");
    let script = if update_script.exists() {
        update_script.to_path_buf()
    } else {
        // Bare-metal install
        std::path::PathBuf::from("/opt/bolly/bin/update")
    };

    if !script.exists() {
        return Json(serde_json::json!({ "ok": false, "error": "update script not found" }));
    }

    log::info!("[update] applying update via {}", script.display());

    // Run update in background, then exit process so entrypoint/systemd restarts with new binary
    tokio::spawn(async move {
        // Give time for response to be sent
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let result = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(script.to_str().unwrap_or(""))
            .output()
            .await;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::info!("[update] script output: {stdout}");
                if !stderr.is_empty() {
                    log::warn!("[update] script stderr: {stderr}");
                }
                if output.status.success() {
                    log::info!("[update] update complete, restarting...");
                    // Exit with 0 — entrypoint/systemd will restart with new binary
                    std::process::exit(0);
                } else {
                    log::error!("[update] update script failed");
                }
            }
            Err(e) => {
                log::error!("[update] failed to run update script: {e}");
            }
        }
    });

    Json(serde_json::json!({ "ok": true, "message": "updating... server will restart" }))
}

async fn fetch_latest_tag() -> Option<String> {
    let channel = std::env::var("BOLLY_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    let repo = "triangle-int/bolly";

    let url = if channel == "nightly" {
        format!("https://api.github.com/repos/{repo}/releases/tags/nightly")
    } else {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "bolly-update")
        .send()
        .await
        .ok()?;

    let body: serde_json::Value = resp.json().await.ok()?;
    body["tag_name"].as_str().map(|s| s.to_string())
}
