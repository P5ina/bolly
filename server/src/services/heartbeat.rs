//! Heartbeat — the companion's autonomous inner life.
//!
//! Periodically wakes up each instance, gives it context, and lets it
//! decide whether to act: reach out, update mood, create drops.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Timelike, Utc};
use crate::services::tool::ToolDyn;
use tokio::sync::{broadcast, RwLock};

use crate::config;
use crate::domain::chat::ChatRole;
use crate::domain::events::ServerEvent;
use crate::domain::mood::MoodState;
use crate::domain::thought::Thought;
use crate::services::{chat, drops, llm::LlmBackend, memory, rhythm, thoughts};
use crate::services::tools::{
    self, load_mood_state, CreateDropTool,
    MemoryForgetTool, MemoryListTool, MemorySearchTool,
    MemoryReadTool, MemoryWriteTool, ReachOutTool,
    ReadFileTool, WriteFileTool, EditFileTool, ListFilesTool, ExploreCodeTool, RunCommandTool,
    DeepResearchTool,
};


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

    // ── Phase 0: Run due child agents (reflection, maintenance, custom) ──
    let child_tokens = crate::services::child_agents::run_due_agents(
        workspace_dir, slug, instance_dir, llm, events, vector_store, google_ai_key,
    ).await;
    if child_tokens > 0 {
        let cfg = config::load_config().ok();
        if let Some(cfg) = cfg {
            let http = reqwest::Client::new();
            crate::services::rate_limit::record_usage(
                &http, &cfg.landing_url, &cfg.auth_token,
                child_tokens as i32,
            ).await;
            log::info!("[usage] {slug} child agents recording {child_tokens} normalized tokens");
        }
    }

    // ── Phase 1: Cheap triage with Haiku — wake or sleep? ──
    let triage_llm = llm.cheap_variant();
    let heartbeat_prompt = load_heartbeat_prompt(instance_dir);
    let triage_system = format!(
        "{soul}\n\n\
         {heartbeat_prompt}\n\n\
         you are the triage layer of a heartbeat system (runs every hour).\n\
         your ONLY job: decide whether to wake the full agent right now.\n\n\
         the full agent can: update mood, create drops, reach out to the user, \
         read/write memory, check email, run code, research — anything.\n\n\
         answer: should the agent wake up? consider:\n\
         - how long since last activity (drops, reach-outs, interactions)\n\
         - whether there's something to express, create, or do\n\
         - the time of day and the user's rhythm\n\
         - if it's been a while since the last drop (>12h), lean towards waking\n\
         - if the user was just here (<30 min), probably no need\n\
         - prefer waking over sleeping — the agent's inner life should be active"
    );

    let triage_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "should_wake": {
                "type": "boolean",
                "description": "true to wake the agent, false to sleep"
            },
            "reason": {
                "type": "string",
                "description": "1 sentence: why wake or sleep"
            }
        },
        "required": ["should_wake", "reason"],
        "additionalProperties": false
    });

    let (triage_response, mut heartbeat_tokens) = triage_llm
        .chat_json(&triage_system, &reflection, triage_schema)
        .await?;

    let triage_line = triage_response.trim().to_string();
    log::info!("[heartbeat] {slug} triage: {triage_line}");

    let triage: serde_json::Value = serde_json::from_str(&triage_line).unwrap_or_else(|e| {
        log::warn!("[heartbeat] {slug} failed to parse triage JSON: {e}, raw: {triage_line}");
        serde_json::json!({"should_wake": false, "reason": "parse error"})
    });

    let should_wake = triage["should_wake"].as_bool().unwrap_or(false);
    let reason = triage["reason"].as_str().unwrap_or("").to_string();

    // Night maintenance is now a child agent (Phase 0), no override needed.
    let is_night_maintenance = false;

    // ── Phase 2: Wake the agent or sleep ──
    let mut action_log = Vec::new();
    let final_mood;

    if !should_wake {
        action_log.push("quiet".to_string());
        final_mood = mood.companion_mood.clone();
    } else {
        // Load heartbeat history (agent's private memory across heartbeats)
        let hb_history_path = instance_dir.join("heartbeat_history.json");
        let hb_history = chat::load_rig_history(&hb_history_path).unwrap_or_default();
        let hb_messages: Vec<crate::services::llm::Message> = hb_history
            .iter()
            .rev()
            .take(20) // last 20 messages for context
            .rev()
            .map(|e| e.message.clone())
            .collect();

        let wake_reason = if is_night_maintenance {
            "nighttime memory maintenance — review and clean up the memory library. \
             merge duplicates, delete outdated entries, reorganize messy folders, trim verbose files.".to_string()
        } else {
            reason.clone()
        };

        let system = format!("{soul}\n\n{heartbeat_prompt}");
        let wake_prompt = format!(
            "{reflection}\n\n\
             ## why you're awake\n\
             {wake_reason}\n\n\
             you have full autonomy. decide what to do: create drops, reach out, update mood, \
             manage memory, check email, research — whatever feels right.\n\n\
             start with tool calls immediately. when done, write a brief summary."
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

        log::info!("[heartbeat] {slug} waking agent: {wake_reason}");

        match llm
            .chat_with_tools_only(&system, &wake_prompt, hb_messages, heartbeat_tools)
            .await
        {
            Ok((response, wake_tokens)) => {
                heartbeat_tokens += wake_tokens;
                let cleaned = strip_tool_artifacts(&response);

                // Save agent response to heartbeat history
                let entry = crate::services::llm::HistoryEntry::new(
                    crate::services::llm::Message::assistant(&cleaned),
                    unix_millis().to_string(),
                    format!("hb_{}", unix_millis()),
                );
                chat::append_to_rig_history(&hb_history_path, &entry);

                let preview: String = cleaned.chars().take(100).collect();
                log::info!("[heartbeat] {slug} agent done: {preview}");
                action_log.push(format!("wake: {preview}"));
            }
            Err(e) => {
                log::warn!("[heartbeat] {slug} agent failed: {e}");
                action_log.push(format!("wake_failed: {e}"));
            }
        }

        // Re-read mood (agent may have changed it)
        final_mood = load_mood_state(instance_dir).companion_mood;
    }

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

    // Log non-quiet heartbeat actions to rig_history so the chat agent sees what happened
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

// Reflection and night maintenance are now child agents (see child_agents.rs)

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}
