use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    routing::post,
};
use serde::Deserialize;
use std::path::Path;

use crate::{app::state::AppState, config::InstanceConfig};

const DEFAULT_VOICE_ID: &str = "TWutjvRaJqAX89preB4e";

pub fn router() -> Router<AppState> {
    Router::new().route("/api/tts", post(synthesize))
}

#[derive(Debug, Deserialize)]
struct TtsRequest {
    text: String,
    #[serde(default)]
    instance_slug: String,
}

/// Resolve ElevenLabs voice ID: instance config > env var > default.
pub fn resolve_voice_id(workspace_dir: &Path, instance_slug: &str) -> String {
    if !instance_slug.is_empty() {
        let inst = InstanceConfig::load(workspace_dir, instance_slug);
        if !inst.elevenlabs_voice_id.is_empty() {
            return inst.elevenlabs_voice_id;
        }
    }
    std::env::var("ELEVENLABS_VOICE_ID").unwrap_or_else(|_| DEFAULT_VOICE_ID.into())
}

/// Synthesize speech and return raw MP3 bytes.
pub async fn synthesize_bytes(
    http: &reqwest::Client,
    api_key: &str,
    voice_id: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    let url = format!(
        "https://api.elevenlabs.io/v1/text-to-speech/{}/stream?output_format=mp3_44100_128",
        voice_id
    );

    let body = serde_json::json!({
        "text": text,
        "model_id": "eleven_v3",
    });

    let response = http
        .post(&url)
        .header("xi-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("ElevenLabs request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let err_text = response.text().await.unwrap_or_default();
        return Err(format!("ElevenLabs API error ({status}): {err_text}"));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("failed to read audio bytes: {e}"))
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
        return Err((StatusCode::SERVICE_UNAVAILABLE, "ElevenLabs API key not configured".into()));
    }
    if request.text.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "text is required".into()));
    }

    let voice_id = resolve_voice_id(&state.workspace_dir, &request.instance_slug);

    let bytes = synthesize_bytes(&state.http_client, &api_key, &voice_id, &request.text)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("audio/mpeg"));
    headers.insert("cache-control", HeaderValue::from_static("no-cache"));

    Ok((headers, Body::from(bytes)))
}
