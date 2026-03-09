use serde::Deserialize;

use crate::config::LlmProvider;

#[derive(Debug, Deserialize)]
pub struct UpdateLlmRequest {
    pub provider: LlmProvider,
    pub model: Option<String>,
    pub api_key: String,
}
