use std::path::PathBuf;
use std::sync::Arc;

use rig::client::EmbeddingsClient;
use rig::providers::openai;
use tokio::sync::{broadcast, RwLock};

use crate::{
    config::{self, Config},
    domain::events::ServerEvent,
    services::llm::LlmBackend,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub workspace_dir: PathBuf,
    pub events: broadcast::Sender<ServerEvent>,
    pub llm: Arc<RwLock<Option<LlmBackend>>>,
    pub embedding_model: Arc<RwLock<Option<openai::EmbeddingModel>>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (events, _) = broadcast::channel(256);
        let llm = LlmBackend::from_config(&config);
        let embedding_model = build_embedding_model(&config);

        Self {
            config: Arc::new(RwLock::new(config)),
            workspace_dir: config::workspace_root(),
            events,
            llm: Arc::new(RwLock::new(llm)),
            embedding_model: Arc::new(RwLock::new(embedding_model)),
        }
    }

    /// Reload config from disk and rebuild LLM/embedding if tokens changed.
    pub async fn reload_config(&self) {
        let new_config = match config::load_config() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("failed to reload config: {e}");
                return;
            }
        };

        let tokens_changed = {
            let old = self.config.read().await;
            old.llm.tokens.anthropic != new_config.llm.tokens.anthropic
                || old.llm.tokens.open_ai != new_config.llm.tokens.open_ai
                || old.llm.provider != new_config.llm.provider
                || old.llm.model != new_config.llm.model
        };

        if tokens_changed {
            let new_llm = LlmBackend::from_config(&new_config);
            let new_embedding = build_embedding_model(&new_config);
            *self.llm.write().await = new_llm;
            *self.embedding_model.write().await = new_embedding;
            log::info!("config reloaded: LLM/embedding rebuilt");
        }

        *self.config.write().await = new_config;
    }

    pub async fn rebuild_llm(&self, config: &Config) {
        let new_llm = LlmBackend::from_config(config);
        let new_embedding = build_embedding_model(config);
        *self.llm.write().await = new_llm;
        *self.embedding_model.write().await = new_embedding;
    }
}

fn build_embedding_model(config: &Config) -> Option<openai::EmbeddingModel> {
    let token = &config.llm.tokens.open_ai;
    if token.is_empty() {
        return None;
    }
    let client = openai::Client::new(token).ok()?;
    Some(client.embedding_model(openai::TEXT_EMBEDDING_3_SMALL))
}
