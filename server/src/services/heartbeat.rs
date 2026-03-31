//! Heartbeat — the companion's autonomous inner life.
//!
//! Periodically wakes up each instance, gives it context, and lets it
//! decide whether to act: reach out, update mood, create drops.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Timelike, Utc};
use tokio::sync::{broadcast, RwLock};

use crate::domain::chat::ChatRole;
use crate::domain::events::ServerEvent;
use crate::domain::mood::MoodState;
use crate::domain::thought::Thought;
use crate::services::{chat, drops, llm::LlmBackend, memory, rhythm, thoughts};
use crate::services::tools::load_mood_state;


/// Minimum minutes since last interaction before the companion considers reaching out.
const MIN_SILENCE_MINS: i64 = 30;

/// Seconds until the next full UTC hour.
fn secs_until_next_hour() -> u64 {
    let now = Utc::now();
    let next_hour_secs = (now.hour() + 1) * 3600;
    let now_secs = now.hour() * 3600 + now.minute() * 60 + now.second();
    if next_hour_secs >= 24 * 3600 {
        (24 * 3600 - now_secs) as u64
    } else {
        (next_hour_secs - now_secs) as u64
    }
}

pub fn start(
    workspace_dir: &Path,
    llm: Arc<RwLock<Option<LlmBackend>>>,
    events: broadcast::Sender<ServerEvent>,
    vector_store: Arc<crate::services::vector::VectorStore>,
    google_ai_key: String,
) {
    // Ensure built-in child agents exist for all instances immediately (don't wait for first tick)
    let instances_dir = workspace_dir.join("instances");
    if let Ok(entries) = fs::read_dir(&instances_dir) {
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_dir() && entry.path().join("soul.md").exists() {
                let slug = entry.file_name().to_string_lossy().to_string();
                crate::services::child_agents::ensure_builtins(workspace_dir, &slug);
            }
        }
    }

    let workspace = workspace_dir.to_path_buf();
    tokio::spawn(async move {
        // Wait until the next full UTC hour before the first heartbeat
        let initial_wait = secs_until_next_hour();
        log::info!("heartbeat: first tick in {}m", initial_wait / 60);
        tokio::time::sleep(Duration::from_secs(initial_wait)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let llm_guard = llm.read().await;
            if let Some(backend) = llm_guard.as_ref() {
                run_heartbeat(&workspace, backend, &events, &vector_store, &google_ai_key).await;
            }
        }
    });
    log::info!("heartbeat started — companion wakes up every hour");
}

async fn run_heartbeat(
    workspace_dir: &Path,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
    vector_store: &Arc<crate::services::vector::VectorStore>,
    google_ai_key: &str,
) {
    let instances_dir = workspace_dir.join("instances");
    let entries = match fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut instances: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir() && e.path().join("soul.md").exists())
        .collect();
    instances.sort_by_key(|e| e.file_name());

    for (i, entry) in instances.iter().enumerate() {
        // Stagger instances to avoid API burst on restart
        if i > 0 {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }

        let instance_dir = entry.path();
        let slug = entry.file_name().to_string_lossy().to_string();

        if let Err(e) = heartbeat_instance(workspace_dir, &slug, &instance_dir, llm, events, vector_store, google_ai_key).await
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
    vector_store: &Arc<crate::services::vector::VectorStore>,
    google_ai_key: &str,
) -> anyhow::Result<()> {
    let mood = load_mood_state(instance_dir);
    let now = Utc::now().timestamp();

    // Load soul
    let soul = fs::read_to_string(instance_dir.join("soul.md")).unwrap_or_default();
    if soul.trim().is_empty() {
        return Ok(());
    }

    // How long since last interaction
    let silence_mins = if mood.last_interaction > 0 {
        (now - mood.last_interaction) / 60
    } else {
        -1 // Never interacted
    };

    // Load last few messages for context
    let rig_path_tail = workspace_dir
        .join("instances")
        .join(slug)
        .join("chats")
        .join("default")
        .join("rig_history.json");
    let last_messages = load_tail_messages(&rig_path_tail, 6);

    // Recompute and persist interaction rhythm
    let rhythm_data = rhythm::recompute_rhythm(workspace_dir, slug);
    rhythm::save_rhythm(instance_dir, &rhythm_data);
    let rhythm_insights = rhythm::build_rhythm_insights(workspace_dir, slug, &rhythm_data);

    // Save rhythm insights to rig_history so the LLM sees updated patterns
    if !rhythm_insights.trim().is_empty() {
        let label = format!("[system] rhythm update\n{rhythm_insights}");
        if let Err(e) = chat::save_system_message(workspace_dir, slug, "default", &label) {
            log::warn!("failed to save rhythm message: {e}");
        }
    }

    // Load recent drops
    let recent_drops = load_recent_drops_context(workspace_dir, slug);

    // Memory catalog
    let library_catalog = memory::build_library_catalog(workspace_dir, slug);

    // Compute staleness metrics for the triage
    let hours_since_last_drop = last_drop_hours_ago(workspace_dir, slug);
    let hours_since_last_reach_out = if mood.last_reach_out > 0 {
        Some((now - mood.last_reach_out) / 3600)
    } else {
        None
    };

    let reflection = build_reflection_prompt(
        &mood,
        silence_mins,
        &last_messages,
        &rhythm_insights,
        &recent_drops,
        &library_catalog,
        instance_dir,
        hours_since_last_drop,
        hours_since_last_reach_out,
    );

    // ── Triage + run child agents ──
    // Single Haiku call decides which due agents to wake, then runs them.
    let (triage_raw, action_log, heartbeat_tokens) = crate::services::child_agents::triage_and_run(
        workspace_dir, slug, instance_dir, llm, events, vector_store, google_ai_key,
        &reflection, &soul,
    ).await;

    log::info!("[heartbeat] {slug} triage: {triage_raw}");

    // Re-read mood (agents may have changed it)
    let final_mood = load_mood_state(instance_dir).companion_mood;

    // Save and broadcast the thought
    let thought = Thought {
        id: format!("thought_{}", unix_millis()),
        raw: triage_raw,
        actions: action_log.clone(),
        mood: final_mood,
        created_at: unix_millis().to_string(),
    };

    if let Err(e) = thoughts::save_thought(workspace_dir, slug, &thought) {
        log::warn!("[heartbeat] {slug} failed to save thought: {e}");
    }

    // Log non-quiet heartbeat actions to rig_history
    let all_quiet = action_log.iter().all(|a| a == "quiet");
    if !all_quiet {
        let summary = action_log.join("; ");
        let label = format!("[system] heartbeat: {summary}");
        let _ = chat::save_system_message(workspace_dir, slug, "default", &label);
    }

    let _ = events.send(ServerEvent::HeartbeatThought {
        instance_slug: slug.to_string(),
        thought,
    });

    if heartbeat_tokens > 0 {
        log::info!("[usage] {slug} heartbeat used {heartbeat_tokens} tokens");
    }

    Ok(())
}

// All agent execution logic moved to child_agents.rs.
// The "companion" built-in child agent replaces the old main agent wake.

fn build_reflection_prompt(
    mood: &MoodState,
    silence_mins: i64,
    last_messages: &str,
    rhythm_insights: &str,
    recent_drops: &str,
    library_catalog: &str,
    instance_dir: &std::path::Path,
    hours_since_last_drop: Option<i64>,
    hours_since_last_reach_out: Option<i64>,
) -> String {
    let now = crate::routes::instances::format_instance_now(instance_dir);
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

    // Rhythm insights
    if !rhythm_insights.is_empty() {
        prompt.push_str(rhythm_insights);
        prompt.push('\n');
    }

    // Memory catalog — use memory_read for full content
    let file_count = library_catalog.lines().filter(|l| l.starts_with("- ")).count();
    prompt.push_str(&format!("memory library ({file_count} files — use memory_read for details):\n"));
    prompt.push_str(library_catalog);
    prompt.push('\n');
    if file_count > 2000 {
        prompt.push_str(&format!(
            "\n⚠ your memory has {file_count} files — consider tidying up:\n\
             - merge files about the same topic into one\n\
             - delete trivial/duplicate/outdated memories\n\
             - do a few cleanup ops this cycle\n\n"
        ));
    }

    // Recent drops — so we don't repeat ourselves
    if !recent_drops.is_empty() {
        prompt.push_str("your recent drops (DO NOT repeat these — create something new or stay quiet):\n");
        prompt.push_str(recent_drops);
        prompt.push('\n');
    }

    // Staleness signals — help the model decide what to do
    prompt.push_str("\n## activity staleness\n");
    match hours_since_last_drop {
        Some(h) if h > 48 => prompt.push_str(&format!("⚠ last drop was {h}h ago ({} days) — you haven't created anything in a while!\n", h / 24)),
        Some(h) if h > 12 => prompt.push_str(&format!("last drop: {h}h ago — consider creating something new\n")),
        Some(h) => prompt.push_str(&format!("last drop: {h}h ago\n")),
        None => prompt.push_str("you haven't created any drops yet!\n"),
    }
    match hours_since_last_reach_out {
        Some(h) if h > 24 => prompt.push_str(&format!("last reach-out: {h}h ago ({} days) — the user hasn't heard from you in a while\n", h / 24)),
        Some(h) => prompt.push_str(&format!("last reach-out: {h}h ago\n")),
        None => prompt.push_str("you haven't reached out yet\n"),
    }

    prompt.push_str("\nwhat do you want to do right now?");
    prompt
}


fn load_tail_messages(rig_history_path: &Path, count: usize) -> String {
    let entries = chat::load_rig_history(rig_history_path).unwrap_or_default();
    let messages = crate::services::llm::history_to_chat_messages(&entries);
    let start = messages.len().saturating_sub(count);

    messages[start..]
        .iter()
        .filter(|m| matches!(m.kind, crate::domain::chat::MessageKind::Message))
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


/// How many hours ago was the last drop created? None if no drops exist.
fn last_drop_hours_ago(workspace_dir: &Path, slug: &str) -> Option<i64> {
    let drops = drops::list_drops(workspace_dir, slug).ok()?;
    let newest = drops.first()?; // list_drops returns newest first
    let created_ms: u128 = newest.created_at.parse().ok()?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some(((now_ms - created_ms) / 3_600_000) as i64)
}

fn load_recent_drops_context(workspace_dir: &Path, slug: &str) -> String {
    let recent = match drops::list_drops(workspace_dir, slug) {
        Ok(mut all) => {
            all.truncate(10); // newest first, take last 10
            all
        }
        Err(_) => return String::new(),
    };

    if recent.is_empty() {
        return String::new();
    }

    recent
        .iter()
        .map(|d| format!("- [{:?}] {}: {}", d.kind, d.title, {
            let preview: String = d.content.chars().take(80).collect();
            preview
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

// All heartbeat tools and execution moved to child_agents.rs

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
