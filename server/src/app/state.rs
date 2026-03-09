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
    pub config: Arc<Config>,
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
            config: Arc::new(config),
            workspace_dir: config::workspace_root(),
            events,
            llm: Arc::new(RwLock::new(llm)),
            embedding_model: Arc::new(RwLock::new(embedding_model)),
        }
    }

    pub async fn rebuild_llm(&self, config: &Config) {
        let new_llm = LlmBackend::from_config(config);
        let new_embedding = build_embedding_model(config);
        let mut llm_guard = self.llm.write().await;
        *llm_guard = new_llm;
        let mut emb_guard = self.embedding_model.write().await;
        *emb_guard = new_embedding;
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
