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
    let (machine_id, mut agent_rx) = match wait_for_registration(&mut socket, &state).await {
        Some(v) => v,
        None => return,
    };

    log::info!("[machine-ws] agent '{machine_id}' connected");

    loop {
        tokio::select! {
            // Forward toolcalls to the agent
            toolcall_msg = agent_rx.recv() => {
                match toolcall_msg {
                    Some(msg) => {
                        if socket.send(Message::Text(msg.into())).await.is_err() {
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
                                    state.machine_registry.complete(&request_id, result).await;
                                }
                                AgentMessage::Heartbeat { machine_id: mid } => {
                                    state.machine_registry.heartbeat(&mid).await;
                                }
                                AgentMessage::Register { .. } => {
                                    // Already registered, ignore duplicate
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }

    log::info!("[machine-ws] agent '{machine_id}' disconnected");
    state.machine_registry.unregister(&machine_id).await;
}

/// Wait for the agent to send a Register message. Returns (machine_id, mpsc receiver for toolcalls).
async fn wait_for_registration(
    socket: &mut WebSocket,
    state: &AppState,
) -> Option<(String, tokio::sync::mpsc::UnboundedReceiver<String>)> {
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
                        if let Ok(AgentMessage::Register { machine_id, os, hostname, screen_width, screen_height }) =
                            serde_json::from_str::<AgentMessage>(&text)
                        {
                            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                            let info = MachineInfo {
                                machine_id: machine_id.clone(),
                                os,
                                hostname,
                                screen_width,
                                screen_height,
                                last_seen: chrono::Utc::now().timestamp(),
                            };
                            state.machine_registry.register(info, tx).await;

                            // Send ack
                            let ack = serde_json::json!({"type": "registered", "machine_id": machine_id});
                            let _ = socket.send(Message::Text(serde_json::to_string(&ack).unwrap().into())).await;

                            return Some((machine_id, rx));
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
