use axum::{Json, Router, extract::State, http::StatusCode, routing::{delete, get, post, put}};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    app::state::AppState,
    config::{self, LlmProvider, McpServerConfig},
    domain::config::UpdateLlmRequest,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/config/llm", put(update_llm))
        .route("/api/config/status", get(get_status))
        .route("/api/config/mcp", get(list_mcp_servers))
        .route("/api/config/mcp", post(add_mcp_server))
        .route("/api/config/mcp/{name}", delete(remove_mcp_server))
        .route("/api/config/github", get(get_github))
        .route("/api/config/github", put(update_github))
}

async fn get_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(json!({
        "llm_configured": config.llm.is_configured(),
        "provider": config.llm.provider.map(|p| format!("{p:?}").to_lowercase()),
        "model": config.llm.model_name(),
    }))
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
        LlmProvider::OpenRouter => "openrouter",
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
        LlmProvider::OpenRouter => {
            tokens.insert(
                "OPENROUTER".into(),
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
    *state.config.write().await = new_config.clone();

    let model = new_config.llm.model_name();

    Ok(Json(json!({
        "status": "ok",
        "provider": provider_str,
        "model": model,
    })))
}

// ---------------------------------------------------------------------------
// MCP server management
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct McpServerInfo {
    name: String,
    url: Option<String>,
    connected: bool,
}

async fn list_mcp_servers(State(state): State<AppState>) -> Json<Vec<McpServerInfo>> {
    let config = state.config.read().await;
    let connected_names = state.mcp_registry.server_names().await;
    let servers: Vec<McpServerInfo> = config
        .mcp_servers
        .iter()
        .map(|s| McpServerInfo {
            name: s.name.clone(),
            url: s.url.clone(),
            connected: connected_names.contains(&s.name),
        })
        .collect();
    Json(servers)
}

#[derive(Deserialize)]
struct AddMcpServerRequest {
    name: String,
    url: String,
}

async fn add_mcp_server(
    State(state): State<AppState>,
    Json(request): Json<AddMcpServerRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = request.name.trim().to_string();
    let url = request.url.trim().to_string();
    if name.is_empty() || url.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name and url are required".into()));
    }

    // Update config
    {
        let mut config = state.config.write().await;
        if config.mcp_servers.iter().any(|s| s.name == name) {
            return Err((StatusCode::CONFLICT, format!("MCP server '{name}' already exists")));
        }
        config.mcp_servers.push(McpServerConfig {
            name: name.clone(),
            url: Some(url.clone()),
            command: None,
        });
        save_config(&config)?;
    }

    // Reconnect all MCP servers
    let configs = state.config.read().await.mcp_servers.clone();
    state.mcp_registry.reconnect(&configs).await;
    let tool_count = state.mcp_registry.tool_count().await;

    Ok(Json(json!({
        "status": "ok",
        "name": name,
        "tool_count": tool_count,
    })))
}

async fn remove_mcp_server(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    {
        let mut config = state.config.write().await;
        let before = config.mcp_servers.len();
        config.mcp_servers.retain(|s| s.name != name);
        if config.mcp_servers.len() == before {
            return Err((StatusCode::NOT_FOUND, format!("MCP server '{name}' not found")));
        }
        save_config(&config)?;
    }

    // Reconnect
    let configs = state.config.read().await.mcp_servers.clone();
    state.mcp_registry.reconnect(&configs).await;

    Ok(Json(json!({ "status": "ok" })))
}

// ---------------------------------------------------------------------------
// GitHub integration
// ---------------------------------------------------------------------------

async fn get_github(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    let has_token = !config.github.token.is_empty();
    Json(json!({
        "configured": has_token,
    }))
}

#[derive(Deserialize)]
struct UpdateGithubRequest {
    token: String,
}

async fn update_github(
    State(state): State<AppState>,
    Json(request): Json<UpdateGithubRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let token = request.token.trim().to_string();

    {
        let mut config = state.config.write().await;
        config.github.token = token.clone();
        save_config(&config)?;
    }

    let configured = !token.is_empty();
    Ok(Json(json!({
        "status": "ok",
        "configured": configured,
    })))
}

/// Write the current config back to disk.
fn save_config(config: &config::Config) -> Result<(), (StatusCode, String)> {
    let config_path = config::config_path();
    let raw = toml::to_string_pretty(config).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to serialize config: {e}"))
    })?;
    std::fs::write(&config_path, &raw).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to write config: {e}"))
    })?;
    Ok(())
}
