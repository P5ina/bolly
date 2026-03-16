use axum::{Json, Router, extract::{Path, State}, http::StatusCode, routing::{delete, get, post, put}};
use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::{app::state::AppState, domain::instance::InstanceSummary, domain::memory::MemoryEntry, services::{chat, memory, tools, workspace}};

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
        .route("/api/instances/{instance_slug}/stats", get(get_stats))
        .route("/api/instances/{instance_slug}/memory", get(list_memory))
        .route("/api/instances/{instance_slug}/memory/search", get(search_memory))
        .route("/api/instances/{instance_slug}/memory/{*path}", get(read_memory_file))
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
    let wd = state.workspace_dir.clone();
    let slug = instance_slug.clone();
    let stats = tokio::spawn(async move {
        chat::compute_context_stats_async(wd, slug, "default".to_string()).await
    }).await.unwrap_or_else(|_| chat::compute_context_stats(&state.workspace_dir, &instance_slug, "default"));
    Json(stats)
}

async fn get_context_stats_chat(
    State(state): State<AppState>,
    Path((instance_slug, chat_id)): Path<(String, String)>,
) -> Json<chat::ContextStats> {
    let wd = state.workspace_dir.clone();
    let slug = instance_slug.clone();
    let cid = chat_id.clone();
    let stats = tokio::spawn(async move {
        chat::compute_context_stats_async(wd, slug, cid).await
    }).await.unwrap_or_else(|_| chat::compute_context_stats(&state.workspace_dir, &instance_slug, &chat_id));
    Json(stats)
}

// ---------------------------------------------------------------------------
// Stats / Analytics
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct StatsResponse {
    /// Messages per hour of day (0-23)
    hourly_activity: [u32; 24],
    /// Messages per day of week (0=Mon, 6=Sun)
    daily_activity: [u32; 7],
    /// Total user messages
    total_messages: u32,
    /// Average message length (chars)
    avg_message_length: f64,
    /// Average seconds between messages in a session
    avg_response_interval_secs: f64,
    /// Daily message counts: [(date_str, count)]
    daily_history: Vec<(String, u32)>,
    /// Mood distribution: {mood: count}
    mood_counts: std::collections::HashMap<String, u32>,
    /// Current streak (consecutive days with messages)
    streak_days: u32,
    /// First message timestamp (millis)
    first_message_at: Option<String>,
}

async fn get_stats(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<StatsResponse> {
    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);

    // Resolve timezone for local time computation
    let tz: chrono_tz::Tz = read_timezone(&instance_dir)
        .and_then(|s| s.parse().ok())
        .unwrap_or(chrono_tz::UTC);

    // Scan ALL messages across all chats
    let chats_dir = instance_dir.join("chats");
    let mut user_timestamps: Vec<i64> = Vec::new(); // seconds
    let mut total_chars: u64 = 0;

    if let Ok(entries) = fs::read_dir(&chats_dir) {
        for entry in entries.filter_map(Result::ok) {
            let msgs_path = entry.path().join("messages.json");
            if let Ok(raw) = fs::read_to_string(&msgs_path) {
                if let Ok(msgs) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) {
                    for msg in &msgs {
                        if msg["role"].as_str() != Some("user") { continue; }
                        if let Some(ts_str) = msg["created_at"].as_str() {
                            if let Ok(ts_ms) = ts_str.parse::<i64>() {
                                user_timestamps.push(ts_ms / 1000);
                                total_chars += msg["content"].as_str().map(|s| s.len() as u64).unwrap_or(0);
                            }
                        }
                    }
                }
            }
        }
    }

    user_timestamps.sort();
    let total_messages = user_timestamps.len() as u32;

    // Compute hourly & daily activity + daily_history from timestamps using local timezone
    let mut hourly_activity = [0u32; 24];
    let mut daily_activity = [0u32; 7];
    let mut daily_map: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();

    for &ts in &user_timestamps {
        if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
            let local = dt.with_timezone(&tz);
            hourly_activity[local.hour() as usize] += 1;
            daily_activity[local.weekday().num_days_from_monday() as usize] += 1;
            let date = local.format("%Y-%m-%d").to_string();
            *daily_map.entry(date).or_insert(0) += 1;
        }
    }

    let daily_history: Vec<(String, u32)> = daily_map.into_iter().collect();

    // Average message length
    let avg_message_length = if total_messages > 0 { total_chars as f64 / total_messages as f64 } else { 0.0 };

    // Average response interval (within sessions, gap < 2h)
    let mut intervals: Vec<f64> = Vec::new();
    for pair in user_timestamps.windows(2) {
        let gap = (pair[1] - pair[0]) as f64;
        if gap < 7200.0 && gap > 0.0 { intervals.push(gap); }
    }
    let avg_response_interval_secs = if intervals.is_empty() { 0.0 }
        else { intervals.iter().sum::<f64>() / intervals.len() as f64 };

    // Scan thoughts for mood distribution
    let mut mood_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let thoughts_dir = instance_dir.join("thoughts");
    if let Ok(entries) = fs::read_dir(&thoughts_dir) {
        for entry in entries.filter_map(Result::ok) {
            if entry.path().extension().and_then(|e| e.to_str()) != Some("json") { continue; }
            if let Ok(raw) = fs::read_to_string(entry.path()) {
                if let Ok(thought) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(m) = thought["mood"].as_str() {
                        if !m.is_empty() {
                            *mood_counts.entry(m.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }

    // Streak: consecutive days ending today or yesterday (local time)
    let local_now = chrono::Utc::now().with_timezone(&tz);
    let today = local_now.format("%Y-%m-%d").to_string();
    let yesterday = (local_now - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let dates: std::collections::HashSet<String> = daily_history.iter().map(|(d, _)| d.clone()).collect();
    let mut streak_days = 0u32;
    let mut check_date = if dates.contains(&today) { local_now.date_naive() }
        else if dates.contains(&yesterday) { (local_now - chrono::Duration::days(1)).date_naive() }
        else { local_now.date_naive() };
    loop {
        if dates.contains(&check_date.format("%Y-%m-%d").to_string()) {
            streak_days += 1;
            check_date -= chrono::Duration::days(1);
        } else {
            break;
        }
    }

    let first_message_at = user_timestamps.first().map(|ts| (ts * 1000).to_string());

    Json(StatsResponse {
        hourly_activity,
        daily_activity,
        total_messages,
        avg_message_length,
        avg_response_interval_secs,
        daily_history,
        mood_counts,
        streak_days,
        first_message_at,
    })
}

// ---------------------------------------------------------------------------
// Memory library
// ---------------------------------------------------------------------------

async fn list_memory(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<Vec<MemoryEntry>> {
    Json(memory::scan_library(&state.workspace_dir, &instance_slug))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_search_limit")]
    limit: usize,
}

fn default_search_limit() -> usize { 10 }

async fn search_memory(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> Json<Vec<memory::SearchResult>> {
    Json(memory::search(&state.workspace_dir, &instance_slug, &params.q, params.limit))
}

async fn read_memory_file(
    State(state): State<AppState>,
    Path((instance_slug, file_path)): Path<(String, String)>,
) -> Result<String, StatusCode> {
    // Validate path — prevent traversal
    if file_path.contains("..") || file_path.starts_with('/') {
        return Err(StatusCode::BAD_REQUEST);
    }
    let full_path = state.workspace_dir
        .join("instances")
        .join(&instance_slug)
        .join("memory")
        .join(&file_path);
    fs::read_to_string(&full_path).map_err(|_| StatusCode::NOT_FOUND)
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

    // Reject accounts with empty passwords — they won't work and are likely
    // Google OAuth accounts mistakenly added as SMTP/IMAP
    for acct in &accounts {
        if acct.smtp_password.is_empty() || acct.imap_password.is_empty() {
            return StatusCode::BAD_REQUEST;
        }
    }

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
