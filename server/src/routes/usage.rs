use axum::{Json, Router, extract::State, http::StatusCode, routing::get};

use crate::{app::state::AppState, services::rate_limit};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/usage", get(get_usage))
}

async fn get_usage(
    State(state): State<AppState>,
) -> Result<Json<rate_limit::Usage>, StatusCode> {
    let pool = state.pg_pool.as_ref().ok_or(StatusCode::NOT_FOUND)?;
    let iid = state.instance_id.as_deref().ok_or(StatusCode::NOT_FOUND)?;

    let usage = rate_limit::get_usage(pool, iid)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(usage))
}
