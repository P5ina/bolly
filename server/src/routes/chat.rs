use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use std::io::ErrorKind;

use crate::{
    app::state::AppState,
    config,
    domain::{
        chat::{ChatRequest, ChatResponse},
        events::ServerEvent,
    },
    services::chat,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chat", post(post_chat))
        .route("/api/chat/{instance_slug}/messages", get(get_messages))
}

async fn post_chat(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let config_path = config::config_path();
    let brave_key = {
        let cfg = state.config.read().await;
        cfg.llm.tokens.brave_search.clone()
    };
    let brave_ref = if brave_key.is_empty() { None } else { Some(brave_key.as_str()) };

    let llm_guard = state.llm.read().await;
    let llm_ref = llm_guard.as_ref();
    let emb_guard = state.embedding_model.read().await;
    let emb_ref = emb_guard.as_ref();

    let response = chat::append_chat_turn(&state.workspace_dir, &config_path, request, llm_ref, emb_ref, brave_ref)
        .await
        .map_err(map_chat_error)?;

    // Drop locks before reload
    drop(llm_guard);
    drop(emb_guard);

    // Reload config from disk in case the LLM saved new API keys via set_api_key tool
    state.reload_config().await;

    for message in &response.messages {
        let _ = state.events.send(ServerEvent::ChatMessageCreated {
            instance_slug: response.instance_slug.clone(),
            message: message.clone(),
        });
    }

    if let Ok(Some(instance)) =
        chat::discover_instance(&state.workspace_dir, &response.instance_slug)
    {
        let _ = state
            .events
            .send(ServerEvent::InstanceDiscovered { instance });
    }

    Ok(Json(response))
}

async fn get_messages(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let response =
        chat::load_messages(&state.workspace_dir, &instance_slug).map_err(map_chat_error)?;
    Ok(Json(response))
}

fn map_chat_error(error: std::io::Error) -> (StatusCode, String) {
    let status = match error.kind() {
        ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (status, error.to_string())
}
