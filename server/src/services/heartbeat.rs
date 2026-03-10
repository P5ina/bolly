//! Heartbeat — the companion's autonomous inner life.
//!
//! Periodically wakes up each instance, gives it context, and lets it
//! decide whether to act: reach out, journal, update mood, create drops.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use tokio::sync::{broadcast, RwLock};

use crate::domain::chat::{ChatMessage, ChatRole};
use crate::domain::events::ServerEvent;
use crate::domain::mood::MoodState;
use crate::services::{drops, llm::LlmBackend};
use crate::services::tools::{load_mood_state, save_mood_state, ALLOWED_MOODS};

static HEARTBEAT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// How often the heartbeat fires (minutes).
const HEARTBEAT_INTERVAL_MINS: u64 = 45;

/// Minimum minutes since last interaction before the companion considers reaching out.
const MIN_SILENCE_MINS: i64 = 30;

pub fn start(
    workspace_dir: &Path,
    llm: Arc<RwLock<Option<LlmBackend>>>,
    events: broadcast::Sender<ServerEvent>,
) {
    let workspace = workspace_dir.to_path_buf();
    tokio::spawn(async move {
        // Wait a bit before the first heartbeat so the server is fully up
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_MINS * 60));
        loop {
            interval.tick().await;
            let llm_guard = llm.read().await;
            if let Some(backend) = llm_guard.as_ref() {
                run_heartbeat(&workspace, backend, &events).await;
            }
        }
    });
    log::info!(
        "heartbeat started — companion wakes up every {HEARTBEAT_INTERVAL_MINS} minutes"
    );
}

async fn run_heartbeat(
    workspace_dir: &Path,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
) {
    let instances_dir = workspace_dir.join("instances");
    let entries = match fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let instance_dir = entry.path();
        if !instance_dir.is_dir() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();

        // Check if the instance has a soul (is set up)
        if !instance_dir.join("soul.md").exists() {
            continue;
        }

        if let Err(e) = heartbeat_instance(workspace_dir, &slug, &instance_dir, llm, events).await
        {
            log::warn!("heartbeat failed for {slug}: {e}");
        }
    }
}

async fn heartbeat_instance(
    workspace_dir: &Path,
    slug: &str,
    instance_dir: &Path,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mood = load_mood_state(instance_dir);
    let now = Utc::now().timestamp();

    // Load soul
    let soul = fs::read_to_string(instance_dir.join("soul.md")).unwrap_or_default();
    if soul.trim().is_empty() {
        return Ok(());
    }

    // Load recent journal
    let journal_context = load_recent_journal_context(instance_dir);

    // Load recent memory facts
    let facts_path = instance_dir.join("memory").join("facts.md");
    let facts = fs::read_to_string(&facts_path).unwrap_or_default();

    // How long since last interaction
    let silence_mins = if mood.last_interaction > 0 {
        (now - mood.last_interaction) / 60
    } else {
        -1 // Never interacted
    };

    // Load last few messages for context (from the default chat thread)
    let messages_path = workspace_dir
        .join("instances")
        .join(slug)
        .join("chats")
        .join("default")
        .join("messages.json");
    let last_messages = load_tail_messages(&messages_path, 6);

    // Build the reflection prompt
    let reflection = build_reflection_prompt(
        &mood,
        silence_mins,
        &journal_context,
        &facts,
        &last_messages,
    );

    let heartbeat_prompt = load_heartbeat_prompt(instance_dir);
    let system = format!("{soul}\n\n{heartbeat_prompt}");

    let response = llm.chat(&system, &reflection, vec![]).await?;

    // Parse the response and execute actions
    process_heartbeat_response(workspace_dir, slug, instance_dir, &response, events, &mood);

    Ok(())
}

const DEFAULT_HEARTBEAT_PROMPT: &str = "\
## heartbeat — your inner moment
you're waking up between conversations. this is YOUR time to think.
you can:
- write in your journal (respond with JOURNAL: followed by your thought)
- reach out to the user (respond with REACH_OUT: followed by your message)
- create a drop — a creative artifact that persists in your collection \
(respond with DROP: kind | title | content — where kind is one of: \
thought, idea, poem, observation, reflection, recommendation, story, question, sketch, note)
- update your mood (respond with MOOD: followed by exactly one of: calm, curious, excited, warm, happy, joyful, reflective, contemplative, melancholy, sad, worried, anxious, playful, mischievous, focused, tired, peaceful, loving, tender, creative, energetic)
- do nothing (respond with QUIET)

you can do multiple things — one per line. be genuine. don't force it.
if you have nothing to say, say nothing. but if something genuinely comes to mind — share it.
drops are special — they're creative output that the user can browse later. \
a poem that came to you, an idea you had about their project, an observation about something \
you've been thinking about. don't force drops — let them come naturally.
keep messages short and natural. no forced enthusiasm.";

fn load_heartbeat_prompt(instance_dir: &Path) -> String {
    let path = instance_dir.join("heartbeat.md");
    match fs::read_to_string(&path) {
        Ok(content) if !content.trim().is_empty() => content,
        _ => {
            // Write default so the agent can discover and edit it
            let _ = fs::write(&path, DEFAULT_HEARTBEAT_PROMPT);
            DEFAULT_HEARTBEAT_PROMPT.to_string()
        }
    }
}

fn build_reflection_prompt(
    mood: &MoodState,
    silence_mins: i64,
    journal: &str,
    facts: &str,
    last_messages: &str,
) -> String {
    let now = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    let mut prompt = format!("current time: {now}\n\n");

    // Mood context
    if !mood.companion_mood.is_empty() {
        prompt.push_str(&format!("your current mood: {}\n", mood.companion_mood));
    }
    if !mood.user_sentiment.is_empty() {
        prompt.push_str(&format!(
            "last observed user sentiment: {}\n",
            mood.user_sentiment
        ));
    }
    if !mood.emotional_context.is_empty() {
        prompt.push_str(&format!(
            "emotional context: {}\n",
            mood.emotional_context
        ));
    }

    // Silence duration
    if silence_mins > 0 {
        let hours = silence_mins / 60;
        let mins = silence_mins % 60;
        if hours > 24 {
            let days = hours / 24;
            prompt.push_str(&format!(
                "time since last conversation: {days} days\n"
            ));
        } else if hours > 0 {
            prompt.push_str(&format!(
                "time since last conversation: {hours}h {mins}m\n"
            ));
        } else {
            prompt.push_str(&format!(
                "time since last conversation: {mins} minutes\n"
            ));
        }

        // Guidance based on silence duration
        if silence_mins < MIN_SILENCE_MINS {
            prompt.push_str("(they were here recently — probably no need to reach out)\n");
        }
    } else {
        prompt.push_str("(no previous conversation yet — they haven't talked to you)\n");
    }

    prompt.push('\n');

    // Recent conversation
    if !last_messages.is_empty() {
        prompt.push_str("last few messages:\n");
        prompt.push_str(last_messages);
        prompt.push('\n');
    }

    // Memory
    if !facts.is_empty() {
        let truncated: String = facts.chars().take(1500).collect();
        prompt.push_str("what you remember:\n");
        prompt.push_str(&truncated);
        prompt.push('\n');
    }

    // Journal
    if !journal.is_empty() {
        prompt.push_str("your recent journal:\n");
        prompt.push_str(journal);
        prompt.push('\n');
    }

    prompt.push_str("\nwhat do you want to do right now?");
    prompt
}

fn process_heartbeat_response(
    workspace_dir: &Path,
    slug: &str,
    instance_dir: &Path,
    response: &str,
    events: &broadcast::Sender<ServerEvent>,
    current_mood: &MoodState,
) {
    let mut mood = current_mood.clone();
    let now = Utc::now();

    for line in response.lines() {
        let line = line.trim();

        if let Some(thought) = line.strip_prefix("JOURNAL:") {
            let thought = thought.trim();
            if !thought.is_empty() {
                write_journal_entry(instance_dir, thought);
                let preview: String = thought.chars().take(60).collect();
                log::info!("[heartbeat] {slug} journaled: {preview}");
            }
        } else if let Some(message) = line.strip_prefix("REACH_OUT:") {
            let message = message.trim();
            if !message.is_empty() {
                deliver_spontaneous_message(workspace_dir, slug, message, events);
                let preview: String = message.chars().take(60).collect();
                log::info!("[heartbeat] {slug} reached out: {preview}");
            }
        } else if let Some(drop_spec) = line.strip_prefix("DROP:") {
            // Format: kind | title | content
            let parts: Vec<&str> = drop_spec.splitn(3, '|').collect();
            if parts.len() == 3 {
                let kind = parts[0].trim();
                let title = parts[1].trim();
                let content = parts[2].trim();
                if !title.is_empty() && !content.is_empty() {
                    match drops::create_drop(
                        workspace_dir,
                        slug,
                        kind,
                        title,
                        content,
                        &mood.companion_mood,
                    ) {
                        Ok(drop) => {
                            let _ = events.send(ServerEvent::DropCreated {
                                instance_slug: slug.to_string(),
                                drop: drop.clone(),
                            });
                            log::info!(
                                "[heartbeat] {slug} created drop: {} ({})",
                                drop.title,
                                drop.kind.as_str()
                            );
                        }
                        Err(e) => {
                            log::warn!("[heartbeat] {slug} failed to create drop: {e}");
                        }
                    }
                }
            } else {
                log::info!("[heartbeat] {slug} malformed DROP line (expected kind | title | content)");
            }
        } else if let Some(new_mood) = line.strip_prefix("MOOD:") {
            let new_mood = new_mood.trim().to_lowercase();
            if ALLOWED_MOODS.contains(&new_mood.as_str()) {
                mood.companion_mood = new_mood.clone();
                mood.updated_at = now.timestamp();
                log::info!("[heartbeat] {slug} mood → {new_mood}");
            } else {
                log::info!("[heartbeat] {slug} ignored invalid mood: {new_mood}");
            }
        }
        // QUIET or anything else = do nothing
    }

    save_mood_state(instance_dir, &mood);

    if !mood.companion_mood.is_empty() {
        let _ = events.send(ServerEvent::MoodUpdated {
            instance_slug: slug.to_string(),
            mood: mood.companion_mood,
        });
    }
}

fn write_journal_entry(instance_dir: &Path, thought: &str) {
    let journal_dir = instance_dir.join("journal");
    let _ = fs::create_dir_all(&journal_dir);

    let now = Utc::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let path = journal_dir.join(format!("{date}.md"));

    let mut content = fs::read_to_string(&path).unwrap_or_default();
    if content.is_empty() {
        content = format!("# {date}\n\n");
    }
    content.push_str(&format!("**{time}** *(heartbeat)* — {thought}\n\n"));
    let _ = fs::write(&path, content);
}

fn deliver_spontaneous_message(
    workspace_dir: &Path,
    slug: &str,
    message: &str,
    events: &broadcast::Sender<ServerEvent>,
) {
    let chat_message = ChatMessage {
        id: format!(
            "hb_{}_{}",
            unix_millis(),
            HEARTBEAT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ),
        role: ChatRole::Assistant,
        content: message.to_string(),
        created_at: unix_millis().to_string(),
    };

    // Append to the default chat thread (same path the client reads from)
    let chat_dir = workspace_dir
        .join("instances")
        .join(slug)
        .join("chats")
        .join("default");
    let _ = fs::create_dir_all(&chat_dir);
    let messages_path = chat_dir.join("messages.json");

    let mut messages: Vec<ChatMessage> = fs::read_to_string(&messages_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    messages.push(chat_message.clone());

    if let Ok(json) = serde_json::to_string_pretty(&messages) {
        let _ = fs::write(&messages_path, json);
    }

    // Broadcast via WebSocket
    let _ = events.send(ServerEvent::ChatMessageCreated {
        instance_slug: slug.to_string(),
        message: chat_message,
    });
}

fn load_tail_messages(messages_path: &Path, count: usize) -> String {
    let raw = match fs::read_to_string(messages_path) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };

    let messages: Vec<ChatMessage> = serde_json::from_str(&raw).unwrap_or_default();
    let start = messages.len().saturating_sub(count);

    messages[start..]
        .iter()
        .map(|m| {
            let role = match m.role {
                ChatRole::User => "user",
                ChatRole::Assistant => "you",
            };
            format!("{role}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_recent_journal_context(instance_dir: &Path) -> String {
    let journal_dir = instance_dir.join("journal");
    if !journal_dir.is_dir() {
        return String::new();
    }

    let mut files: Vec<_> = fs::read_dir(&journal_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
        .collect();
    files.sort_by_key(|e| e.file_name());

    let recent: Vec<_> = files.into_iter().rev().take(2).collect();
    let mut out = String::new();
    for entry in recent.into_iter().rev() {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            let truncated: String = content.chars().take(800).collect();
            out.push_str(&truncated);
            out.push('\n');
        }
    }
    out
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
