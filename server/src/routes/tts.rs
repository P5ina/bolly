use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::post,
};
use serde::Deserialize;

use crate::app::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/tts", post(synthesize))
}

#[derive(Debug, Deserialize)]
struct TtsRequest {
    text: String,
    #[serde(default = "default_voice_id")]
    voice_id: String,
}

fn default_voice_id() -> String {
    "21m00Tcm4TlvDq8ikWAM".into() // Rachel
}

async fn synthesize(
    State(state): State<AppState>,
    Json(request): Json<TtsRequest>,
) -> Result<(HeaderMap, Body), (StatusCode, String)> {
    let api_key = {
        let cfg = state.config.read().await;
        cfg.llm.tokens.elevenlabs.clone()
    };

    if api_key.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "ElevenLabs API key not configured".into(),
        ));
    }

    if request.text.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "text is required".into()));
    }

    let url = format!(
        "https://api.elevenlabs.io/v1/text-to-speech/{}/stream",
        request.voice_id
    );

    let body = serde_json::json!({
        "text": request.text,
        "model_id": "eleven_turbo_v2_5",
        "output_format": "mp3_44100_128",
    });

    let response = state
        .http_client
        .post(&url)
        .header("xi-api-key", &api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("ElevenLabs request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("ElevenLabs API error ({status}): {text}"),
        ));
    }

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("audio/mpeg"));
    headers.insert("cache-control", HeaderValue::from_static("no-cache"));

    let stream = response.bytes_stream();
    let body = Body::from_stream(stream);

    Ok((headers, body))
}
