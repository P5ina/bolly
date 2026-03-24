use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateLlmRequest {
    pub model: Option<String>,
    pub api_key: String,
}
