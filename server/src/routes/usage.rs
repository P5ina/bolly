use axum::{Json, Router, extract::State, http::StatusCode, routing::get};

use crate::{app::state::AppState, services::rate_limit};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/usage", get(get_usage))
}

async fn get_usage(
    State(state): State<AppState>,
) -> Result<Json<rate_limit::Usage>, StatusCode> {
    if state.landing_url.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let usage = rate_limit::get_usage(&state.http_client, &state.landing_url, &state.landing_auth_token)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(usage))
}
