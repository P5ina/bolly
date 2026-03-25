use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    config::{self, Config},
    domain::events::ServerEvent,
    services::llm::LlmBackend,
    services::mcp::McpRegistry,
    services::keyword_search::KeywordStore,
    services::vector::VectorStore,
};

/// A pending secret request waiting for user input.
pub struct PendingSecret {
    #[allow(dead_code)]
    pub target: String,
    pub responder: tokio::sync::oneshot::Sender<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub workspace_dir: PathBuf,
    pub events: broadcast::Sender<ServerEvent>,
    pub llm: Arc<RwLock<Option<LlmBackend>>>,
    /// Active agent tasks per instance slug — cancellation tokens.
    pub agent_tasks: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Pending secret requests awaiting user input.
    pub pending_secrets: Arc<Mutex<HashMap<String, PendingSecret>>>,
    /// Connected MCP servers and their tools.
    pub mcp_registry: McpRegistry,
    /// Shared HTTP client for landing API calls.
    pub http_client: reqwest::Client,
    /// Landing server URL (empty for self-hosted).
    pub landing_url: String,
    /// Auth token for landing API calls.
    pub landing_auth_token: String,
    /// Qdrant vector store for semantic memory search.
    pub vector_store: Arc<VectorStore>,
    /// BM25 keyword search over memory files.
    pub keyword_store: Arc<KeywordStore>,
}

/// Inject MCP servers from environment variables (FAL_KEY, etc.)
fn inject_env_mcp_servers(config: &mut Config) {
    if let Ok(fal_key) = std::env::var("FAL_KEY") {
        if !fal_key.is_empty() {
            let auth_value = format!("Bearer {fal_key}");
            if let Some(existing) = config.mcp_servers.iter_mut().find(|s| s.name == "fal-ai") {
                existing.headers.insert("Authorization".to_string(), auth_value);
            } else {
                config.mcp_servers.push(crate::config::McpServerConfig {
                    name: "fal-ai".to_string(),
                    url: Some("https://mcp.fal.ai/mcp".to_string()),
                    command: None,
                    headers: [("Authorization".to_string(), auth_value)]
                        .into_iter().collect(),
                });
                log::info!("MCP: auto-added fal-ai (FAL_KEY present)");
            }
        }
    }
}

impl AppState {
    pub async fn new(mut config: Config) -> Self {
        let (events, _) = broadcast::channel(4096);
        let llm = LlmBackend::from_config(&config);

        inject_env_mcp_servers(&mut config);

        // Connect to configured MCP servers
        let mcp_connections = crate::services::mcp::connect_all(&config.mcp_servers).await;
        let mcp_registry = McpRegistry::new(mcp_connections, config.mcp_servers.clone());
        let mcp_tool_count = mcp_registry.tool_count().await;
        if mcp_tool_count > 0 {
            log::info!("MCP: {} tools from external servers", mcp_tool_count);
        }

        let http_client = reqwest::Client::new();
        let landing_url = config.landing_url.clone();
        let landing_auth_token = config.auth_token.clone();

        // Connect to Qdrant vector store
        let vector_store = VectorStore::connect(&config.qdrant_url).await;

        // Fetch plan from landing API if configured
        if !landing_url.is_empty() && !landing_auth_token.is_empty() {
            match http_client
                .get(format!("{landing_url}/api/internal/plan"))
                .bearer_auth(&landing_auth_token)
                .send()
                .await
            {
                Ok(res) => {
                    if let Ok(body) = res.json::<serde_json::Value>().await {
                        if let Some(plan) = body["plan"].as_str() {
                            log::info!("fetched plan from landing API: {plan}");
                            config.plan = plan.to_string();
                        }
                    }
                }
                Err(e) => log::warn!("failed to fetch plan from landing API: {e}"),
            }
        }

        Self {
            config: Arc::new(RwLock::new(config)),
            workspace_dir: config::workspace_root(),
            events,
            llm: Arc::new(RwLock::new(llm)),
            agent_tasks: Arc::new(Mutex::new(HashMap::new())),
            pending_secrets: Arc::new(Mutex::new(HashMap::new())),
            mcp_registry,
            http_client,
            landing_url,
            landing_auth_token,
            vector_store: Arc::new(vector_store),
            keyword_store: Arc::new(KeywordStore::new()),
        }
    }

    /// Reload config from disk and rebuild LLM if tokens changed.
    pub async fn reload_config(&self) {
        let mut new_config = match config::load_config() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("failed to reload config: {e}");
                return;
            }
        };

        // Re-apply runtime env injections (same as AppState::new)
        inject_env_mcp_servers(&mut new_config);

        let (tokens_changed, mcp_changed) = {
            let old = self.config.read().await;
            let tokens = old.llm.tokens.anthropic != new_config.llm.tokens.anthropic;
            let mcp = old.mcp_servers.len() != new_config.mcp_servers.len()
                || old.mcp_servers.iter().zip(new_config.mcp_servers.iter())
                    .any(|(a, b)| a.name != b.name || a.url != b.url);
            (tokens, mcp)
        };

        if tokens_changed {
            let new_llm = LlmBackend::from_config(&new_config);
            *self.llm.write().await = new_llm;
            log::info!("config reloaded: LLM rebuilt");
        }

        if mcp_changed {
            self.mcp_registry.reconnect(&new_config.mcp_servers).await;
            log::info!("config reloaded: MCP servers reconnected ({} tools)", self.mcp_registry.tool_count().await);
        }

        // Preserve plan from API — it's not in config.toml
        let mut cfg = self.config.write().await;
        let plan = cfg.plan.clone();
        *cfg = new_config;
        if cfg.plan.is_empty() {
            cfg.plan = plan;
        }
    }

}
