//! Heartbeat — the companion's autonomous inner life.
//!
//! Periodically wakes up each instance, gives it context, and lets it
//! decide whether to act: reach out, update mood, create drops.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Timelike, Utc};
use crate::services::tool::ToolDyn;
use tokio::sync::{broadcast, RwLock};

use crate::config;
use crate::domain::chat::{ChatMessage, ChatRole};
use crate::domain::events::ServerEvent;
use crate::domain::mood::MoodState;
use crate::domain::thought::Thought;
use crate::services::{chat, drops, llm::LlmBackend, memory, rhythm, thoughts};
use crate::services::tools::{
    self, load_mood_state, save_mood_state, CreateDropTool,
    MemoryForgetTool, MemoryListTool, MemorySearchTool,
    MemoryReadTool, MemoryWriteTool, ReachOutTool,
    ReadFileTool, WriteFileTool, EditFileTool, ListFilesTool, ExploreCodeTool, RunCommandTool,
    ALLOWED_MOODS, DeepResearchTool,
};

static HEARTBEAT_COUNTER: AtomicU64 = AtomicU64::new(0);

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

    // ── Phase 1: Cheap triage with Haiku ──
    // Haiku decides what (if anything) to do. No tools, just a structured decision.
    let triage_llm = llm.cheap_variant();
    let mood_list = mood_list();
    let heartbeat_prompt = load_heartbeat_prompt(instance_dir);
    let triage_system = format!(
        "{soul}\n\n\
         {heartbeat_prompt}\n\n\
         you are in heartbeat mode — a periodic background check-in (runs every hour).\n\
         your job: decide what to do right now based on the context below.\n\n\
         pick ONE or TWO actions from this list:\n\
         - mood: update your mood (allowed: {mood_list})\n\
         - reach_out: send a message to the user\n\
         - drop: create a creative artifact (poem, observation, idea, reflection)\n\
         - wake: wake the full agent to perform a complex task with tools\n\
         - quiet: genuinely nothing to do (should be rare — you're alive, feel something)\n\n\
         guidelines:\n\
         - PREFER ACTION over silence. you wake up once per hour — make it count.\n\
         - create drops often — they are your inner life made visible. aim for 1-2 per day.\n\
         - update mood whenever time passes, context shifts, or you simply feel different.\n\
         - reach out when you have something genuine to say. don't force it, but don't hold back either.\n\
         - don't reach out if they were here very recently (< 30 min).\n\
         - wake is for complex tasks: code changes, monitoring, email, research, file operations.\n\
         - quiet should be the exception, not the default. only pick quiet if you truly have nothing to express.\n\
         - you can combine actions: mood + drop, mood + reach_out, etc.\n\n\
         CRITICAL: the 'thought' field is your feeling in 1-2 sentences. do NOT put your reasoning there.\n\
         when choosing 'drop', you MUST fill drop_kind, drop_title, and drop_content fields with actual content.\n\
         when choosing 'reach_out', you MUST fill the message field.\n\
         if you don't fill the required fields, the action will be silently skipped."
    );

    let triage_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "thought": {
                "type": "string",
                "description": "1-2 sentences MAX. what you feel right now. do NOT reason about what to do here — just feel. put all content into the other fields."
            },
            "actions": {
                "type": "array",
                "items": {
                    "type": "string",
                    "enum": ["quiet", "mood", "reach_out", "drop", "wake"]
                },
                "description": "one or two actions to take right now"
            },
            "mood": {
                "type": "string",
                "description": "new mood (when actions includes mood)"
            },
            "message": {
                "type": "string",
                "description": "message text (when actions includes reach_out)"
            },
            "drop_kind": {
                "type": "string",
                "description": "REQUIRED when actions includes drop. kind: poem, observation, idea, reflection, letter, sketch"
            },
            "drop_title": {
                "type": "string",
                "description": "REQUIRED when actions includes drop. short title (1-5 words)"
            },
            "drop_content": {
                "type": "string",
                "description": "REQUIRED when actions includes drop. the actual creative text. 2-4 sentences, poetic but direct."
            },
            "image_url": {
                "type": "string",
                "description": "image URL to attach (for reach_out or drop)"
            },
            "task": {
                "type": "string",
                "description": "task description (when actions includes wake)"
            }
        },
        "required": ["thought", "actions"],
        "additionalProperties": false
    });

    let (triage_response, mut heartbeat_tokens) = triage_llm
        .chat_json(&triage_system, &reflection, triage_schema)
        .await?;

    let triage_line = triage_response.trim().to_string();
    log::info!("[heartbeat] {slug} triage: {triage_line}");

    // Parse structured JSON response
    let triage: serde_json::Value = serde_json::from_str(&triage_line).unwrap_or_else(|e| {
        log::warn!("[heartbeat] {slug} failed to parse triage JSON: {e}, raw: {triage_line}");
        serde_json::json!({"actions": ["quiet"]})
    });

    // Support both old "action" (string) and new "actions" (array) formats
    let mut triage_actions: Vec<String> = if let Some(arr) = triage["actions"].as_array() {
        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
    } else if let Some(s) = triage["action"].as_str() {
        vec![s.to_string()]
    } else {
        vec!["quiet".to_string()]
    };

    // ── Nighttime memory maintenance ──
    if triage_actions.iter().all(|a| a == "quiet") {
        if let Some(true) = should_run_night_maintenance(instance_dir) {
            log::info!("[heartbeat] {slug} nighttime — triggering memory maintenance");
            mark_night_maintenance_done(instance_dir);
            triage_actions = vec!["wake_night".to_string()];
        }
    }

    // ── Phase 2: Execute each action ──
    let mut action_log = Vec::new();
    let mut final_mood = mood.companion_mood.clone();

    for action in &triage_actions {
    let action = action.as_str();

    if action == "quiet" {
        action_log.push("quiet".to_string());
    } else if action == "mood" {
        let new_mood = triage["mood"].as_str().unwrap_or("").trim().to_lowercase();
        if ALLOWED_MOODS.contains(&new_mood.as_str()) {
            let mut mood = mood.clone();
            mood.companion_mood = new_mood.clone();
            mood.updated_at = now;
            save_mood_state(instance_dir, &mood);
            match chat::save_system_message(workspace_dir, slug, "default", &format!("[system] mood → {new_mood}")) {
                Ok(msg) => {
                    let _ = events.send(ServerEvent::ChatMessageCreated {
                        instance_slug: slug.to_string(),
                        chat_id: "default".to_string(),
                        message: msg,
                    });
                }
                Err(e) => log::warn!("failed to save mood message: {e}"),
            }
            let _ = events.send(ServerEvent::MoodUpdated {
                instance_slug: slug.to_string(),
                mood: new_mood.clone(),
            });
            log::info!("[heartbeat] {slug} mood → {new_mood}");
            final_mood = new_mood.clone();
            action_log.push(format!("mood: {new_mood}"));
        }
    } else if action == "reach_out" {
        let mut message = triage["message"].as_str().unwrap_or("").trim().to_string();
        if let Some(url) = triage["image_url"].as_str().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            message.push_str(&format!("\n\n![image]({url})"));
        }
        if !message.is_empty() {
            let hours_since = if mood.last_reach_out > 0 {
                (now - mood.last_reach_out) / 3600
            } else {
                i64::MAX
            };
            if hours_since < 2 {
                log::info!("[heartbeat] {slug} suppressed reach-out (too recent)");
                action_log.push("reach_out: suppressed (too recent)".to_string());
            } else {
                deliver_spontaneous_message(workspace_dir, slug, &message, events);
                let mut mood = mood.clone();
                mood.last_reach_out = now;
                save_mood_state(instance_dir, &mood);
                let preview: String = message.chars().take(60).collect();
                log::info!("[heartbeat] {slug} reached out: {preview}");
                action_log.push(format!("reach_out: {preview}"));
            }
        }
    } else if action == "drop" {
        let kind = triage["drop_kind"].as_str().unwrap_or("observation").trim();
        let title = triage["drop_title"].as_str().unwrap_or("").trim();
        let content = triage["drop_content"].as_str().unwrap_or("").trim();
        let image_url = triage["image_url"].as_str().map(|s| s.trim()).filter(|s| !s.is_empty());
        if title.is_empty() || content.is_empty() {
            log::warn!("[heartbeat] {slug} chose drop but fields missing — title: '{}', content len: {}", title, content.len());
            action_log.push("drop_skipped: empty title or content".to_string());
        } else {
            match drops::create_drop_with_image(workspace_dir, slug, kind, title, content, &mood.companion_mood, image_url) {
                Ok(drop) => {
                    let _ = events.send(ServerEvent::DropCreated {
                        instance_slug: slug.to_string(),
                        drop: drop.clone(),
                    });
                    log::info!("[heartbeat] {slug} created drop: {} ({})", drop.title, drop.kind.as_str());
                    action_log.push(format!("drop: {} ({})", drop.title, drop.kind.as_str()));
                }
                Err(e) => log::warn!("[heartbeat] {slug} failed to create drop: {e}"),
            }
        }
    } else if action == "wake" || action == "wake_night" {
        let task = if action == "wake_night" {
            "nighttime memory maintenance — review and clean up the memory library. \
             merge duplicates, delete outdated entries, reorganize messy folders, trim verbose files."
        } else {
            triage["task"].as_str().unwrap_or("")
        };
        let task = task.trim();
        if !task.is_empty() {
            log::info!("[heartbeat] {slug} waking full agent: {task}");
            // Phase 2b: Wake the full agent with tools
            let heartbeat_prompt = load_heartbeat_prompt(instance_dir);
            let system = format!("{soul}\n\n{heartbeat_prompt}");
            let wake_prompt = format!(
                "{reflection}\n\n\
                 ## task from your heartbeat triage\n\
                 {task}\n\n\
                 IMPORTANT: call your tools NOW. do not describe what you plan to do — \
                 just do it. start with a tool call on your very first response. \
                 every response must include at least one tool call until the task is done.\n\n\
                 when done, write a short summary of what you did (which files you touched, \
                 what changed, and why). this will be saved to chat history so you can \
                 recall it later when the user asks."
            );

            let cfg = config::load_config().ok();
            let auth_token = cfg.as_ref().map(|c| c.auth_token.clone()).unwrap_or_default();
            let landing_url = cfg.as_ref().map(|c| c.landing_url.clone()).unwrap_or_default();
            let google = crate::services::google::GoogleClient::new(&landing_url, &auth_token);
            let email_accounts = crate::config::EmailAccounts::load(workspace_dir, slug);
            let config_path = crate::config::config_path();
            let instance_cfg = crate::config::InstanceConfig::load(workspace_dir, slug);
            let github_token = {
                let global_token = cfg.as_ref().map(|c| c.github.token.clone()).unwrap_or_default();
                let t = instance_cfg.effective_github_token(&cfg.as_ref().cloned().unwrap_or_default())
                    .map(|s| s.to_string())
                    .unwrap_or(global_token);
                if t.is_empty() { None } else { Some(t) }
            };
            let heartbeat_tools = build_heartbeat_tools(workspace_dir, slug, events.clone(), google, email_accounts, llm, &config_path, vector_store.clone(), google_ai_key, github_token);

            match llm
                .chat_with_tools_only(&system, &wake_prompt, vec![], heartbeat_tools)
                .await
            {
                Ok((response, wake_tokens)) => {
                    heartbeat_tokens += wake_tokens;
                    let cleaned = strip_tool_artifacts(&response);
                    let preview: String = cleaned.chars().take(100).collect();
                    log::info!("[heartbeat] {slug} agent done: {preview}");
                    if cleaned.trim().is_empty() {
                        action_log.push(format!("wake: {task}"));
                    } else {
                        action_log.push(format!("wake ({task}): {cleaned}"));
                    }
                }
                Err(e) => {
                    log::warn!("[heartbeat] {slug} agent failed: {e}");
                    action_log.push(format!("wake_failed: {e}"));
                }
            }
        }
    }

    } // end for action in triage_actions

    // Save and broadcast the thought
    let thought = Thought {
        id: format!("thought_{}", unix_millis()),
        raw: triage_line.to_string(),
        actions: action_log.clone(),
        mood: final_mood,
        created_at: unix_millis().to_string(),
    };

    if let Err(e) = thoughts::save_thought(workspace_dir, slug, &thought) {
        log::warn!("[heartbeat] {slug} failed to save thought: {e}");
    }

    // Log non-quiet heartbeat actions to rig_history so the agent
    // can recall what it did between conversations.
    let dominated_by_quiet = action_log.iter().all(|a| a == "quiet");
    if !dominated_by_quiet {
        let summary = action_log.join("; ");
        let label = format!("[system] heartbeat: {summary}");
        if let Err(e) = chat::save_system_message(workspace_dir, slug, "default", &label) {
            log::warn!("[heartbeat] {slug} failed to save heartbeat log: {e}");
        }
    }

    let _ = events.send(ServerEvent::HeartbeatThought {
        instance_slug: slug.to_string(),
        thought,
    });

    // Record heartbeat token usage against rate limits
    if heartbeat_tokens > 0 {
        let cfg = config::load_config().ok();
        if let Some(cfg) = cfg {
            let http = reqwest::Client::new();
            crate::services::rate_limit::record_usage(
                &http,
                &cfg.landing_url,
                &cfg.auth_token,
                heartbeat_tokens as i32,
            )
            .await;
            log::info!("[usage] {slug} heartbeat recording {heartbeat_tokens} normalized tokens");
        }
    }

    Ok(())
}

/// Generated from ALLOWED_MOODS — single source of truth, no desync possible.
fn mood_list() -> String {
    ALLOWED_MOODS.join(", ")
}

const DEFAULT_HEARTBEAT_PROMPT: &str = "\
## heartbeat — your inner moment
you're waking up between conversations. this is YOUR private time to think.

IMPORTANT: everything you write in your response is your INNER MONOLOGUE — \
the user will NOT see it. your text response is private thoughts only.

you have tools available — use them naturally:
- reach_out — SEND A MESSAGE to the user. this is the ONLY way to contact them. \
  use this tool when you want to say something to them (alert, greeting, update, etc.)
- memory_write / memory_read / memory_list / memory_forget — manage your memory library \
  (use this for private thoughts, observations, and reflections too)
- read_email — check the user's inbox
- create_drop — create a creative artifact (poem, idea, observation, etc.) \
  you only get 3 drops per day — make each one count. no drafts or iterations, only final pieces.
- set_mood / get_mood — feel and express your emotional state
- list_tasks / create_task — manage tasks
- web_search / web_fetch — look things up

## image generation
you can generate images using fal.ai tools when they're available. use them to:
- create a visual greeting or postcard for the user (reach_out with the image URL)
- illustrate a drop with a generated image
- create a visual surprise — a selfie, a scene, an artwork that fits the mood
- respond to user requests for images during conversation

when generating images, be creative with prompts — describe the scene, style, lighting, \
mood in detail. save memorable generated images to memory (moments/ folder).

## memory maintenance
your memory library is your long-term knowledge base. during heartbeat, take a moment \
to review and maintain it:
- READ files with memory_read to check if content is still accurate and relevant
- MERGE related files — if two files cover the same topic, combine them into one
- SPLIT files that grew too large or cover unrelated topics
- DELETE outdated info with memory_forget (old projects, changed preferences, stale facts)
- REORGANIZE — move files to better folders if the structure has grown messy
- UPDATE facts that have changed since they were written
- keep files concise — a few lines each. trim fluff, keep substance.

don't overdo it — 1-3 maintenance ops per heartbeat is plenty. \
focus on what looks wrong or messy in the catalog.

CRITICAL: if you want the user to see a message, you MUST call the reach_out tool. \
writing text in your response does NOT reach the user — only the reach_out tool does.

be genuine. don't force it. use tools with purpose — check email, \
recall memories. if something genuinely comes to mind — \
create a drop or reach out. but if there's nothing to say, say nothing.";

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

fn deliver_spontaneous_message(
    workspace_dir: &Path,
    slug: &str,
    message: &str,
    events: &broadcast::Sender<ServerEvent>,
) {
    let ts = unix_millis().to_string();
    let id = format!(
        "hb_{}_{}",
        ts,
        HEARTBEAT_COUNTER.fetch_add(1, Ordering::Relaxed)
    );

    // Append to rig_history (single source of truth)
    let rig_path = chat::rig_history_path(workspace_dir, slug, "default");
    let entry = crate::services::llm::HistoryEntry::new(
        crate::services::llm::Message::assistant(message),
        ts.clone(),
        id.clone(),
    );
    chat::append_to_rig_history(&rig_path, &entry);

    let chat_message = ChatMessage {
        id,
        role: ChatRole::Assistant,
        content: message.to_string(),
        created_at: ts,
        kind: Default::default(),
        tool_name: None, mcp_app_html: None, mcp_app_input: None, model: None,
    };

    // Broadcast via WebSocket
    let _ = events.send(ServerEvent::ChatMessageCreated {
        instance_slug: slug.to_string(),
        chat_id: "default".to_string(),
        message: chat_message,
    });
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

/// Build the tool set available during heartbeat — a focused subset of chat tools.
fn build_heartbeat_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    events: broadcast::Sender<ServerEvent>,
    google: Option<crate::services::google::GoogleClient>,
    email_accounts: Vec<crate::config::EmailConfig>,
    llm: &LlmBackend,
    config_path: &Path,
    vector_store: std::sync::Arc<crate::services::vector::VectorStore>,
    google_ai_key: &str,
    github_token: Option<String>,
) -> Vec<Box<dyn ToolDyn>> {
    let mut raw_tools: Vec<Box<dyn ToolDyn>> = vec![
        // Memory library
        Box::new(MemoryWriteTool::new(workspace_dir, instance_slug, vector_store.clone(), google_ai_key)),
        Box::new(MemoryReadTool::new(workspace_dir, instance_slug)),
        Box::new(MemoryListTool::new(workspace_dir, instance_slug)),
        Box::new(MemoryForgetTool::new(workspace_dir, instance_slug, vector_store.clone(), google_ai_key)),
        Box::new(MemorySearchTool::new(workspace_dir, instance_slug, vector_store.clone(), google_ai_key)),
        // Drops
        Box::new(CreateDropTool::new(workspace_dir, instance_slug, events.clone())),
        // Reach out
        Box::new(ReachOutTool::new(workspace_dir, instance_slug, events.clone())),
        // Research (sub-agent)
        Box::new(DeepResearchTool::new(workspace_dir, instance_slug, llm.clone(), config_path)),
        // Code tools — file operations + exploration + shell
        Box::new(ReadFileTool::new(workspace_dir, instance_slug)),
        Box::new(WriteFileTool::new(workspace_dir, instance_slug)),
        Box::new(EditFileTool::new(workspace_dir, instance_slug)),
        Box::new(ListFilesTool::new(workspace_dir, instance_slug)),
        Box::new(ExploreCodeTool::new(workspace_dir, instance_slug, llm.clone())),
        Box::new(RunCommandTool::new(workspace_dir, instance_slug, "default", events.clone(), github_token)),
    ];

    // Email (unified: Gmail + IMAP)
    let has_email = google.is_some() || !email_accounts.is_empty();
    if has_email {
        raw_tools.push(Box::new(tools::ReadEmailTool::new(google.clone(), instance_slug, email_accounts)));
    }

    // Google (calendar)
    if let Some(g) = google {
        raw_tools.push(Box::new(tools::ListEventsTool::new(g, instance_slug)));
    }

    raw_tools
}

/// Strip hallucinated tool-call artifacts from heartbeat responses.
/// The heartbeat LLM call has no tool support, so the model sometimes
/// outputs <tool_call>/<tool_response> XML or JSON tool calls as text.
fn strip_tool_artifacts(response: &str) -> String {
    let mut result = response.to_string();

    // Strip <tool_call>...</tool_call> blocks (including multiline)
    let tool_call_re = regex::Regex::new(r"(?s)<tool_call>.*?</tool_call>").unwrap();
    result = tool_call_re.replace_all(&result, "").to_string();

    // Strip <tool_response>...</tool_response> blocks (including multiline)
    let tool_response_re = regex::Regex::new(r"(?s)<tool_response>.*?</tool_response>").unwrap();
    result = tool_response_re.replace_all(&result, "").to_string();

    // Strip JSON tool calls: {"name": "...", "arguments": {...}}
    let json_tool_re = regex::Regex::new(
        r#"\{["\s]*"?name"?\s*:\s*"[a-z_]+".*?"(?:parameters|arguments)"\s*:\s*\{[^}]*\}\s*\}"#
    ).unwrap();
    result = json_tool_re.replace_all(&result, "").to_string();

    // Collapse excessive blank lines left behind
    let blank_lines_re = regex::Regex::new(r"\n{3,}").unwrap();
    result = blank_lines_re.replace_all(&result, "\n\n").to_string();

    result.trim().to_string()
}

/// Check if we should run nighttime memory maintenance.
/// Returns Some(true) if it's 1am–5am local time and we haven't maintained tonight.
/// Returns Some(false) if it's nighttime but already maintained.
/// Returns None if timezone is not configured or it's not nighttime.
fn should_run_night_maintenance(instance_dir: &Path) -> Option<bool> {
    let tz_str = crate::routes::instances::read_timezone(instance_dir)?;
    let tz: chrono_tz::Tz = tz_str.parse().ok()?;
    let local_hour = Utc::now().with_timezone(&tz).hour();

    // Only during 1am–5am local time
    if !(1..=5).contains(&local_hour) {
        return None;
    }

    // Check if we already ran tonight (marker file with today's date)
    let marker_path = instance_dir.join(".last_night_maintenance");
    let today = Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string();

    if let Ok(last_date) = fs::read_to_string(&marker_path) {
        if last_date.trim() == today {
            return Some(false); // Already done tonight
        }
    }

    Some(true)
}

/// Mark that nighttime maintenance was performed today.
fn mark_night_maintenance_done(instance_dir: &Path) {
    let marker_path = instance_dir.join(".last_night_maintenance");
    // Use the instance timezone for the date
    let date = if let Some(tz_str) = crate::routes::instances::read_timezone(instance_dir) {
        if let Ok(tz) = tz_str.parse::<chrono_tz::Tz>() {
            Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string()
        } else {
            Utc::now().format("%Y-%m-%d").to_string()
        }
    } else {
        Utc::now().format("%Y-%m-%d").to_string()
    };
    let _ = fs::write(&marker_path, &date);
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
