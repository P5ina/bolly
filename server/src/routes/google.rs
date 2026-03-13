use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};

use crate::app::state::AppState;
use crate::services::google::GoogleClient;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/instances/{slug}/google/accounts",
            get(list_google_accounts),
        )
        .route(
            "/api/instances/{slug}/google/connect",
            get(google_connect_url),
        )
        .route(
            "/api/instances/{slug}/google/accounts/{email}",
            delete(disconnect_google_account),
        )
}

/// GET /api/instances/:slug/google/accounts — list connected Google accounts
async fn list_google_accounts(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    let google = GoogleClient::from_env(&config.auth_token);

    let Some(google) = google else {
        return Json(serde_json::json!({ "accounts": [] })).into_response();
    };

    match google.accounts(&slug).await {
        Ok(accounts) => {
            let emails: Vec<serde_json::Value> = accounts
                .iter()
                .map(|a| serde_json::json!({ "email": a.email }))
                .collect();
            Json(serde_json::json!({ "accounts": emails })).into_response()
        }
        Err(e) => {
            log::warn!("failed to list Google accounts for {slug}: {e}");
            Json(serde_json::json!({ "accounts": [] })).into_response()
        }
    }
}

/// GET /api/instances/:slug/google/connect — return OAuth URL
async fn google_connect_url(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    let auth_token = &config.auth_token;

    let landing_url = match std::env::var("LANDING_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "LANDING_URL not configured" })),
            )
                .into_response()
        }
    };

    // Build the connect URL that points to the landing OAuth flow
    // The redirect should bring the user back to the client settings page
    let connect_url = format!(
        "{}/dashboard/connect-google?token={}&instance={}&redirect=/{}",
        landing_url.trim_end_matches('/'),
        auth_token,
        slug,
        // Redirect back to instance settings after OAuth
        format!("{slug}/settings")
    );

    Json(serde_json::json!({ "url": connect_url })).into_response()
}

/// DELETE /api/instances/:slug/google/accounts/:email — disconnect a Google account
async fn disconnect_google_account(
    State(state): State<AppState>,
    Path((slug, email)): Path<(String, String)>,
) -> impl IntoResponse {
    let config = state.config.read().await;
    let auth_token = config.auth_token.clone();

    let landing_url = match std::env::var("LANDING_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "error": "LANDING_URL not configured" })),
            )
                .into_response()
        }
    };

    // Call the landing disconnect endpoint
    let client = reqwest::Client::new();
    let res = client
        .post(format!(
            "{}/dashboard/disconnect-google",
            landing_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "token": auth_token,
            "instance": slug,
            "email": email,
        }))
        .send()
        .await;

    match res {
        Ok(r) if r.status().is_success() => {
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            log::warn!("disconnect-google failed: {status} {body}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("landing returned {status}") })),
            )
                .into_response()
        }
        Err(e) => {
            log::warn!("disconnect-google request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "failed to reach landing" })),
            )
                .into_response()
        }
    }
}
