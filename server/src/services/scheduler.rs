use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use tokio::sync::broadcast;

use crate::app::state::AppState;
use crate::domain::chat::{ChatMessage, ChatRole};
use crate::domain::events::ServerEvent;
use crate::services::tools::ScheduledMessage;

static SCHED_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Spawn a background task that checks for scheduled messages every 30 seconds.
pub fn start(state: AppState) {
    let workspace = state.workspace_dir.clone();
    let events = state.events.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            check_and_deliver(&workspace, &events, &state).await;
        }
    });
    log::info!("scheduler started — checking for scheduled messages every 30s");
}

async fn check_and_deliver(workspace_dir: &Path, events: &broadcast::Sender<ServerEvent>, state: &AppState) {
    let instances_dir = workspace_dir.join("instances");
    let entries = match fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let now = Utc::now().timestamp();

    for entry in entries.filter_map(Result::ok) {
        let instance_dir = entry.path();
        if !instance_dir.is_dir() {
            continue;
        }
        let instance_slug = entry.file_name().to_string_lossy().to_string();
        let schedule_dir = instance_dir.join("scheduled");
        if !schedule_dir.is_dir() {
            continue;
        }

        let files = match fs::read_dir(&schedule_dir) {
            Ok(f) => f,
            Err(_) => continue,
        };

        for file in files.filter_map(Result::ok) {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let raw = match fs::read_to_string(&path) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let scheduled: ScheduledMessage = match serde_json::from_str(&raw) {
                Ok(s) => s,
                Err(_) => {
                    // Corrupt file, remove it
                    let _ = fs::remove_file(&path);
                    continue;
                }
            };

            if scheduled.deliver_at > now {
                continue; // Not yet due
            }

            // Deliver the message
            let message = ChatMessage {
                id: format!(
                    "sched_{}_{}",
                    unix_millis(),
                    SCHED_COUNTER.fetch_add(1, Ordering::Relaxed)
                ),
                role: ChatRole::Assistant,
                content: scheduled.message.clone(),
                created_at: unix_millis().to_string(),
                kind: Default::default(),
                tool_name: None, mcp_app_html: None, mcp_app_input: None, model: None,
            };

            // Append to rig_history (the single source of truth for chat)
            let rig_path = crate::services::chat::rig_history_path(workspace_dir, &instance_slug, "default");
            let entry = crate::services::llm::HistoryEntry {
                message: crate::services::llm::Message::assistant(&message.content),
                ts: Some(message.created_at.clone()),
                id: Some(message.id.clone()),
                mcp_app_html: None,
                mcp_app_input: None,
                model: None,
            };
            crate::services::chat::append_to_rig_history(&rig_path, &entry);

            // Broadcast via WebSocket
            let msg_id = message.id.clone();
            let msg_content = message.content.clone();
            match events.send(ServerEvent::ChatMessageCreated {
                instance_slug: instance_slug.clone(),
                chat_id: "default".to_string(),
                message,
            }) {
                Ok(n) => log::info!("broadcast scheduled message to {n} receivers"),
                Err(e) => log::warn!("broadcast failed (no receivers?): {e}"),
            }

            // Synthesize TTS if voice_enabled for this instance
            let inst_config = crate::config::InstanceConfig::load(workspace_dir, &instance_slug);
            if inst_config.voice_enabled {
                let api_key = {
                    let cfg = state.config.read().await;
                    cfg.llm.tokens.elevenlabs.clone()
                };
                if !api_key.is_empty() {
                    let voice_id = crate::routes::tts::resolve_voice_id(workspace_dir, &instance_slug);
                    let mood = crate::services::tools::companion::load_mood_state(&instance_dir).companion_mood;
                    match crate::routes::tts::synthesize_bytes(&state.http_client, &api_key, &voice_id, &msg_content, &mood).await {
                        Ok(audio_bytes) => {
                            use base64::Engine;
                            let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&audio_bytes);
                            let _ = events.send(ServerEvent::ChatAudioReady {
                                instance_slug: instance_slug.clone(),
                                chat_id: "default".to_string(),
                                audio_base64,
                                message_ids: vec![msg_id.clone()],
                            });
                        }
                        Err(e) => log::warn!("[scheduler-tts] failed for {}: {e}", msg_id),
                    }
                }
            }

            // Remove the scheduled file
            let _ = fs::remove_file(&path);

            log::info!("delivered scheduled message for {instance_slug}: {}", scheduled.id);
        }
    }
}


fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
