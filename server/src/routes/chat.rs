use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use std::io::ErrorKind;
use tokio_util::sync::CancellationToken;

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
        .route("/api/chat/{instance_slug}/stop", post(stop_agent))
}

async fn post_chat(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let instance_slug = request.instance_slug.clone();
    let content = request.content.trim().to_string();

    if instance_slug.is_empty() || content.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "slug and content required".into()));
    }

    // Save user message immediately
    let user_message = chat::save_user_message(&state.workspace_dir, &instance_slug, &content)
        .map_err(map_chat_error)?;

    // Broadcast user message
    let _ = state.events.send(ServerEvent::ChatMessageCreated {
        instance_slug: instance_slug.clone(),
        message: user_message.clone(),
    });

    // Discover instance
    if let Ok(Some(instance)) = chat::discover_instance(&state.workspace_dir, &instance_slug) {
        let _ = state
            .events
            .send(ServerEvent::InstanceDiscovered { instance });
    }

    // Cancel any existing agent task for this instance
    {
        let mut tasks = state.agent_tasks.lock().await;
        if let Some(token) = tasks.remove(&instance_slug) {
            token.cancel();
        }
    }

    // Create cancellation token for the new agent loop
    let cancel = CancellationToken::new();
    {
        let mut tasks = state.agent_tasks.lock().await;
        tasks.insert(instance_slug.clone(), cancel.clone());
    }

    // Start agent loop in background
    let bg_state = state.clone();
    tokio::spawn(async move {
        run_agent_loop(bg_state, instance_slug, cancel).await;
    });

    // Return immediately with the user message
    Ok(Json(ChatResponse {
        instance_slug: request.instance_slug,
        messages: vec![user_message],
    }))
}

/// Agent loop: keeps calling the LLM until it responds without tool use or is cancelled.
async fn run_agent_loop(state: AppState, instance_slug: String, cancel: CancellationToken) {
    let _ = state.events.send(ServerEvent::AgentRunning {
        instance_slug: instance_slug.clone(),
    });

    // Max outer iterations to prevent infinite loops
    const MAX_ITERATIONS: usize = 20;
    let mut iteration = 0;

    loop {
        if cancel.is_cancelled() {
            log::info!("[agent] {instance_slug} — cancelled by user");
            break;
        }

        if iteration >= MAX_ITERATIONS {
            log::info!("[agent] {instance_slug} — reached max iterations");
            break;
        }

        iteration += 1;

        // Get current LLM/config state
        let config_path = config::config_path();
        let brave_key = {
            let cfg = state.config.read().await;
            cfg.llm.tokens.brave_search.clone()
        };

        let llm_guard = state.llm.read().await;
        let llm_ref = match llm_guard.as_ref() {
            Some(l) => l.clone(),
            None => {
                log::warn!("[agent] {instance_slug} — no LLM configured");
                break;
            }
        };
        drop(llm_guard);

        let emb_guard = state.embedding_model.read().await;
        let emb_clone = emb_guard.clone();
        drop(emb_guard);

        // Run one LLM turn (rig handles internal tool loop up to 8 sub-turns)
        let result = chat::run_single_turn(
            &state.workspace_dir,
            &config_path,
            &instance_slug,
            &llm_ref,
            emb_clone.as_ref(),
            if brave_key.is_empty() { None } else { Some(brave_key.as_str()) },
            state.events.clone(),
        )
        .await;

        // Reload config in case LLM changed it
        state.reload_config().await;

        match result {
            Ok(assistant_messages) => {
                // Broadcast each message chunk separately
                for msg in &assistant_messages {
                    let _ = state.events.send(ServerEvent::ChatMessageCreated {
                        instance_slug: instance_slug.clone(),
                        message: msg.clone(),
                    });
                }

                // Check if the last message suggests more work is needed
                let last_content = assistant_messages
                    .last()
                    .map(|m| m.content.as_str())
                    .unwrap_or("");
                if !should_continue(last_content) {
                    break;
                }

                log::info!(
                    "[agent] {instance_slug} — iteration {iteration}, continuing"
                );

                // Inject a synthetic "continue" prompt so the LLM keeps going
                let continue_msg = chat::save_user_message(
                    &state.workspace_dir,
                    &instance_slug,
                    "[continue — the user is waiting for you to finish]",
                )
                .ok();

                // Don't broadcast the synthetic continue message to the UI
                if continue_msg.is_none() {
                    log::warn!("[agent] {instance_slug} — failed to save continue message");
                    break;
                }
            }
            Err(e) => {
                log::warn!("[agent] {instance_slug} — error: {e}");
                break;
            }
        }
    }

    // Clean up
    {
        let mut tasks = state.agent_tasks.lock().await;
        tasks.remove(&instance_slug);
    }

    let _ = state.events.send(ServerEvent::AgentStopped {
        instance_slug: instance_slug.clone(),
    });
}

/// Heuristic: should the agent continue with another iteration?
/// Returns true if the response indicates more work is needed.
fn should_continue(response: &str) -> bool {
    let lower = response.to_lowercase();
    // Explicit continuation signals
    let continue_phrases = [
        "let me continue",
        "i'll continue",
        "continuing",
        "next step",
        "now i'll",
        "now let me",
        "moving on to",
        "let me now",
        "i'll now",
        "working on",
        "let me work on",
        "[continue]",
    ];
    for phrase in &continue_phrases {
        if lower.contains(phrase) {
            return true;
        }
    }
    false
}

async fn stop_agent(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> StatusCode {
    let mut tasks = state.agent_tasks.lock().await;
    if let Some(token) = tasks.remove(&instance_slug) {
        token.cancel();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
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
