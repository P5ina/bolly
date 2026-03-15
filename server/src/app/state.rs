use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    config::{self, Config},
    domain::events::ServerEvent,
    services::llm::LlmBackend,
    services::mcp::McpRegistry,
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
    /// Shared Postgres pool for rate limiting (None for self-hosted).
    pub pg_pool: Option<PgPool>,
    /// Instance identifier for rate limit tracking.
    pub instance_id: Option<String>,
    /// Pending secret requests awaiting user input.
    pub pending_secrets: Arc<Mutex<HashMap<String, PendingSecret>>>,
    /// Connected MCP servers and their tools.
    pub mcp_registry: McpRegistry,
}

impl AppState {
    pub async fn new(mut config: Config) -> Self {
        let (events, _) = broadcast::channel(256);
        let llm = LlmBackend::from_config(&config);

        // Connect to configured MCP servers
        let mcp_connections = crate::services::mcp::connect_all(&config.mcp_servers).await;
        let mcp_registry = McpRegistry::new(mcp_connections);
        let mcp_tool_count = mcp_registry.tool_count().await;
        if mcp_tool_count > 0 {
            log::info!("MCP: {} tools from external servers", mcp_tool_count);
        }

        let (pg_pool, instance_id) = match std::env::var("DATABASE_URL") {
            Ok(url) if !url.is_empty() => {
                let pool = PgPool::connect(&url).await.ok();
                if pool.is_none() {
                    log::warn!("DATABASE_URL set but connection failed — rate limiting disabled");
                }
                let id = std::env::var("BOLLY_INSTANCE_ID").ok().filter(|s| !s.is_empty());
                (pool, id)
            }
            _ => (None, None),
        };

        // Fetch plan from landing DB if we have a pool + instance ID
        if let (Some(pool), Some(id)) = (&pg_pool, &instance_id) {
            match sqlx::query_scalar::<_, String>("SELECT plan::text FROM tenants WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await
            {
                Ok(Some(plan)) => {
                    log::info!("fetched plan from DB: {plan}");
                    config.plan = plan;
                }
                Ok(None) => log::warn!("instance {id} not found in tenants table"),
                Err(e) => log::warn!("failed to fetch plan from DB: {e}"),
            }
        }

        Self {
            config: Arc::new(RwLock::new(config)),
            workspace_dir: config::workspace_root(),
            events,
            llm: Arc::new(RwLock::new(llm)),
            agent_tasks: Arc::new(Mutex::new(HashMap::new())),
            pg_pool,
            instance_id,
            pending_secrets: Arc::new(Mutex::new(HashMap::new())),
            mcp_registry,
        }
    }

    /// Reload config from disk and rebuild LLM if tokens changed.
    pub async fn reload_config(&self) {
        let new_config = match config::load_config() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("failed to reload config: {e}");
                return;
            }
        };

        let (tokens_changed, mcp_changed) = {
            let old = self.config.read().await;
            let tokens = old.llm.tokens.anthropic != new_config.llm.tokens.anthropic
                || old.llm.tokens.open_ai != new_config.llm.tokens.open_ai
                || old.llm.provider != new_config.llm.provider
                || old.llm.model != new_config.llm.model;
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

        // Preserve plan from DB — it's not in config.toml
        let mut cfg = self.config.write().await;
        let plan = cfg.plan.clone();
        *cfg = new_config;
        if cfg.plan.is_empty() {
            cfg.plan = plan;
        }
    }

    pub async fn rebuild_llm(&self, config: &Config) {
        let new_llm = LlmBackend::from_config(config);
        *self.llm.write().await = new_llm;
    }
}
