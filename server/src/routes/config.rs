use axum::{Json, Router, extract::State, http::StatusCode, routing::{delete, get, post, put}};
// Note: `put` still used by update_model_mode
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    app::state::AppState,
    config::{self, McpServerConfig},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/config/model-mode", put(update_model_mode))
        .route("/api/config/status", get(get_status))
        .route("/api/config/llm", put(update_llm_key))
        .route("/api/config/provider", put(update_provider))
        .route("/api/config/mcp", get(list_mcp_servers))
        .route("/api/config/mcp", post(add_mcp_server))
        .route("/api/config/mcp/suggested", get(suggested_mcp_servers))
        .route("/api/config/mcp/{name}", delete(remove_mcp_server))
        .route("/api/config/github", get(get_github))
        .route("/api/config/github", put(update_github))
        .route("/api/config/server", get(get_server))
        .route("/api/config/server", put(update_server))
        // Claude CLI OAuth
        .route("/api/claude-cli/status", get(claude_cli_status))
        .route("/api/claude-cli/oauth/start", get(claude_cli_oauth_start))
        .route("/api/claude-cli/oauth/exchange", post(claude_cli_oauth_exchange))
}

async fn get_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    let mode = match config.llm.model_mode {
        config::ModelMode::Auto => "auto",
        config::ModelMode::Fast => "fast",
        config::ModelMode::Heavy => "heavy",
    };
    // Which optional keys are configured
    let t = &config.llm.tokens;
    let keys: Vec<&str> = [
        ("anthropic", !t.anthropic.is_empty()),
        ("google_ai", !t.google_ai.is_empty()),
        ("elevenlabs", !t.elevenlabs.is_empty()),
        ("openrouter", !t.open_router.is_empty()),
    ]
    .iter()
    .filter(|(_, configured)| *configured)
    .map(|(name, _)| *name)
    .collect();

    let provider = match config.llm.provider {
        config::LlmProvider::Api => "api",
        config::LlmProvider::ClaudeCli => "claude_cli",
    };

    Json(json!({
        "llm_configured": config.llm.is_configured(),
        "provider": provider,
        "model": config.llm.model_name(),
        "fast_model": config.llm.fast_model_name(),
        "model_mode": mode,
        "configured_keys": keys,
        "host": config.host,
        "port": config.port,
        "auth_token_set": !config.auth_token.is_empty(),
        "is_managed": !config.landing_url.is_empty() || std::env::var("FLY_APP_NAME").is_ok(),
    }))
}

// ---------------------------------------------------------------------------
// Model mode
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UpdateModelModeRequest {
    mode: String,
}

async fn update_model_mode(
    State(state): State<AppState>,
    Json(request): Json<UpdateModelModeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mode = match request.mode.to_lowercase().as_str() {
        "auto" => config::ModelMode::Auto,
        "fast" => config::ModelMode::Fast,
        "heavy" => config::ModelMode::Heavy,
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown mode: {other}"))),
    };

    {
        let mut cfg = state.config.write().await;
        cfg.llm.model_mode = mode;
        save_config(&cfg)?;
    }

    Ok(Json(json!({ "status": "ok", "model_mode": request.mode.to_lowercase() })))
}

// ---------------------------------------------------------------------------
// LLM API keys
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UpdateLlmKeyRequest {
    /// Which key to set: "api_key" (anthropic), "google_ai", "elevenlabs", "openrouter"
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    google_ai: Option<String>,
    #[serde(default)]
    elevenlabs: Option<String>,
    #[serde(default)]
    openrouter: Option<String>,
}

async fn update_llm_key(
    State(state): State<AppState>,
    Json(req): Json<UpdateLlmKeyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Validate Anthropic key before saving
    if let Some(key) = &req.api_key {
        let key = key.trim();
        if !key.is_empty() {
            let http = reqwest::Client::new();
            let res = http
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .body(r#"{"model":"claude-haiku-4-5-20241022","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}"#)
                .send()
                .await
                .map_err(|e| (StatusCode::BAD_GATEWAY, format!("failed to reach Anthropic: {e}")))?;

            if res.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err((StatusCode::UNAUTHORIZED, "invalid API key".into()));
            }
            // 400 (bad request) is fine — means key is valid but request was malformed (shouldn't happen)
            // 429 (rate limited) is fine — means key is valid
            // 200 is fine — means key works
            if res.status().is_server_error() {
                return Err((StatusCode::BAD_GATEWAY, "Anthropic API error — try again".into()));
            }
        }
    }

    let mut changes = Vec::new();

    {
        let mut cfg = state.config.write().await;

        if let Some(key) = &req.api_key {
            cfg.llm.tokens.anthropic = key.trim().to_string();
            changes.push("anthropic");
        }
        if let Some(key) = &req.google_ai {
            cfg.llm.tokens.google_ai = key.trim().to_string();
            changes.push("google_ai");
        }
        if let Some(key) = &req.elevenlabs {
            cfg.llm.tokens.elevenlabs = key.trim().to_string();
            changes.push("elevenlabs");
        }
        if let Some(key) = &req.openrouter {
            cfg.llm.tokens.open_router = key.trim().to_string();
            changes.push("openrouter");
        }

        save_config(&cfg)?;
    }

    // Rebuild LLM backend if anthropic key changed
    // Note: can't use reload_config() here — it compares disk config with
    // in-memory config, but we just updated in-memory above, so it sees no diff.
    if changes.contains(&"anthropic") {
        let cfg = state.config.read().await;
        let new_llm = crate::services::llm::LlmBackend::from_config(&cfg);
        drop(cfg);
        *state.llm.write().await = new_llm;
        log::info!("LLM backend rebuilt after API key change");
    }

    Ok(Json(json!({ "status": "ok", "updated": changes })))
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

/// Curated list of popular MCP servers users can add with one click.
async fn suggested_mcp_servers(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    let installed: Vec<String> = config.mcp_servers.iter().map(|s| s.name.clone()).collect();

    let suggested = serde_json::json!([
        {
            "name": "fal-ai",
            "description": "AI image & video generation (Flux, SDXL, etc.)",
            "url": "https://mcp.fal.ai/mcp",
            "requires_key": true,
            "key_env": "FAL_KEY",
            "key_url": "https://fal.ai/dashboard/keys",
            "installed": installed.contains(&"fal-ai".to_string()),
        },
        {
            "name": "brave-search",
            "description": "Web search via Brave Search API",
            "url": "https://mcp.bravesearch.com/sse",
            "requires_key": true,
            "key_env": "BRAVE_API_KEY",
            "key_url": "https://brave.com/search/api/",
            "installed": installed.contains(&"brave-search".to_string()),
        },
        {
            "name": "github",
            "description": "GitHub repos, issues, PRs, code search",
            "url": "https://api.githubcopilot.com/mcp/",
            "requires_key": true,
            "key_env": "GITHUB_TOKEN",
            "key_url": "https://github.com/settings/tokens",
            "installed": installed.contains(&"github".to_string()),
        },
        {
            "name": "firecrawl",
            "description": "Web scraping and crawling",
            "url": "https://mcp.firecrawl.dev/sse",
            "requires_key": true,
            "key_env": "FIRECRAWL_API_KEY",
            "key_url": "https://firecrawl.dev",
            "installed": installed.contains(&"firecrawl".to_string()),
        },
    ]);

    Json(suggested)
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
            args: Default::default(),
            headers: Default::default(),
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

// ---------------------------------------------------------------------------
// Server settings (host, port, auth_token)
// ---------------------------------------------------------------------------

async fn get_server(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(json!({
        "host": config.host,
        "port": config.port,
        "auth_token_set": !config.auth_token.is_empty(),
    }))
}

#[derive(Deserialize)]
struct UpdateServerRequest {
    host: Option<String>,
    port: Option<u16>,
    auth_token: Option<String>,
}

async fn update_server(
    State(state): State<AppState>,
    Json(request): Json<UpdateServerRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut needs_restart = false;

    {
        let mut config = state.config.write().await;
        if let Some(host) = &request.host {
            if config.host != *host {
                config.host = host.trim().to_string();
                needs_restart = true;
            }
        }
        if let Some(port) = request.port {
            if port > 0 && config.port != port {
                config.port = port;
                needs_restart = true;
            }
        }
        if let Some(token) = &request.auth_token {
            config.auth_token = token.trim().to_string();
        }
        save_config(&config)?;
    }

    Ok(Json(json!({
        "status": "ok",
        "needs_restart": needs_restart,
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

// ---------------------------------------------------------------------------
// Provider switching
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UpdateProviderRequest {
    provider: String,
}

async fn update_provider(
    State(state): State<AppState>,
    Json(req): Json<UpdateProviderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let provider = match req.provider.as_str() {
        "api" | "anthropic" => config::LlmProvider::Api,
        "claude_cli" | "cli" => config::LlmProvider::ClaudeCli,
        other => return Err((StatusCode::BAD_REQUEST, format!("unknown provider: {other}"))),
    };

    // Auto-install and start Meridian proxy when switching to Claude subscription
    if provider == config::LlmProvider::ClaudeCli {
        if let Err(e) = crate::services::claude_cli::ensure_meridian_installed().await {
            log::warn!("Meridian install failed: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to install Meridian: {e}")));
        }
        if !crate::services::claude_cli::is_meridian_running().await {
            if let Err(e) = crate::services::claude_cli::start_meridian().await {
                log::warn!("Meridian start failed: {e}");
            }
        }
    }

    {
        let mut cfg = state.config.write().await;
        cfg.llm.provider = provider;
        save_config(&cfg)?;
    }
    state.reload_config().await;

    Ok(Json(json!({ "status": "ok", "provider": req.provider })))
}

// ---------------------------------------------------------------------------
// Claude CLI status & OAuth
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CliStatusQuery {
    #[serde(default)]
    instance_slug: Option<String>,
}

async fn claude_cli_status(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<CliStatusQuery>,
) -> Json<serde_json::Value> {
    use crate::services::claude_cli;

    let meridian_running = claude_cli::is_meridian_running().await;

    let authenticated = query.instance_slug
        .as_deref()
        .map(|slug| claude_cli::has_valid_token(&state.workspace_dir, slug))
        .unwrap_or(false);

    Json(json!({
        "meridian_running": meridian_running,
        "authenticated": authenticated,
    }))
}

/// Start OAuth PKCE flow — returns an authorization URL for the user to visit.
async fn claude_cli_oauth_start(
    State(_state): State<AppState>,
) -> Json<serde_json::Value> {
    use crate::services::claude_cli;

    let oauth_state = claude_cli::initiate_oauth();
    let auth_url = claude_cli::build_auth_url(&oauth_state);

    // Store the PKCE state temporarily for the exchange step.
    // We use a simple file since this is a short-lived flow (10 min).
    let state_path = config::workspace_root().join(".claude_oauth_state.json");
    if let Ok(json) = serde_json::to_string_pretty(&oauth_state) {
        let _ = std::fs::write(&state_path, json);
    }

    Json(json!({
        "auth_url": auth_url,
    }))
}

#[derive(Deserialize)]
struct OAuthExchangeRequest {
    code: String,
    instance_slug: String,
}

/// Exchange authorization code for tokens and store per-instance.
async fn claude_cli_oauth_exchange(
    State(state): State<AppState>,
    Json(req): Json<OAuthExchangeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::services::claude_cli;

    // Load PKCE state
    let state_path = config::workspace_root().join(".claude_oauth_state.json");
    let state_json = std::fs::read_to_string(&state_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("No pending OAuth flow: {e}")))?;
    let oauth_state: claude_cli::OAuthState = serde_json::from_str(&state_json)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid OAuth state: {e}")))?;

    // Install + start Meridian if needed
    if let Err(e) = claude_cli::ensure_meridian_installed().await {
        log::warn!("Meridian install failed during OAuth: {e}");
    }
    if !claude_cli::is_meridian_running().await {
        let _ = claude_cli::start_meridian().await;
    }

    // Exchange code for tokens
    let http = reqwest::Client::new();
    let tokens = claude_cli::exchange_code(&http, &req.code, &oauth_state)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("OAuth exchange failed: {e}")))?;

    // Save per-instance
    let workspace = state.workspace_dir.clone();
    claude_cli::save_token(&workspace, &req.instance_slug, &tokens)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save token: {e}")))?;

    // Clean up state file
    let _ = std::fs::remove_file(&state_path);

    // Auto-switch provider to claude_cli
    {
        let mut cfg = state.config.write().await;
        cfg.llm.provider = config::LlmProvider::ClaudeCli;
        let _ = save_config(&cfg);
    }
    state.reload_config().await;

    Ok(Json(json!({
        "status": "ok",
        "expires_at": tokens.expires_at,
    })))
}
