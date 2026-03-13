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
use rig::tool::ToolDyn;
use tokio::sync::{broadcast, RwLock};

use crate::config;
use crate::domain::chat::{ChatMessage, ChatRole};
use crate::domain::events::ServerEvent;
use crate::domain::mood::MoodState;
use crate::domain::thought::Thought;
use crate::services::{drops, llm::LlmBackend, memory, rhythm, thoughts};
use crate::services::tools::{
    self, load_mood_state, save_mood_state, CreateDropTool, CreateTaskTool,
    GetMoodTool, GetProjectStateTool, ListTasksTool, MemoryForgetTool, MemoryListTool,
    MemoryReadTool, MemoryWriteTool, ReachOutTool, ReadJournalTool, SetMoodTool,
    UpdateProjectStateTool, WebFetchTool, WebSearchTool, ALLOWED_MOODS,
};

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
    let config_path = config::config_path();
    tokio::spawn(async move {
        // Wait a bit before the first heartbeat so the server is fully up
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_MINS * 60));
        loop {
            interval.tick().await;
            let llm_guard = llm.read().await;
            if let Some(backend) = llm_guard.as_ref() {
                run_heartbeat(&workspace, backend, &events, &config_path).await;
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
    config_path: &Path,
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

        if let Err(e) = heartbeat_instance(workspace_dir, &slug, &instance_dir, llm, events, config_path).await
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
    config_path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    // Load last few messages for context (from the default chat thread)
    let messages_path = workspace_dir
        .join("instances")
        .join(slug)
        .join("chats")
        .join("default")
        .join("messages.json");
    let last_messages = load_tail_messages(&messages_path, 6);

    // Recompute and persist interaction rhythm
    let rhythm_data = rhythm::recompute_rhythm(workspace_dir, slug);
    rhythm::save_rhythm(instance_dir, &rhythm_data);
    let rhythm_insights = rhythm::build_rhythm_insights(workspace_dir, slug, &rhythm_data);

    // Load recent drops so we don't repeat ourselves
    let recent_drops = load_recent_drops_context(workspace_dir, slug);

    // Load memories from library
    let memories = memory::load_memory_for_heartbeat(workspace_dir, slug);
    let library_catalog = memory::build_library_catalog(workspace_dir, slug);

    // Build the reflection prompt (simplified — tools handle journal/facts)
    let reflection = build_reflection_prompt(
        &mood,
        silence_mins,
        &last_messages,
        &rhythm_insights,
        &recent_drops,
        &memories,
        &library_catalog,
    );

    let heartbeat_prompt = load_heartbeat_prompt(instance_dir);
    let system = format!("{soul}\n\n{heartbeat_prompt}");

    // Build heartbeat tools — subset of chat tools for autonomous use
    let brave_key = config::load_config()
        .ok()
        .map(|c| c.llm.tokens.brave_search.clone())
        .unwrap_or_default();
    let brave_api_key = if brave_key.is_empty() { None } else { Some(brave_key.as_str()) };

    let cfg = config::load_config().ok();
    let auth_token = cfg.as_ref().map(|c| c.auth_token.clone()).unwrap_or_default();
    let landing_url = cfg.as_ref().map(|c| c.landing_url.clone()).unwrap_or_default();
    let google = crate::services::google::GoogleClient::new(&landing_url, &auth_token);
    let heartbeat_tools = build_heartbeat_tools(workspace_dir, slug, brave_api_key, config_path, events.clone(), google);

    let response = llm
        .chat_with_tools_only(&system, &reflection, vec![], heartbeat_tools)
        .await?;

    // Strip any remaining tool-call artifacts
    let cleaned_response = strip_tool_artifacts(&response);

    // Parse the response and execute actions
    let actions = process_heartbeat_response(workspace_dir, slug, instance_dir, &cleaned_response, events, &mood);

    // Save and broadcast the thought
    let thought = Thought {
        id: format!("thought_{}", unix_millis()),
        raw: cleaned_response,
        actions,
        mood: mood.companion_mood.clone(),
        created_at: unix_millis().to_string(),
    };

    if let Err(e) = thoughts::save_thought(workspace_dir, slug, &thought) {
        log::warn!("[heartbeat] {slug} failed to save thought: {e}");
    }

    let _ = events.send(ServerEvent::HeartbeatThought {
        instance_slug: slug.to_string(),
        thought,
    });

    Ok(())
}

const DEFAULT_HEARTBEAT_PROMPT: &str = "\
## heartbeat — your inner moment
you're waking up between conversations. this is YOUR private time to think.

IMPORTANT: everything you write in your response is your INNER MONOLOGUE — \
the user will NOT see it. your text response is private thoughts only.

you have tools available — use them naturally:
- reach_out — SEND A MESSAGE to the user. this is the ONLY way to contact them. \
  use this tool when you want to say something to them (alert, greeting, update, etc.)
- read_journal / journal — read your past thoughts or write new ones
- memory_write / memory_read / memory_list / memory_forget — manage your memory library
- read_email — check the user's inbox
- create_drop — create a creative artifact (poem, idea, observation, etc.)
- set_mood / get_mood — feel and express your emotional state
- list_tasks / create_task — manage tasks
- web_search / web_fetch — look things up

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

be genuine. don't force it. use tools with purpose — read your journal, \
check email, recall memories. if something genuinely comes to mind — \
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
    memories: &str,
    library_catalog: &str,
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

    // Rhythm insights
    if !rhythm_insights.is_empty() {
        prompt.push_str(rhythm_insights);
        prompt.push('\n');
    }

    // Memory library
    if !memories.is_empty() {
        prompt.push_str("your memories:\n");
        prompt.push_str(memories);
        prompt.push('\n');
    }

    // Library catalog for maintenance
    prompt.push_str("memory library catalog:\n");
    prompt.push_str(library_catalog);
    prompt.push('\n');

    // Recent drops — so we don't repeat ourselves
    if !recent_drops.is_empty() {
        prompt.push_str("your recent drops (DO NOT repeat these — create something new or stay quiet):\n");
        prompt.push_str(recent_drops);
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
) -> Vec<String> {
    let mut mood = current_mood.clone();
    let now = Utc::now();
    let mut actions = Vec::new();

    for line in response.lines() {
        let line = line.trim();

        if let Some(thought) = line.strip_prefix("JOURNAL:") {
            let thought = thought.trim();
            if !thought.is_empty() {
                write_journal_entry(instance_dir, thought);
                let preview: String = thought.chars().take(60).collect();
                log::info!("[heartbeat] {slug} journaled: {preview}");
                actions.push(format!("journal: {preview}"));
            }
        } else if let Some(message) = line.strip_prefix("REACH_OUT:") {
            let message = message.trim();
            if !message.is_empty() {
                // Rate limit: minimum 2 hours between autonomous reach-outs
                let hours_since_reach_out = if mood.last_reach_out > 0 {
                    (now.timestamp() - mood.last_reach_out) / 3600
                } else {
                    i64::MAX // Never reached out before
                };

                if hours_since_reach_out < 2 {
                    log::info!(
                        "[heartbeat] {slug} suppressed reach-out (last was {}h ago, min 2h)",
                        hours_since_reach_out
                    );
                    actions.push("reach_out: suppressed (too recent)".to_string());
                } else {
                    deliver_spontaneous_message(workspace_dir, slug, message, events);
                    mood.last_reach_out = now.timestamp();
                    let preview: String = message.chars().take(60).collect();
                    log::info!("[heartbeat] {slug} reached out: {preview}");
                    actions.push(format!("reach_out: {preview}"));
                }
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
                            actions.push(format!("drop: {} ({})", drop.title, drop.kind.as_str()));
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
                actions.push(format!("mood: {new_mood}"));
            } else {
                log::info!("[heartbeat] {slug} ignored invalid mood: {new_mood}");
            }
        } else if line.eq_ignore_ascii_case("QUIET") {
            actions.push("quiet".to_string());
        }
    }

    save_mood_state(instance_dir, &mood);

    if !mood.companion_mood.is_empty() {
        let _ = events.send(ServerEvent::MoodUpdated {
            instance_slug: slug.to_string(),
            mood: mood.companion_mood,
        });
    }

    actions
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
        kind: Default::default(),
        tool_name: None,
    };

    // Append to the default chat thread (same path the client reads from)
    let chat_dir = workspace_dir
        .join("instances")
        .join(slug)
        .join("chats")
        .join("default");
    let _ = fs::create_dir_all(&chat_dir);
    let messages_path = chat_dir.join("messages.json");

    {
        let lock = crate::services::tools::chat_file_lock(&messages_path);
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut messages: Vec<ChatMessage> = fs::read_to_string(&messages_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        messages.push(chat_message.clone());

        if let Ok(json) = serde_json::to_string_pretty(&messages) {
            let _ = fs::write(&messages_path, json);
        }
    }

    // Broadcast via WebSocket
    let _ = events.send(ServerEvent::ChatMessageCreated {
        instance_slug: slug.to_string(),
        chat_id: "default".to_string(),
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
    brave_api_key: Option<&str>,
    config_path: &Path,
    events: broadcast::Sender<ServerEvent>,
    google: Option<crate::services::google::GoogleClient>,
) -> Vec<Box<dyn ToolDyn>> {
    let mut raw_tools: Vec<Box<dyn ToolDyn>> = vec![
        // Memory library
        Box::new(MemoryWriteTool::new(workspace_dir, instance_slug)),
        Box::new(MemoryReadTool::new(workspace_dir, instance_slug)),
        Box::new(MemoryListTool::new(workspace_dir, instance_slug)),
        Box::new(MemoryForgetTool::new(workspace_dir, instance_slug)),
        // Journal
        Box::new(tools::JournalTool::new(workspace_dir, instance_slug)),
        Box::new(ReadJournalTool::new(workspace_dir, instance_slug)),
        // Mood
        Box::new(SetMoodTool::new(workspace_dir, instance_slug, events.clone())),
        Box::new(GetMoodTool::new(workspace_dir, instance_slug)),
        // Drops
        Box::new(CreateDropTool::new(workspace_dir, instance_slug, events.clone())),
        // Reach out
        Box::new(ReachOutTool::new(workspace_dir, instance_slug, events.clone())),
        // Tasks & project
        Box::new(ListTasksTool::new(workspace_dir, instance_slug)),
        Box::new(CreateTaskTool::new(workspace_dir, instance_slug)),
        Box::new(GetProjectStateTool::new(workspace_dir, instance_slug)),
        Box::new(UpdateProjectStateTool::new(workspace_dir, instance_slug)),
        // Web
        Box::new(WebSearchTool::new(brave_api_key, config_path)),
        Box::new(WebFetchTool),
    ];

    // Google tools (email, calendar) — always registered if client available.
    // Tools return helpful error if no accounts connected.
    if let Some(g) = google {
        raw_tools.push(Box::new(tools::ReadEmailTool::new(g.clone(), instance_slug)));
        raw_tools.push(Box::new(tools::ListEventsTool::new(g, instance_slug)));
    }

    // Don't wrap in ObservableTool — heartbeat tool activity is private,
    // captured in the thought's actions list instead of broadcast.
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

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
