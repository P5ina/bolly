use axum::{Json, Router, extract::{Path, State}, http::StatusCode, routing::{delete, get, post, put}};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::{app::state::AppState, domain::instance::InstanceSummary, services::{chat, tools, workspace}};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/instances", get(list_instances))
        .route("/api/instances/{instance_slug}", delete(delete_instance))
        .route("/api/instances/{instance_slug}/mood", get(get_mood))
        .route("/api/instances/{instance_slug}/companion-name", get(get_companion_name))
        .route("/api/instances/{instance_slug}/companion-name", put(set_companion_name))
        .route("/api/instances/{instance_slug}/timezone", get(get_timezone))
        .route("/api/instances/{instance_slug}/timezone", put(set_timezone))
        .route("/api/instances/{instance_slug}/secret", post(submit_secret))
        .route("/api/instances/{instance_slug}/secret/{secret_id}", delete(cancel_secret))
        .route("/api/instances/{instance_slug}/context-stats", get(get_context_stats))
        .route("/api/instances/{instance_slug}/{chat_id}/context-stats", get(get_context_stats_chat))
        .route("/api/instances/{instance_slug}/email", get(get_email_config))
        .route("/api/instances/{instance_slug}/email", put(set_email_config))
        .route("/api/instances/{instance_slug}/email", delete(delete_email_config))
}

async fn list_instances(State(state): State<AppState>) -> Json<Vec<InstanceSummary>> {
    let instances = workspace::read_instances(&state.workspace_dir.join("instances"))
        .unwrap_or_default();
    Json(instances)
}

async fn delete_instance(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> StatusCode {
    // Validate slug to prevent path traversal
    if instance_slug.contains('/') || instance_slug.contains('\\') || instance_slug == ".." || instance_slug == "." {
        return StatusCode::BAD_REQUEST;
    }

    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);
    if !instance_dir.exists() {
        return StatusCode::NOT_FOUND;
    }

    match fs::remove_dir_all(&instance_dir) {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Serialize)]
struct MoodResponse {
    mood: String,
}

async fn get_mood(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<MoodResponse> {
    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);
    let mood_state = tools::load_mood_state(&instance_dir);
    Json(MoodResponse {
        mood: mood_state.companion_mood,
    })
}

#[derive(Serialize)]
struct CompanionNameResponse {
    name: String,
}

#[derive(Deserialize)]
struct SetCompanionNameRequest {
    name: String,
}

async fn get_companion_name(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<CompanionNameResponse> {
    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);
    let name = read_identity_name(&instance_dir).unwrap_or_default();
    Json(CompanionNameResponse { name })
}

async fn set_companion_name(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
    Json(req): Json<SetCompanionNameRequest>,
) -> StatusCode {
    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);
    let state_path = instance_dir.join("project_state.json");

    let mut project_state: serde_json::Value = fs::read_to_string(&state_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if project_state.get("identity").is_none() {
        project_state["identity"] = serde_json::json!({});
    }
    project_state["identity"]["name"] = serde_json::Value::String(req.name);

    fs::create_dir_all(&instance_dir).ok();
    match serde_json::to_string_pretty(&project_state) {
        Ok(body) => {
            if fs::write(&state_path, body).is_ok() {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn read_identity_name(instance_dir: &std::path::Path) -> Option<String> {
    let raw = fs::read_to_string(instance_dir.join("project_state.json")).ok()?;
    let state: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let name = state.get("identity")?.get("name")?.as_str()?;
    if name.is_empty() { None } else { Some(name.to_string()) }
}

// ---------------------------------------------------------------------------
// Timezone
// ---------------------------------------------------------------------------

async fn get_timezone(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<serde_json::Value> {
    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);
    let tz = read_timezone(&instance_dir).unwrap_or_default();
    Json(serde_json::json!({ "timezone": tz }))
}

#[derive(Deserialize)]
struct SetTimezoneRequest {
    timezone: String,
}

async fn set_timezone(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
    Json(req): Json<SetTimezoneRequest>,
) -> StatusCode {
    // Validate timezone string
    if !req.timezone.is_empty() {
        if req.timezone.parse::<chrono_tz::Tz>().is_err() {
            return StatusCode::BAD_REQUEST;
        }
    }

    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);
    let state_path = instance_dir.join("project_state.json");

    let mut project_state: serde_json::Value = fs::read_to_string(&state_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    project_state["timezone"] = serde_json::Value::String(req.timezone);

    fs::create_dir_all(&instance_dir).ok();
    match serde_json::to_string_pretty(&project_state) {
        Ok(body) => {
            if fs::write(&state_path, body).is_ok() {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Format current time in the instance's configured timezone (or UTC).
pub fn format_instance_now(instance_dir: &std::path::Path) -> String {
    let now = chrono::Utc::now();
    if let Some(tz_str) = read_timezone(instance_dir) {
        if let Ok(tz) = tz_str.parse::<chrono_tz::Tz>() {
            return now.with_timezone(&tz).format("%A, %B %-d, %Y %H:%M %Z").to_string();
        }
    }
    now.format("%A, %B %-d, %Y %H:%M UTC").to_string()
}

pub fn read_timezone(instance_dir: &std::path::Path) -> Option<String> {
    let raw = fs::read_to_string(instance_dir.join("project_state.json")).ok()?;
    let state: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let tz = state.get("timezone")?.as_str()?;
    if tz.is_empty() { None } else { Some(tz.to_string()) }
}

// ---------------------------------------------------------------------------
// Secret submission endpoint
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SubmitSecretRequest {
    id: String,
    value: String,
}

async fn submit_secret(
    State(state): State<AppState>,
    Path(_instance_slug): Path<String>,
    Json(req): Json<SubmitSecretRequest>,
) -> StatusCode {
    let mut secrets = state.pending_secrets.lock().await;
    match secrets.remove(&req.id) {
        Some(pending) => {
            let _ = pending.responder.send(req.value);
            StatusCode::OK
        }
        None => StatusCode::NOT_FOUND,
    }
}

async fn cancel_secret(
    State(state): State<AppState>,
    Path((_instance_slug, secret_id)): Path<(String, String)>,
) -> StatusCode {
    let mut secrets = state.pending_secrets.lock().await;
    match secrets.remove(&secret_id) {
        Some(_pending) => {
            // Dropping the PendingSecret drops the oneshot Sender,
            // which causes the tool's rx.await to return Err → "cancelled"
            StatusCode::OK
        }
        None => StatusCode::NOT_FOUND,
    }
}

// ---------------------------------------------------------------------------
// Context stats endpoint
// ---------------------------------------------------------------------------

async fn get_context_stats(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<chat::ContextStats> {
    Json(chat::compute_context_stats(&state.workspace_dir, &instance_slug, "default"))
}

async fn get_context_stats_chat(
    State(state): State<AppState>,
    Path((instance_slug, chat_id)): Path<(String, String)>,
) -> Json<chat::ContextStats> {
    Json(chat::compute_context_stats(&state.workspace_dir, &instance_slug, &chat_id))
}

// ---------------------------------------------------------------------------
// Email config (per-instance SMTP/IMAP)
// ---------------------------------------------------------------------------

async fn get_email_config(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<serde_json::Value> {
    let accounts = crate::config::EmailAccounts::load(&state.workspace_dir, &instance_slug);
    let items: Vec<serde_json::Value> = accounts.iter().map(|cfg| {
        serde_json::json!({
            "smtp_host": cfg.smtp_host,
            "smtp_port": cfg.smtp_port,
            "smtp_user": cfg.smtp_user,
            "smtp_from": cfg.smtp_from,
            "imap_host": cfg.imap_host,
            "imap_port": cfg.imap_port,
            "imap_user": cfg.imap_user,
            // Never expose passwords
        })
    }).collect();
    Json(serde_json::json!({ "accounts": items }))
}

async fn set_email_config(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> StatusCode {
    // Accept either { accounts: [...] } or a single account object (legacy)
    let accounts: Vec<crate::config::EmailConfig> = if let Some(arr) = body.get("accounts").and_then(|v| v.as_array()) {
        match serde_json::from_value::<Vec<crate::config::EmailConfig>>(serde_json::Value::Array(arr.clone())) {
            Ok(a) => a,
            Err(_) => return StatusCode::BAD_REQUEST,
        }
    } else {
        match serde_json::from_value::<crate::config::EmailConfig>(body) {
            Ok(single) => vec![single],
            Err(_) => return StatusCode::BAD_REQUEST,
        }
    };

    match crate::config::EmailAccounts::save(&accounts, &state.workspace_dir, &instance_slug) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn delete_email_config(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> StatusCode {
    let path = state.workspace_dir
        .join("instances")
        .join(&instance_slug)
        .join("email.toml");
    if path.exists() {
        match fs::remove_file(&path) {
            Ok(_) => StatusCode::NO_CONTENT,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    } else {
        StatusCode::NO_CONTENT
    }
}
