use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use tokio::sync::broadcast;

use crate::domain::chat::{ChatMessage, ChatRole};
use crate::domain::events::ServerEvent;
use crate::services::tools::ScheduledMessage;

static SCHED_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Spawn a background task that checks for scheduled messages every 30 seconds.
pub fn start(workspace_dir: &Path, events: broadcast::Sender<ServerEvent>) {
    let workspace = workspace_dir.to_path_buf();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            check_and_deliver(&workspace, &events);
        }
    });
    log::info!("scheduler started — checking for scheduled messages every 30s");
}

fn check_and_deliver(workspace_dir: &Path, events: &broadcast::Sender<ServerEvent>) {
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
                tool_name: None, mcp_app_html: None, mcp_app_input: None,
            };

            // Append to chat history
            append_scheduled_message(workspace_dir, &instance_slug, &message);

            // Broadcast via WebSocket
            match events.send(ServerEvent::ChatMessageCreated {
                instance_slug: instance_slug.clone(),
                chat_id: "default".to_string(),
                message,
            }) {
                Ok(n) => log::info!("broadcast scheduled message to {n} receivers"),
                Err(e) => log::warn!("broadcast failed (no receivers?): {e}"),
            }

            // Remove the scheduled file
            let _ = fs::remove_file(&path);

            log::info!("delivered scheduled message for {instance_slug}: {}", scheduled.id);
        }
    }
}

fn append_scheduled_message(workspace_dir: &Path, instance_slug: &str, message: &ChatMessage) {
    let chat_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("chats")
        .join("default");
    let _ = fs::create_dir_all(&chat_dir);
    let messages_path = chat_dir.join("messages.json");

    let lock = crate::services::tools::chat_file_lock(&messages_path);
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

    let mut messages: Vec<ChatMessage> = match fs::read_to_string(&messages_path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    messages.push(message.clone());

    if let Ok(json) = serde_json::to_string_pretty(&messages) {
        let _ = fs::write(&messages_path, json);
    }
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
