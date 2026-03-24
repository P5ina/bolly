use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateLlmRequest {
    pub api_key: String,
}
