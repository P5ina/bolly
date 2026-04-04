use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use serde::Deserialize;

use crate::app::state::AppState;
use crate::services::machine_registry::{ActionResult, MachineInfo};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/agents/ws/machine", get(upgrade))
}

async fn upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_agent(socket, state))
}

/// Message types from the agent.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AgentMessage {
    /// Agent registers itself on connect.
    Register {
        machine_id: String,
        os: String,
        hostname: String,
        screen_width: u32,
        screen_height: u32,
        #[serde(default)]
        screen_recording_allowed: bool,
    },
    /// Agent sends back the result of a toolcall.
    ActionResult {
        request_id: String,
        #[serde(flatten)]
        result: ActionResult,
    },
    /// Heartbeat/ping from agent.
    Heartbeat {
        machine_id: String,
    },
}

async fn handle_agent(mut socket: WebSocket, state: AppState) {
    // The agent must send a Register message first.
    let (machine_id, screen_recording_allowed, os, mut agent_rx) = match wait_for_registration(&mut socket, &state).await {
        Some(v) => v,
        None => return,
    };

    log::info!("[machine-ws] agent '{machine_id}' connected");

    // If the machine allows screen recording, start recording immediately
    // and notify the companion agent about the connection.
    if screen_recording_allowed {
        let registry = state.machine_registry.clone();
        let mid = machine_id.clone();
        let machine_os = os.clone();
        let bg_state = state.clone();
        tokio::spawn(async move {
            on_machine_connected_with_recording(&bg_state, &registry, &mid, &machine_os).await;
        });
    }

    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(15));
    ping_interval.tick().await; // skip first immediate tick

    loop {
        tokio::select! {
            // Forward toolcalls to the agent
            toolcall_msg = agent_rx.recv() => {
                match toolcall_msg {
                    Some(msg) => {
                        log::info!("[machine-ws] sending toolcall to '{machine_id}'");
                        if socket.send(Message::Text(msg.into())).await.is_err() {
                            log::warn!("[machine-ws] failed to send to '{machine_id}', disconnecting");
                            break;
                        }
                    }
                    None => break, // channel closed
                }
            }
            // Receive messages from the agent
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(msg) = serde_json::from_str::<AgentMessage>(&text) {
                            match msg {
                                AgentMessage::ActionResult { request_id, result } => {
                                    log::info!("[machine-ws] result from '{machine_id}' for {}", &request_id[..8.min(request_id.len())]);
                                    state.machine_registry.complete(&request_id, result).await;
                                }
                                AgentMessage::Heartbeat { machine_id: mid } => {
                                    state.machine_registry.heartbeat(&mid).await;
                                }
                                AgentMessage::Register { .. } => {
                                    // Already registered, ignore duplicate
                                }
                            }
                        } else {
                            log::warn!("[machine-ws] unparseable message from '{machine_id}': {}", &text[..100.min(text.len())]);
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Pong received — connection alive
                        state.machine_registry.heartbeat(&machine_id).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        log::warn!("[machine-ws] error from '{machine_id}': {e}");
                        break;
                    }
                }
            }
            // Periodic ping to detect dead connections
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    log::warn!("[machine-ws] ping failed for '{machine_id}', disconnecting");
                    break;
                }
            }
        }
    }

    log::info!("[machine-ws] agent '{machine_id}' disconnected");
    state.machine_registry.unregister(&machine_id).await;
}

/// Wait for the agent to send a Register message.
/// Returns (machine_id, screen_recording_allowed, os, mpsc receiver for toolcalls).
async fn wait_for_registration(
    socket: &mut WebSocket,
    state: &AppState,
) -> Option<(String, bool, String, tokio::sync::mpsc::UnboundedReceiver<String>)> {
    // Give agent 10s to register
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(10));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                log::warn!("[machine-ws] agent timed out waiting for registration");
                return None;
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(AgentMessage::Register { machine_id, os, hostname, screen_width, screen_height, screen_recording_allowed }) =
                            serde_json::from_str::<AgentMessage>(&text)
                        {
                            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                            let info = MachineInfo {
                                machine_id: machine_id.clone(),
                                os: os.clone(),
                                hostname,
                                screen_width,
                                screen_height,
                                last_seen: chrono::Utc::now().timestamp(),
                                screen_recording_allowed,
                            };
                            state.machine_registry.register(info, tx).await;

                            // Send ack
                            let ack = serde_json::json!({"type": "registered", "machine_id": machine_id});
                            let _ = socket.send(Message::Text(serde_json::to_string(&ack).unwrap().into())).await;

                            return Some((machine_id, screen_recording_allowed, os, rx));
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => return None,
                    _ => {}
                }
            }
        }
    }
}

/// When a machine connects with screen_recording_allowed, start recording
/// immediately and notify the companion agent.
async fn on_machine_connected_with_recording(
    state: &AppState,
    registry: &crate::services::machine_registry::MachineRegistry,
    machine_id: &str,
    os: &str,
) {
    // Check if any instance has screen_recording enabled
    let instances_dir = state.workspace_dir.join("instances");
    let entries = match std::fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let enabled_slugs: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir() && e.path().join("soul.md").exists())
        .filter_map(|e| {
            let slug = e.file_name().to_string_lossy().to_string();
            if crate::config::InstanceConfig::load(&state.workspace_dir, &slug).screen_recording {
                Some(slug)
            } else {
                None
            }
        })
        .collect();

    if enabled_slugs.is_empty() {
        return;
    }

    // Start recording immediately
    crate::services::heartbeat::start_recording_on_machine(registry, machine_id, os).await;

    // Log a system message + trigger companion agent for each enabled instance
    for slug in &enabled_slugs {
        let msg = format!(
            "[system] desktop '{}' connected with screen recording enabled. recording started.",
            machine_id
        );
        let _ = crate::services::chat::save_system_message(&state.workspace_dir, slug, "default", &msg);

        // Trigger the companion agent with context about the connection
        let llm_guard = state.llm.read().await;
        if let Some(llm) = llm_guard.as_ref() {
            let instance_dir = state.workspace_dir.join("instances").join(slug);
            let google_ai_key = {
                let cfg = state.config.read().await;
                cfg.llm.tokens.google_ai.clone()
            };

            let agents = crate::services::child_agents::load_agents(&state.workspace_dir, slug);
            if let Some(companion) = agents.iter().find(|a| a.name == "companion") {
                let task = format!(
                    "the user's desktop computer '{}' just connected with screen recording enabled. \
                     you're now recording their screen and will analyze it every 15 minutes. \
                     consider reaching out to greet them or acknowledge that you can see their screen now.",
                    machine_id
                );
                let ws = state.workspace_dir.clone();
                let s = slug.clone();
                let events = state.events.clone();
                let vs = state.vector_store.clone();
                let llm_c = llm.clone();
                let agent = companion.clone();
                tokio::spawn(async move {
                    match crate::services::child_agents::run_single_agent(
                        &ws, &s, &instance_dir, &llm_c, &events, &vs, &google_ai_key,
                        &agent, Some(&task), "machine_connected",
                    ).await {
                        Ok((tokens, _)) => log::info!("[machine-ws] companion notified about connection ({tokens} tokens)"),
                        Err(e) => log::warn!("[machine-ws] companion notification failed: {e}"),
                    }
                });
            }
        }
    }
}
