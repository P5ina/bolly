use serde::Serialize;

use crate::config::LlmProvider;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct ServerMetaResponse {
    pub app: &'static str,
    pub version: &'static str,
    pub commit: &'static str,
    pub port: u16,
    pub workspace_dir: String,
    pub instances_count: usize,
    pub skills_count: usize,
    pub llm: LlmSummary,
}

#[derive(Serialize)]
pub struct LlmSummary {
    pub provider: Option<LlmProvider>,
    pub model: Option<String>,
    pub openai_configured: bool,
    pub anthropic_configured: bool,
}
