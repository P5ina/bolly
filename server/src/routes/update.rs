use axum::{Json, Router, extract::State, routing::{get, post}};
use crate::app::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/update/check", get(check_update))
        .route("/api/update/apply", post(apply_update))
        .route("/api/update/channel", get(get_channel).put(set_channel))
}

#[derive(serde::Serialize)]
struct UpdateCheck {
    current: String,
    latest: String,
    update_available: bool,
}

async fn check_update(State(state): State<AppState>) -> Json<UpdateCheck> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let current_tag = format!("v{current}");
    let current_commit = option_env!("GIT_HASH").unwrap_or("dev");
    let channel = get_channel_value(&state.workspace_dir);

    let release = match fetch_release_info(&channel).await {
        Some(r) => r,
        None => {
            return Json(UpdateCheck {
                current: current_tag.clone(),
                latest: current_tag,
                update_available: false,
            });
        }
    };

    // Parse commit SHA from nightly release body: "Auto-built from main (abc1234)"
    let release_commit = release.body.as_deref()
        .and_then(|b| b.split('(').nth(1))
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or("");

    let update_available = if channel == "nightly" {
        !release_commit.is_empty()
            && current_commit != "dev"
            && !current_commit.starts_with(release_commit)
    } else {
        release.tag != current_tag
    };

    let display_current = if channel == "nightly" && current_commit != "dev" {
        format!("{current_tag} ({current_commit})")
    } else {
        current_tag.clone()
    };

    let display_latest = if channel == "nightly" {
        let name = release.name.as_deref().unwrap_or(&release.tag);
        if !release_commit.is_empty() {
            format!("{name} ({release_commit})")
        } else {
            name.to_string()
        }
    } else {
        release.tag
    };

    Json(UpdateCheck {
        current: display_current,
        latest: display_latest,
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

        // Read version before update to detect actual changes
        let persist = std::env::var("BOLLY_HOME").unwrap_or_else(|_| "/data".to_string());
        let version_file = std::path::Path::new(&persist).join("bin/.version");
        let version_before = std::fs::read_to_string(&version_file).unwrap_or_default().trim().to_string();

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

                // Check if the binary actually changed (don't rely on script exit code
                // since the script in the Docker image may be outdated)
                let version_after = std::fs::read_to_string(&version_file).unwrap_or_default().trim().to_string();

                if version_after != version_before && !version_after.is_empty() {
                    log::info!("[update] binary updated: {version_before} → {version_after}, restarting...");
                    std::process::exit(0);
                } else {
                    log::info!("[update] no change after update script (before={version_before}, after={version_after})");
                }
            }
            Err(e) => {
                log::error!("[update] failed to run update script: {e}");
            }
        }
    });

    Json(serde_json::json!({ "ok": true, "message": "updating... server will restart" }))
}

fn get_channel_value(workspace_dir: &std::path::Path) -> String {
    let path = workspace_dir.join(".update-channel");
    if let Ok(ch) = std::fs::read_to_string(&path) {
        let ch = ch.trim().to_string();
        if !ch.is_empty() { return ch; }
    }
    "stable".to_string()
}

async fn get_channel(State(state): State<AppState>) -> Json<serde_json::Value> {
    let channel = get_channel_value(&state.workspace_dir);
    Json(serde_json::json!({ "channel": channel }))
}

#[derive(serde::Deserialize)]
struct SetChannelRequest {
    channel: String,
}

async fn set_channel(
    State(state): State<AppState>,
    Json(req): Json<SetChannelRequest>,
) -> Json<serde_json::Value> {
    let channel = req.channel.trim().to_lowercase();
    if channel != "stable" && channel != "nightly" {
        return Json(serde_json::json!({ "ok": false, "error": "channel must be 'stable' or 'nightly'" }));
    }
    let path = state.workspace_dir.join(".update-channel");
    let _ = std::fs::write(&path, &channel);
    Json(serde_json::json!({ "ok": true, "channel": channel }))
}

struct ReleaseInfo {
    tag: String,
    name: Option<String>,
    body: Option<String>,
}

async fn fetch_release_info(channel: &str) -> Option<ReleaseInfo> {
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

    let data: serde_json::Value = resp.json().await.ok()?;
    Some(ReleaseInfo {
        tag: data["tag_name"].as_str()?.to_string(),
        name: data["name"].as_str().map(|s| s.to_string()),
        body: data["body"].as_str().map(|s| s.to_string()),
    })
}
