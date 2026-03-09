use std::path::PathBuf;
use std::sync::Arc;

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
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (events, _) = broadcast::channel(256);
        let llm = LlmBackend::from_config(&config);

        Self {
            config: Arc::new(config),
            workspace_dir: config::workspace_root(),
            events,
            llm: Arc::new(RwLock::new(llm)),
        }
    }

    pub async fn rebuild_llm(&self, config: &Config) {
        let new_llm = LlmBackend::from_config(config);
        let mut guard = self.llm.write().await;
        *guard = new_llm;
    }
}
