use axum::{Json, Router, routing::get};

use crate::domain::meta::HealthResponse;

pub fn router() -> Router<crate::app::state::AppState> {
    Router::new().route("/healthz", get(health))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
