use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use std::io::ErrorKind;
use tokio_util::sync::CancellationToken;

use crate::{
    app::state::AppState,
    config,
    domain::{
        chat::{ChatRequest, ChatResponse, ChatRole, ChatSummary},
        events::ServerEvent,
    },
    services::{chat, rate_limit},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chat", post(post_chat))
        .route("/api/chat/{instance_slug}/chats", get(list_chats))
        .route("/api/chat/{instance_slug}/messages", get(get_messages_default))
        .route("/api/chat/{instance_slug}/{chat_id}/messages", get(get_messages))
        .route("/api/chat/{instance_slug}/{chat_id}/stop", post(stop_agent))
        .route("/api/chat/{instance_slug}/{chat_id}/context", delete(clear_context))
        // Legacy routes (use default chat_id)
        .route("/api/chat/{instance_slug}/stop", post(stop_agent_default))
        .route("/api/chat/{instance_slug}/context", delete(clear_context_default))
}

fn task_key(slug: &str, chat_id: &str) -> String {
    format!("{slug}/{chat_id}")
}

async fn post_chat(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let instance_slug = request.instance_slug.clone();
    let chat_id = request.chat_id.clone();
    let content = request.content.trim().to_string();

    if instance_slug.is_empty() || content.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "slug and content required".into()));
    }

    // Rate limit check (only when DATABASE_URL is configured)
    if let (Some(pool), Some(iid)) = (&state.pg_pool, &state.instance_id) {
        if let Err(reason) = rate_limit::check(pool, iid).await {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                serde_json::json!({ "error": "rate limit exceeded", "detail": reason }).to_string(),
            ));
        }
    }

    // Save user message immediately
    let user_message = chat::save_user_message(&state.workspace_dir, &instance_slug, &chat_id, &content)
        .map_err(map_chat_error)?;

    // Broadcast user message
    let _ = state.events.send(ServerEvent::ChatMessageCreated {
        instance_slug: instance_slug.clone(),
        chat_id: chat_id.clone(),
        message: user_message.clone(),
    });

    // Discover instance
    if let Ok(Some(instance)) = chat::discover_instance(&state.workspace_dir, &instance_slug) {
        let _ = state
            .events
            .send(ServerEvent::InstanceDiscovered { instance });
    }

    let key = task_key(&instance_slug, &chat_id);

    // If an agent is already running for this chat, don't start another one.
    // The running agent will pick up the new message on its next turn
    // (it re-reads messages from disk each iteration).
    let already_running = {
        let tasks = state.agent_tasks.lock().await;
        tasks.contains_key(&key)
    };

    if !already_running {
        let cancel = CancellationToken::new();
        {
            let mut tasks = state.agent_tasks.lock().await;
            // Double-check after re-acquiring lock
            if !tasks.contains_key(&key) {
                tasks.insert(key.clone(), cancel.clone());
            }
        }

        let bg_state = state.clone();
        let bg_chat_id = chat_id.clone();
        tokio::spawn(async move {
            run_agent_loop(bg_state, instance_slug, bg_chat_id, cancel).await;
        });
    }

    // Return immediately with the user message
    Ok(Json(ChatResponse {
        instance_slug: request.instance_slug,
        chat_id: request.chat_id,
        messages: vec![user_message],
        agent_running: true,
    }))
}

/// Agent loop: keeps calling the LLM until it responds without tool use or is cancelled.
/// New user messages are automatically picked up because each turn re-reads from disk.
async fn run_agent_loop(state: AppState, instance_slug: String, chat_id: String, cancel: CancellationToken) {
    let _ = state.events.send(ServerEvent::AgentRunning {
        instance_slug: instance_slug.clone(),
        chat_id: chat_id.clone(),
    });

    const MAX_ITERATIONS: usize = 20;
    let mut iteration = 0;

    loop {
        if cancel.is_cancelled() {
            log::info!("[agent] {instance_slug}/{chat_id} — cancelled by user");
            break;
        }

        if iteration >= MAX_ITERATIONS {
            log::info!("[agent] {instance_slug}/{chat_id} — reached max iterations");
            break;
        }

        iteration += 1;

        let config_path = config::config_path();
        let brave_key = {
            let cfg = state.config.read().await;
            cfg.llm.tokens.brave_search.clone()
        };

        let llm_guard = state.llm.read().await;
        let llm_ref = match llm_guard.as_ref() {
            Some(l) => l.clone(),
            None => {
                log::warn!("[agent] {instance_slug}/{chat_id} — no LLM configured");
                break;
            }
        };
        drop(llm_guard);

        let emb_guard = state.embedding_model.read().await;
        let emb_clone = emb_guard.clone();
        drop(emb_guard);

        // Wrap single turn in timeout + cancellation so we never hang forever.
        const TURN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

        let turn_fut = chat::run_single_turn(
            &state.workspace_dir,
            &config_path,
            &instance_slug,
            &chat_id,
            &llm_ref,
            emb_clone.as_ref(),
            if brave_key.is_empty() { None } else { Some(brave_key.as_str()) },
            state.events.clone(),
        );

        let result = tokio::select! {
            r = tokio::time::timeout(TURN_TIMEOUT, turn_fut) => {
                match r {
                    Ok(inner) => inner,
                    Err(_) => {
                        log::warn!("[agent] {instance_slug}/{chat_id} — turn timed out after {}s", TURN_TIMEOUT.as_secs());
                        Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "turn timed out — the model or a tool took too long"))
                    }
                }
            }
            _ = cancel.cancelled() => {
                log::info!("[agent] {instance_slug}/{chat_id} — cancelled during turn");
                break;
            }
        };

        // Reload config in case LLM changed it
        state.reload_config().await;

        match result {
            Ok(turn) => {
                for msg in &turn.messages {
                    // Don't broadcast internal tool activity logs as chat messages
                    if msg.content.starts_with("[tool activity]") {
                        continue;
                    }
                    let _ = state.events.send(ServerEvent::ChatMessageCreated {
                        instance_slug: instance_slug.clone(),
                        chat_id: chat_id.clone(),
                        message: msg.clone(),
                    });
                }

                // Record usage for rate limiting
                if let (Some(pool), Some(iid)) = (&state.pg_pool, &state.instance_id) {
                    let tokens: i32 = turn.messages.iter()
                        .map(|m| rate_limit::estimate_tokens(&m.content))
                        .sum();
                    rate_limit::record_usage(pool, iid, tokens).await;
                }

                // Always continue if the agent was cut short by the turn limit
                if turn.hit_turn_limit {
                    log::info!(
                        "[agent] {instance_slug}/{chat_id} — iteration {iteration}, hit turn limit, continuing"
                    );
                } else {
                    let last_content = turn.messages
                        .last()
                        .map(|m| m.content.as_str())
                        .unwrap_or("");

                    if !should_continue(last_content) {
                        if has_pending_user_message(&state, &instance_slug, &chat_id).await {
                            log::info!("[agent] {instance_slug}/{chat_id} — new user message arrived, continuing");
                            continue;
                        }
                        break;
                    }
                }

                log::info!(
                    "[agent] {instance_slug}/{chat_id} — iteration {iteration}, continuing"
                );

                let continue_msg = chat::save_user_message(
                    &state.workspace_dir,
                    &instance_slug,
                    &chat_id,
                    "[continue — the user is waiting for you to finish]",
                )
                .ok();

                if continue_msg.is_none() {
                    log::warn!("[agent] {instance_slug}/{chat_id} — failed to save continue message");
                    break;
                }
            }
            Err(e) => {
                let msg = e.to_string();
                log::warn!("[agent] {instance_slug}/{chat_id} — error: {msg}");

                // Let the client know what happened
                let error_label = if msg.contains("rate limit") || msg.contains("429") {
                    "rate limited — try again in a moment"
                } else if msg.contains("timed out") {
                    "request timed out"
                } else {
                    "something went wrong"
                };
                let error_msg = chat::save_system_message(
                    &state.workspace_dir, &instance_slug, &chat_id,
                    &format!("[system] {error_label}"),
                );
                if let Ok(m) = error_msg {
                    let _ = state.events.send(ServerEvent::ChatMessageCreated {
                        instance_slug: instance_slug.clone(),
                        chat_id: chat_id.clone(),
                        message: m,
                    });
                }
                break;
            }
        }
    }

    // Auto-generate title if missing
    if let Ok(response) = chat::load_messages(&state.workspace_dir, &instance_slug, &chat_id) {
        let needs_title = chat::get_chat_title(&state.workspace_dir, &instance_slug, &chat_id)
            .map(|t| t.is_empty())
            .unwrap_or(true);

        if needs_title && !response.messages.is_empty() {
            let llm_guard = state.llm.read().await;
            if let Some(llm) = llm_guard.as_ref() {
                let snippet: String = response
                    .messages
                    .iter()
                    .take(6)
                    .map(|m| format!("{}: {}", if m.role == ChatRole::User { "user" } else { "assistant" }, m.content.chars().take(200).collect::<String>()))
                    .collect::<Vec<_>>()
                    .join("\n");

                let prompt = format!(
                    "Generate a very short title (3-6 words, no quotes) for this conversation:\n\n{snippet}"
                );

                if let Ok(title) = llm.chat("You generate short chat titles. Respond with only the title, nothing else.", &prompt, vec![]).await {
                    let title = title.trim().trim_matches('"').to_string();
                    let _ = chat::update_chat_title(&state.workspace_dir, &instance_slug, &chat_id, &title);
                }
            }
        }
    }

    // Clean up
    let key = task_key(&instance_slug, &chat_id);
    {
        let mut tasks = state.agent_tasks.lock().await;
        tasks.remove(&key);
    }

    let _ = state.events.send(ServerEvent::AgentStopped {
        instance_slug: instance_slug.clone(),
        chat_id: chat_id.clone(),
    });
}

/// Check if the last message in the chat is from the user (meaning they sent something
/// while the agent was processing and we should do another turn).
async fn has_pending_user_message(state: &AppState, instance_slug: &str, chat_id: &str) -> bool {
    match chat::load_messages(&state.workspace_dir, instance_slug, chat_id) {
        Ok(response) => {
            response.messages.last().is_some_and(|m| m.role == ChatRole::User)
        }
        Err(_) => false,
    }
}

fn should_continue(response: &str) -> bool {
    let lower = response.to_lowercase();
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

async fn list_chats(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Result<Json<Vec<ChatSummary>>, (StatusCode, String)> {
    let chats = chat::list_chats(&state.workspace_dir, &instance_slug)
        .map_err(map_chat_error)?;
    Ok(Json(chats))
}

async fn get_messages_default(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let mut response =
        chat::load_messages(&state.workspace_dir, &instance_slug, "default").map_err(map_chat_error)?;
    response.agent_running = is_agent_running(&state, &instance_slug, "default").await;
    Ok(Json(response))
}

async fn get_messages(
    State(state): State<AppState>,
    Path((instance_slug, chat_id)): Path<(String, String)>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let mut response =
        chat::load_messages(&state.workspace_dir, &instance_slug, &chat_id).map_err(map_chat_error)?;
    response.agent_running = is_agent_running(&state, &instance_slug, &chat_id).await;
    Ok(Json(response))
}

async fn is_agent_running(state: &AppState, instance_slug: &str, chat_id: &str) -> bool {
    let key = task_key(instance_slug, chat_id);
    let tasks = state.agent_tasks.lock().await;
    tasks.contains_key(&key)
}

async fn stop_agent(
    State(state): State<AppState>,
    Path((instance_slug, chat_id)): Path<(String, String)>,
) -> StatusCode {
    let key = task_key(&instance_slug, &chat_id);
    let mut tasks = state.agent_tasks.lock().await;
    if let Some(token) = tasks.remove(&key) {
        token.cancel();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn stop_agent_default(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> StatusCode {
    let key = task_key(&instance_slug, "default");
    let mut tasks = state.agent_tasks.lock().await;
    if let Some(token) = tasks.remove(&key) {
        token.cancel();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn clear_context(
    State(state): State<AppState>,
    Path((instance_slug, chat_id)): Path<(String, String)>,
) -> StatusCode {
    chat::clear_context(&state.workspace_dir, &instance_slug, &chat_id);
    StatusCode::OK
}

async fn clear_context_default(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> StatusCode {
    chat::clear_context(&state.workspace_dir, &instance_slug, "default");
    StatusCode::OK
}

fn map_chat_error(error: std::io::Error) -> (StatusCode, String) {
    let status = match error.kind() {
        ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (status, error.to_string())
}
