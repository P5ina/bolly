use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use std::io::ErrorKind;

use crate::{
    app::state::AppState,
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
    let llm_guard = state.llm.read().await;
    let llm_ref = llm_guard.as_ref();
    let response = chat::append_chat_turn(&state.workspace_dir, request, llm_ref)
        .await
        .map_err(map_chat_error)?;

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
