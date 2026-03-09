use axum::{Json, Router, extract::State, http::StatusCode, routing::put};
use serde_json::json;

use crate::{
    app::state::AppState,
    config::{self, LlmProvider},
    domain::config::UpdateLlmRequest,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/config/llm", put(update_llm))
}

async fn update_llm(
    State(state): State<AppState>,
    Json(request): Json<UpdateLlmRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Read current config.toml as a TOML table so we preserve unrelated fields
    let config_path = config::config_path();
    let raw = std::fs::read_to_string(&config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to read config: {e}"),
        )
    })?;
    let mut doc: toml::Value = toml::from_str(&raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to parse config: {e}"),
        )
    })?;

    // Ensure llm table exists
    let root = doc.as_table_mut().unwrap();
    if !root.contains_key("llm") {
        root.insert("llm".into(), toml::Value::Table(toml::map::Map::new()));
    }
    let llm = root.get_mut("llm").unwrap().as_table_mut().unwrap();

    // Set provider
    let provider_str = match request.provider {
        LlmProvider::Anthropic => "anthropic",
        LlmProvider::OpenAI => "openai",
    };
    llm.insert(
        "provider".into(),
        toml::Value::String(provider_str.into()),
    );

    // Set model if provided
    if let Some(ref model) = request.model {
        llm.insert("model".into(), toml::Value::String(model.clone()));
    }

    // Set the appropriate token
    if !llm.contains_key("tokens") {
        llm.insert("tokens".into(), toml::Value::Table(toml::map::Map::new()));
    }
    let tokens = llm.get_mut("tokens").unwrap().as_table_mut().unwrap();
    match request.provider {
        LlmProvider::Anthropic => {
            tokens.insert(
                "ANTHROPIC".into(),
                toml::Value::String(request.api_key.clone()),
            );
        }
        LlmProvider::OpenAI => {
            tokens.insert(
                "OPEN_AI".into(),
                toml::Value::String(request.api_key.clone()),
            );
        }
    }

    // Write back
    let new_raw = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to serialize config: {e}"),
        )
    })?;
    std::fs::write(&config_path, &new_raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to write config: {e}"),
        )
    })?;

    // Reload config and rebuild LLM backend
    let new_config = config::load_config().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to reload config: {e}"),
        )
    })?;
    state.rebuild_llm(&new_config).await;

    let model = new_config
        .llm
        .model
        .as_deref()
        .unwrap_or(match request.provider {
            LlmProvider::Anthropic => "claude-sonnet-4-6",
            LlmProvider::OpenAI => "gpt-4o",
        });

    Ok(Json(json!({
        "status": "ok",
        "provider": provider_str,
        "model": model,
    })))
}
