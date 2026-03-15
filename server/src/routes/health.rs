use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{Json, Router, routing::get};

use crate::domain::meta::HealthResponse;

static START_TIME: OnceLock<u64> = OnceLock::new();

fn start_time() -> u64 {
    *START_TIME.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    })
}

pub fn router() -> Router<crate::app::state::AppState> {
    // Touch start_time on router init so uptime is measured from server start,
    // not the first health check.
    start_time();
    Router::new().route("/healthz", get(health))
}

async fn health() -> Json<HealthResponse> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let started_at = start_time();
    Json(HealthResponse {
        status: "ok",
        timestamp: now,
        uptime_secs: now.saturating_sub(started_at),
    })
}
