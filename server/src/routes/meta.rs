use axum::{Json, Router, extract::State, routing::get};

use crate::{
    app::state::AppState,
    domain::meta::{LlmSummary, ServerMetaResponse},
    services::workspace,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/meta", get(server_meta))
}

async fn server_meta(State(state): State<AppState>) -> Json<ServerMetaResponse> {
    let instances_dir = state.workspace_dir.join("instances");
    let skills_dir = state.workspace_dir.join("skills");
    let cfg = state.config.read().await;

    Json(ServerMetaResponse {
        app: "bolly",
        port: cfg.port,
        workspace_dir: state.workspace_dir.display().to_string(),
        instances_count: workspace::count_directories(&instances_dir).unwrap_or(0),
        skills_count: workspace::count_directories(&skills_dir).unwrap_or(0),
        llm: LlmSummary {
            provider: cfg.llm.provider,
            model: cfg.llm.model.clone(),
            openai_configured: !cfg.llm.tokens.open_ai.trim().is_empty(),
            anthropic_configured: !cfg.llm.tokens.anthropic.trim().is_empty(),
        },
    })
}
