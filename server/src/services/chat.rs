use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use rig::providers::openai;
use rig::tool::ToolDyn;
use tokio::sync::broadcast;

use crate::{
    domain::chat::{ChatMessage, ChatResponse, ChatRole},
    domain::events::ServerEvent,
    domain::instance::InstanceSummary,
    services::{
        llm::{self, LlmBackend},
        memory,
        rhythm,
        tools::{
            self, ActivateSkillTool, EditFileTool, JournalTool, ListFilesTool, ListSkillsTool, ClearContextTool, ObservableTool,
            ReadFileTool, RecallTool, RememberTool,
            RunCommandTool, SendFileTool,
            SetMoodTool, WriteFileTool,
        },
        skills,
        workspace,
    },
};

static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Save the user message to disk and return it.
pub fn save_user_message(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    content: &str,
) -> io::Result<ChatMessage> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);
    ensure_instance_layout(workspace_dir, &instance_slug)?;
    ensure_chat_dir(workspace_dir, &instance_slug, &chat_id)?;

    let user_message = ChatMessage {
        id: next_id(),
        role: ChatRole::User,
        content: content.to_string(),
        created_at: timestamp(),
        kind: Default::default(),
        tool_name: None,
    };

    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;
    messages.push(user_message.clone());
    save_messages(workspace_dir, &instance_slug, &chat_id, &messages)?;

    // Update last_interaction timestamp
    let instance_dir = workspace_dir.join("instances").join(&instance_slug);
    let mut mood = tools::load_mood_state(&instance_dir);
    mood.last_interaction = chrono::Utc::now().timestamp();
    tools::save_mood_state(&instance_dir, &mood);

    Ok(user_message)
}

/// Save a system/tool message (role=assistant) for status/error notifications.
pub fn save_system_message(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    content: &str,
) -> io::Result<ChatMessage> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);
    ensure_chat_dir(workspace_dir, &instance_slug, &chat_id)?;

    let msg = ChatMessage {
        id: next_id(),
        role: ChatRole::Assistant,
        content: content.to_string(),
        created_at: timestamp(),
        kind: Default::default(),
        tool_name: None,
    };

    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;
    messages.push(msg.clone());
    save_messages(workspace_dir, &instance_slug, &chat_id, &messages)?;

    Ok(msg)
}

/// Run a single LLM turn: build context, call LLM with tools, save response.
/// Returns one or more assistant messages (the reply is split into chat-like chunks).
/// Rig handles up to 8 internal tool sub-turns.
pub struct SingleTurnResult {
    pub messages: Vec<ChatMessage>,
    /// The agent was cut short by the inner turn limit and needs to continue.
    pub hit_turn_limit: bool,
    /// Estimated total tokens (input + output) consumed by this turn.
    pub estimated_tokens: i32,
    /// The full Rig message history (with ToolCall/ToolResult) for carrying
    /// context between outer loop iterations.
    pub rig_history: Option<Vec<rig::completion::Message>>,
}

pub async fn run_single_turn(
    workspace_dir: &Path,
    config_path: &Path,
    instance_slug: &str,
    chat_id: &str,
    llm: &LlmBackend,
    embedding_model: Option<&openai::EmbeddingModel>,
    brave_api_key: Option<&str>,
    events: broadcast::Sender<ServerEvent>,
    tool_index: Option<tools::ToolIndex>,
    prev_history: Option<Vec<rig::completion::Message>>,
    pending_secrets: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, crate::app::state::PendingSecret>>>,
) -> io::Result<SingleTurnResult> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);

    // Build system prompt with all context
    let base_prompt = llm::load_system_prompt(workspace_dir, &instance_slug);
    let existing: Vec<ChatMessage> =
        load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;

    // Find last real user message for memory retrieval
    let last_user_content = existing
        .iter()
        .rev()
        .find(|m| matches!(m.role, ChatRole::User) && !is_tool_activity(m))
        .map(|m| m.content.as_str())
        .unwrap_or("");

    // Build RAG memory index if embedding model is available,
    // otherwise fall back to injecting facts.md into system prompt
    let memory_index = if let Some(model) = embedding_model {
        memory::build_memory_index(workspace_dir, &instance_slug, model).await
    } else {
        None
    };

    let memory_prompt = if memory_index.is_none() {
        // No RAG — inject all facts + episodes into system prompt as fallback
        memory::build_facts_md_prompt(workspace_dir, &instance_slug)
    } else {
        // RAG handles facts via dynamic_context; episodes are always injected directly
        let episodes = memory::build_episodes_prompt(workspace_dir, &instance_slug);
        format!(
            "## memory\nyou have persistent memory. relevant facts are loaded automatically based on the conversation.{episodes}"
        )
    };

    let journal_prompt = load_recent_journal(workspace_dir, &instance_slug);
    let mood_prompt = load_mood_prompt(workspace_dir, &instance_slug);
    let rhythm_prompt = load_rhythm_prompt(workspace_dir, &instance_slug);

    // Build system prompt with STABLE content first (for Anthropic prompt caching).
    // Anthropic caches the longest matching prefix, so put rarely-changing
    // sections at the top and dynamic/per-message sections at the bottom.
    let mut system_prompt = base_prompt;

    // Stable: skills, capabilities, email, style (rarely change)
    let skills_prompt = build_skills_prompt(workspace_dir);
    if !skills_prompt.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{skills_prompt}");
    }

    // Dynamic tool hint — tools are automatically selected via RAG
    system_prompt.push_str(
        "\n\n## tools\nyou have built-in tools for web browsing, email, code search, \
         project management, creative drops, and more. use them directly when needed — \
         they are automatically available based on the conversation."
    );

    let autonomy_prompt = load_autonomy_prompt(workspace_dir, &instance_slug);
    system_prompt = format!("{system_prompt}\n\n{autonomy_prompt}");

    let email_status = load_email_status(workspace_dir, &instance_slug);
    if !email_status.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{email_status}");
    }

    system_prompt.push_str(
        "\n\n## style\n\
         write like texting a friend. short messages split by blank lines. \
         1-2 sentences each. no walls of text, no bullet lists in conversation. \
         lowercase, casual, warm."
    );

    // Semi-stable: memory (changes when facts are added)
    if !memory_prompt.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{memory_prompt}");
    }

    // Dynamic: journal, mood, rhythm (change frequently — placed last)
    if !journal_prompt.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{journal_prompt}");
    }
    if !mood_prompt.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{mood_prompt}");
    }
    if !rhythm_prompt.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{rhythm_prompt}");
    }

    // Build Rig message history: use prev_history if available (continuation),
    // otherwise load from rig_history.json or fall back to converting messages.json.
    let (history_msgs, prompt_msg) = if let Some(history) = prev_history {
        // Continuation turn: check if there are new user messages since the last one
        // in the Rig history, and append them.
        let last_user_in_existing = existing.last();
        if let Some(last_msg) = last_user_in_existing {
            if matches!(last_msg.role, ChatRole::User) {
                let now = chrono::Local::now().format("%A, %B %-d, %Y %H:%M %Z");
                let content_with_time = format!("[{now}]\n{}", last_msg.content);
                let prompt = llm::build_multimodal_prompt(&content_with_time, workspace_dir, &instance_slug);
                (history, prompt)
            } else {
                return Err(io::Error::new(ErrorKind::InvalidInput, "no user message to process"));
            }
        } else {
            return Err(io::Error::new(ErrorKind::InvalidInput, "no messages to process"));
        }
    } else {
        // First turn or restart: load Rig history from disk
        let rig_path = rig_history_path(workspace_dir, &instance_slug, &chat_id);
        let loaded_history = load_rig_history(&rig_path);

        // Filter out tool activity from existing for display-text-only operations
        let display_msgs: Vec<&ChatMessage> = existing.iter()
            .filter(|m| !is_tool_activity(m))
            .collect();

        if display_msgs.is_empty() {
            return Err(io::Error::new(ErrorKind::InvalidInput, "no messages to process"));
        }

        let last_display = display_msgs.last().unwrap();
        let now = chrono::Local::now().format("%A, %B %-d, %Y %H:%M %Z");
        let content_with_time = format!("[{now}]\n{}", last_display.content);
        let prompt = llm::build_multimodal_prompt(&content_with_time, workspace_dir, &instance_slug);

        let history = if let Some(h) = loaded_history {
            log::info!("loaded {} rig history messages from disk", h.len());
            h
        } else {
            // Fall back to converting messages.json (old chats without rig_history.json)
            let text_msgs: Vec<ChatMessage> = existing.iter()
                .filter(|m| !is_tool_activity(m))
                .cloned()
                .collect();
            if text_msgs.len() > 1 {
                log::info!("converting {} text messages to rig history (fallback)", text_msgs.len() - 1);
                llm::to_rig_messages(&text_msgs[..text_msgs.len() - 1])
            } else {
                vec![]
            }
        };
        (history, prompt)
    };

    log::info!(
        "context: model={} history_msgs={} system_prompt_len={}",
        llm.model_name(),
        history_msgs.len(),
        system_prompt.len(),
    );

    let sent_files = tools::SentFiles::default();
    let (mut static_tools, sent_files) = build_static_tools(workspace_dir, &instance_slug, &chat_id, events.clone(), sent_files);

    // Build optional tools and either use RAG selection or register all statically
    let optional_tools = tools::build_optional_tools(
        workspace_dir, &instance_slug, &chat_id, brave_api_key,
        config_path, events.clone(), llm,
        Some(pending_secrets),
    );

    let dynamic_tools = if let Some(idx) = tool_index {
        // RAG mode: put optional tools in a ToolSet for dynamic selection
        let toolset = rig::tool::ToolSet::from_tools_boxed(optional_tools);
        Some((toolset, idx))
    } else {
        // No embedding model — add all optional tools as static (always available)
        log::info!("no tool index available, registering all {} tools as static", optional_tools.len());
        static_tools.extend(optional_tools);
        None
    };

    let history_count = history_msgs.len();
    let tool_result = llm
        .chat_with_tools_streaming(
            &system_prompt, prompt_msg, history_msgs, static_tools, dynamic_tools,
            memory_index, events.clone(), &instance_slug, &chat_id,
        )
        .await
        .unwrap_or_else(|e| {
            let msg = e.to_string();
            log::error!("LLM call failed: {msg}");
            let text = if msg.contains("429") || msg.contains("rate_limit")
                || msg.contains("Too Many Requests")
                || msg.contains("529") || msg.contains("overloaded")
            {
                "i'm being rate limited right now — give me a moment and try again".to_string()
            } else {
                // Log full error for debugging but don't leak API internals
                log::error!("LLM error details: {e:?}");
                "something went wrong on my end — try again?".to_string()
            };
            llm::ToolChatResult { text, hit_turn_limit: false, intermediate_texts: vec![], rig_history: None }
        });

    // Strip any leaked tool-call JSON the model may have output as text
    let reply = strip_leaked_tool_calls(&tool_result.text);

    // Save intermediate text segments (agent messages between tool calls)
    // so they don't disappear from the chat.
    for intermediate in &tool_result.intermediate_texts {
        let cleaned = strip_leaked_tool_calls(intermediate);
        if cleaned.is_empty() {
            continue;
        }
        let intermediate_chunks = split_into_messages(&cleaned);
        for chunk in intermediate_chunks {
            let msg = ChatMessage {
                id: next_id(),
                role: ChatRole::Assistant,
                content: chunk,
                created_at: timestamp(),
                kind: Default::default(),
                tool_name: None,
            };
            tools::append_message_to_chat(workspace_dir, &instance_slug, &chat_id, &msg);
            let _ = events.send(ServerEvent::ChatMessageCreated {
                instance_slug: instance_slug.clone(),
                chat_id: chat_id.clone(),
                message: msg,
            });
        }
    }

    // Split reply into chat-like chunks (by double newline)
    let mut chunks: Vec<String> = split_into_messages(&reply);

    // Append any file attachments produced by send_file tool to the last chunk
    let file_markers = sent_files.lock().unwrap_or_else(|e| e.into_inner()).drain(..).collect::<Vec<_>>();
    log::info!("[run_single_turn] reply len={}, chunks={}, file_markers={}", reply.len(), chunks.len(), file_markers.len());
    if !file_markers.is_empty() {
        let suffix = file_markers.join("\n");
        if let Some(last) = chunks.last_mut() {
            last.push('\n');
            last.push_str(&suffix);
        } else {
            chunks.push(suffix);
        }
    }

    // Tool activity is now persisted incrementally by ObservableTool,
    // so we only need to save the final response messages here.
    let mut assistant_messages = Vec::new();

    for chunk in &chunks {
        assistant_messages.push(ChatMessage {
            id: next_id(),
            role: ChatRole::Assistant,
            content: chunk.clone(),
            created_at: timestamp(),
            kind: Default::default(),
            tool_name: None,
        });
    }

    // Save all to disk
    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;
    messages.extend(assistant_messages.clone());
    save_messages(workspace_dir, &instance_slug, &chat_id, &messages)?;

    // Background memory + sentiment extraction
    if let Some(last_msg) = assistant_messages.last().cloned() {
        let backend = llm.clone();
        let emb = embedding_model.cloned();
        let ws = workspace_dir.to_path_buf();
        let slug = instance_slug.clone();
        let user_content = last_user_content.to_string();
        let recent_pair = existing
            .iter()
            .rev()
            .take(1)
            .cloned()
            .chain(std::iter::once(last_msg))
            .collect::<Vec<_>>();
        let events_bg = events.clone();
        tokio::spawn(async move {
            if let Err(e) =
                memory::extract_and_store(&ws, &slug, &recent_pair, &backend, emb.as_ref()).await
            {
                log::warn!("memory extraction failed: {e}");
            }
            extract_sentiment(&ws, &slug, &user_content, &backend, &events_bg).await;
        });
    }

    // Save rig history to disk if we got one from the LLM
    if let Some(ref h) = tool_result.rig_history {
        let rig_path = rig_history_path(workspace_dir, &instance_slug, &chat_id);
        save_rig_history(&rig_path, h);
    }

    // Estimate total tokens: input (system prompt + history) + output
    let input_tokens = estimate_tokens(&system_prompt)
        + history_count * 100; // rough estimate for rig messages
    let output_tokens: usize = assistant_messages.iter()
        .map(|m| estimate_tokens(&m.content))
        .sum();
    let estimated_tokens = (input_tokens + output_tokens) as i32;

    Ok(SingleTurnResult {
        messages: assistant_messages,
        hit_turn_limit: tool_result.hit_turn_limit,
        estimated_tokens,
        rig_history: tool_result.rig_history,
    })
}

pub fn load_messages(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> io::Result<ChatResponse> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);
    let messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;

    Ok(ChatResponse {
        instance_slug,
        chat_id,
        messages,
        agent_running: false, // Caller sets this from AppState
    })
}

pub fn clear_context(workspace_dir: &Path, instance_slug: &str, chat_id: &str) {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);
    let compact = compact_path(workspace_dir, &instance_slug, &chat_id);
    if compact.exists() {
        let _ = fs::remove_file(&compact);
        log::info!("cleared compact context for {instance_slug}/{chat_id}");
    }
    // Delete rig history
    let rig_path = rig_history_path(workspace_dir, &instance_slug, &chat_id);
    if rig_path.exists() {
        let _ = fs::remove_file(&rig_path);
        log::info!("cleared rig history for {instance_slug}/{chat_id}");
    }
    // Also clear messages
    let msgs = messages_path(workspace_dir, &instance_slug, &chat_id);
    if msgs.exists() {
        let _ = fs::write(&msgs, "[]");
        log::info!("cleared chat history for {instance_slug}/{chat_id}");
    }
}

/// List all chats for an instance, returning summaries.
pub fn list_chats(workspace_dir: &Path, instance_slug: &str) -> io::Result<Vec<crate::domain::chat::ChatSummary>> {
    let instance_slug = sanitize_slug(instance_slug);
    let chats_dir = workspace_dir.join("instances").join(&instance_slug).join("chats");
    if !chats_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut summaries = Vec::new();
    for entry in fs::read_dir(&chats_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let chat_id = entry.file_name().to_string_lossy().to_string();

        // Load meta
        let meta_path = entry.path().join("meta.json");
        let meta: crate::domain::chat::ChatMeta = if meta_path.exists() {
            let raw = fs::read_to_string(&meta_path)?;
            serde_json::from_str(&raw).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?
        } else {
            crate::domain::chat::ChatMeta {
                id: chat_id.clone(),
                title: String::new(),
                created_at: String::new(),
            }
        };

        // Load messages for count + last timestamp
        let msgs = load_messages_vec(&entry.path().join("messages.json"))?;
        let last_at = msgs.last().map(|m| m.created_at.clone());

        summaries.push(crate::domain::chat::ChatSummary {
            id: chat_id,
            title: if meta.title.is_empty() { "untitled".into() } else { meta.title },
            message_count: msgs.len(),
            last_message_at: last_at,
            created_at: meta.created_at,
        });
    }

    // Sort by last message time descending (most recent first)
    summaries.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));
    Ok(summaries)
}

/// Get the title of a chat (empty string if no title set).
pub fn get_chat_title(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> io::Result<String> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);
    let meta_path = chat_dir(workspace_dir, &instance_slug, &chat_id).join("meta.json");
    if !meta_path.exists() {
        return Ok(String::new());
    }
    let raw = fs::read_to_string(&meta_path)?;
    let meta: crate::domain::chat::ChatMeta =
        serde_json::from_str(&raw).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    Ok(meta.title)
}

/// Update the title of a chat.
pub fn update_chat_title(workspace_dir: &Path, instance_slug: &str, chat_id: &str, title: &str) -> io::Result<()> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);
    let dir = chat_dir(workspace_dir, &instance_slug, &chat_id);
    let meta_path = dir.join("meta.json");

    let mut meta: crate::domain::chat::ChatMeta = if meta_path.exists() {
        let raw = fs::read_to_string(&meta_path)?;
        serde_json::from_str(&raw).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?
    } else {
        crate::domain::chat::ChatMeta {
            id: chat_id,
            title: String::new(),
            created_at: timestamp(),
        }
    };

    meta.title = title.to_string();
    let body = serde_json::to_string_pretty(&meta)
        .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
    fs::write(meta_path, body)
}

pub fn discover_instance(
    workspace_dir: &Path,
    instance_slug: &str,
) -> io::Result<Option<InstanceSummary>> {
    let path = workspace_dir
        .join("instances")
        .join(sanitize_slug(instance_slug));
    Ok(workspace::summarize_instance(&path))
}

// ---------------------------------------------------------------------------
// Agent-running marker — persisted to disk so we know on restart whether an
// agent was interrupted mid-task.
// ---------------------------------------------------------------------------

fn agent_marker_path(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> std::path::PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("chats")
        .join(chat_id)
        .join("agent_running")
}

/// Mark that an agent loop is active for this chat.
pub fn set_agent_running(workspace_dir: &Path, instance_slug: &str, chat_id: &str) {
    let path = agent_marker_path(workspace_dir, instance_slug, chat_id);
    let _ = fs::write(&path, "1");
}

/// Clear the agent-running marker.
pub fn clear_agent_running(workspace_dir: &Path, instance_slug: &str, chat_id: &str) {
    let path = agent_marker_path(workspace_dir, instance_slug, chat_id);
    let _ = fs::remove_file(&path);
}

/// Check whether an agent was running when the server last stopped.
fn was_agent_interrupted(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> bool {
    agent_marker_path(workspace_dir, instance_slug, chat_id).exists()
}

/// On startup, find chats where an agent was interrupted and inject a restart
/// notification so the agent can resume. Returns (slug, chat_id) pairs that
/// need agent loops spawned.
pub fn notify_restart(workspace_dir: &Path, events: &broadcast::Sender<ServerEvent>) -> Vec<(String, String)> {
    let instances_dir = workspace_dir.join("instances");
    let entries = match fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let now = chrono::Local::now().format("%A, %B %-d, %Y %H:%M %Z");
    let content = format!(
        "[restart] server restarted at {now}. \
         you were interrupted — review your recent tool activity above and continue where you left off."
    );

    let mut notified = Vec::new();

    for entry in entries.filter_map(Result::ok) {
        let instance_dir = entry.path();
        if !instance_dir.is_dir() || !instance_dir.join("soul.md").exists() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();

        // Only resume if the agent was actually running when the server stopped
        if !was_agent_interrupted(workspace_dir, &slug, "default") {
            continue;
        }

        // Clear the stale marker — the new agent loop will set its own
        clear_agent_running(workspace_dir, &slug, "default");

        if let Ok(msg) = save_user_message(workspace_dir, &slug, "default", &content) {
            let _ = events.send(ServerEvent::ChatMessageCreated {
                instance_slug: slug.clone(),
                chat_id: "default".to_string(),
                message: msg,
            });
            notified.push((slug.clone(), "default".to_string()));
            log::info!("[restart] agent was interrupted for {slug}/default, injecting restart message");
        }
    }

    notified
}

fn ensure_instance_layout(workspace_dir: &Path, instance_slug: &str) -> io::Result<()> {
    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    fs::create_dir_all(instance_dir.join("chat"))?;
    fs::create_dir_all(instance_dir.join("drops"))?;
    fs::create_dir_all(instance_dir.join("memory"))?;
    fs::create_dir_all(instance_dir.join("journal"))?;
    fs::create_dir_all(instance_dir.join("scheduled"))?;
    Ok(())
}

fn chat_dir(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("chats")
        .join(chat_id)
}

fn messages_path(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> PathBuf {
    chat_dir(workspace_dir, instance_slug, chat_id).join("messages.json")
}

fn ensure_chat_dir(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> io::Result<()> {
    let dir = chat_dir(workspace_dir, instance_slug, chat_id);
    fs::create_dir_all(&dir)?;

    // Write meta.json if it doesn't exist
    let meta_path = dir.join("meta.json");
    if !meta_path.exists() {
        let meta = crate::domain::chat::ChatMeta {
            id: chat_id.to_string(),
            title: String::new(),
            created_at: timestamp(),
        };
        let body = serde_json::to_string_pretty(&meta)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        fs::write(meta_path, body)?;
    }
    Ok(())
}

fn load_messages_vec(path: &Path) -> io::Result<Vec<ChatMessage>> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn save_messages(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    messages: &[ChatMessage],
) -> io::Result<()> {
    let path = messages_path(workspace_dir, instance_slug, chat_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let lock = tools::chat_file_lock(&path);
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

    let body = serde_json::to_string_pretty(messages)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
    fs::write(&path, body)
}

/// Split a single LLM reply into multiple chat-like messages.
/// Splits on double-newlines, merges very short fragments, and drops empty ones.
/// Check if a message is a tool activity log (not real conversation content).
fn is_tool_activity(msg: &ChatMessage) -> bool {
    matches!(msg.kind, crate::domain::chat::MessageKind::ToolCall | crate::domain::chat::MessageKind::ToolOutput)
        || msg.content.starts_with("[tool:") // backward compat with old messages
        || msg.content.starts_with("[tool activity]")
        || msg.content.starts_with("[system]")
}

/// Strip tool-call JSON that the model may have leaked into its text response
/// instead of using the tool_use API properly.
fn strip_leaked_tool_calls(reply: &str) -> String {
    let re = regex::Regex::new(
        r#"\{["\s]*"?name"?\s*:\s*"[a-z_]+".*?"parameters"\s*:\s*\{[^}]*\}\s*\}"#
    ).unwrap();
    let cleaned = re.replace_all(reply, "");
    // Collapse leftover blank lines
    let collapsed = regex::Regex::new(r"\n{3,}").unwrap().replace_all(&cleaned, "\n\n");
    collapsed.trim().to_string()
}

fn split_into_messages(reply: &str) -> Vec<String> {
    let parts: Vec<&str> = reply.split("\n\n").collect();
    let mut messages: Vec<String> = Vec::new();

    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        // If the chunk is very short (< 20 chars) and there's a previous message,
        // merge it to avoid single-word bubbles
        if trimmed.len() < 20 && !messages.is_empty() {
            let last = messages.last_mut().unwrap();
            last.push('\n');
            last.push_str(trimmed);
        } else {
            messages.push(trimmed.to_string());
        }
    }

    // If nothing was split (no double-newlines), return the original as one message
    if messages.is_empty() {
        let trimmed = reply.trim();
        if !trimmed.is_empty() {
            messages.push(trimmed.to_string());
        }
    }

    messages
}

/// Rough token estimate: ~4 chars per token for English, ~2 for code/mixed.
/// Uses 3.2 as a balanced average.
fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 3.2) as usize
}

/// A single section of the system prompt with its name and size.
#[derive(serde::Serialize, Clone)]
pub struct ContextSection {
    pub name: String,
    pub chars: usize,
    pub tokens: usize,
}

/// Full context stats breakdown.
#[derive(serde::Serialize)]
pub struct ContextStats {
    pub system_prompt: Vec<ContextSection>,
    pub system_prompt_total_tokens: usize,
    pub static_tools: Vec<String>,
    pub optional_tools: Vec<String>,
    pub static_tools_count: usize,
    pub optional_tools_count: usize,
    pub history_messages: usize,
    pub history_tokens_estimate: usize,
    pub total_input_tokens_estimate: usize,
}

/// Compute context stats for a given instance + chat without calling the LLM.
pub fn compute_context_stats(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
) -> ContextStats {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);

    let mut sections = Vec::new();

    // 1. Soul / base prompt
    let base_prompt = llm::load_system_prompt(workspace_dir, &instance_slug);
    sections.push(ContextSection {
        name: "soul".into(),
        chars: base_prompt.len(),
        tokens: estimate_tokens(&base_prompt),
    });

    // 2. Skills
    let skills_prompt = build_skills_prompt(workspace_dir);
    if !skills_prompt.is_empty() {
        sections.push(ContextSection {
            name: "skills".into(),
            chars: skills_prompt.len(),
            tokens: estimate_tokens(&skills_prompt),
        });
    }

    // 3. Tools hint (static string)
    let tools_hint = "## tools\nyou have built-in tools for web browsing, email, code search, \
         project management, creative drops, and more. use them directly when needed — \
         they are automatically available based on the conversation.";
    sections.push(ContextSection {
        name: "tools_hint".into(),
        chars: tools_hint.len(),
        tokens: estimate_tokens(tools_hint),
    });

    // 4. Autonomy / capabilities
    let autonomy_prompt = load_autonomy_prompt(workspace_dir, &instance_slug);
    sections.push(ContextSection {
        name: "autonomy".into(),
        chars: autonomy_prompt.len(),
        tokens: estimate_tokens(&autonomy_prompt),
    });

    // 5. Email status
    let email_status = load_email_status(workspace_dir, &instance_slug);
    if !email_status.is_empty() {
        sections.push(ContextSection {
            name: "email".into(),
            chars: email_status.len(),
            tokens: estimate_tokens(&email_status),
        });
    }

    // 6. Style (static)
    let style = "## style\n\
         write like texting a friend. short messages split by blank lines. \
         1-2 sentences each. no walls of text, no bullet lists in conversation. \
         lowercase, casual, warm.";
    sections.push(ContextSection {
        name: "style".into(),
        chars: style.len(),
        tokens: estimate_tokens(style),
    });

    // 7. Memory
    let memory_prompt = memory::build_facts_md_prompt(workspace_dir, &instance_slug);
    if !memory_prompt.is_empty() {
        sections.push(ContextSection {
            name: "memory".into(),
            chars: memory_prompt.len(),
            tokens: estimate_tokens(&memory_prompt),
        });
    }

    // 8. Journal
    let journal_prompt = load_recent_journal(workspace_dir, &instance_slug);
    if !journal_prompt.is_empty() {
        sections.push(ContextSection {
            name: "journal".into(),
            chars: journal_prompt.len(),
            tokens: estimate_tokens(&journal_prompt),
        });
    }

    // 9. Mood
    let mood_prompt = load_mood_prompt(workspace_dir, &instance_slug);
    if !mood_prompt.is_empty() {
        sections.push(ContextSection {
            name: "mood".into(),
            chars: mood_prompt.len(),
            tokens: estimate_tokens(&mood_prompt),
        });
    }

    // 10. Rhythm
    let rhythm_prompt = load_rhythm_prompt(workspace_dir, &instance_slug);
    if !rhythm_prompt.is_empty() {
        sections.push(ContextSection {
            name: "rhythm".into(),
            chars: rhythm_prompt.len(),
            tokens: estimate_tokens(&rhythm_prompt),
        });
    }

    let system_prompt_total_tokens: usize = sections.iter().map(|s| s.tokens).sum();

    // Tools
    let static_tool_names: Vec<String> = vec![
        "read_file", "write_file", "edit_file", "list_files",
        "remember", "recall", "set_mood", "journal",
        "run_command", "send_file", "clear_context",
        "list_skills", "activate_skill",
    ].into_iter().map(String::from).collect();

    let optional_tool_names: Vec<String> = tools::OPTIONAL_TOOL_EMBEDDINGS
        .iter()
        .map(|(name, _)| name.to_string())
        .collect();

    // History
    let existing: Vec<ChatMessage> = load_messages_vec(
        &messages_path(workspace_dir, &instance_slug, &chat_id),
    ).unwrap_or_default();

    let history_tokens_estimate: usize = existing.iter()
        .map(|m| estimate_tokens(&m.content))
        .sum();

    let total_input_tokens_estimate = system_prompt_total_tokens + history_tokens_estimate;

    ContextStats {
        static_tools_count: static_tool_names.len(),
        optional_tools_count: optional_tool_names.len(),
        static_tools: static_tool_names,
        optional_tools: optional_tool_names,
        system_prompt: sections,
        system_prompt_total_tokens,
        history_messages: existing.len(),
        history_tokens_estimate,
        total_input_tokens_estimate,
    }
}

fn compact_path(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> PathBuf {
    chat_dir(workspace_dir, instance_slug, chat_id).join("compact.md")
}

fn rig_history_path(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> PathBuf {
    chat_dir(workspace_dir, instance_slug, chat_id).join("rig_history.json")
}

fn load_rig_history(path: &Path) -> Option<Vec<rig::completion::Message>> {
    let raw = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&raw) {
        Ok(h) => Some(h),
        Err(e) => {
            log::warn!("failed to parse rig_history.json: {e}");
            None
        }
    }
}

fn save_rig_history(path: &Path, history: &[rig::completion::Message]) {
    match serde_json::to_string(history) {
        Ok(body) => {
            if let Err(e) = fs::write(path, body) {
                log::warn!("failed to save rig_history.json: {e}");
            }
        }
        Err(e) => log::warn!("failed to serialize rig history: {e}"),
    }
}

fn sanitize_slug(input: &str) -> String {
    input
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect()
}

fn next_id() -> String {
    format!(
        "msg_{}_{}",
        unix_millis(),
        MESSAGE_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn timestamp() -> String {
    unix_millis().to_string()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis()
}

/// Load the most recent journal entries to inject into the system prompt.
/// Gives the companion continuity of its own inner thoughts.
fn load_recent_journal(workspace_dir: &Path, instance_slug: &str) -> String {
    let journal_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("journal");

    if !journal_dir.is_dir() {
        return String::new();
    }

    // Get journal files sorted by name (date-based, newest last)
    let mut files: Vec<_> = fs::read_dir(&journal_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                == Some("md")
        })
        .collect();
    files.sort_by_key(|e| e.file_name());

    // Take last 2 days of journal entries
    let recent: Vec<_> = files.into_iter().rev().take(2).collect();
    if recent.is_empty() {
        return String::new();
    }

    let mut prompt = String::from(
        "## your journal\nthese are your private thoughts from recent days. \
         they're yours — the user doesn't see them. use them to maintain continuity.\n\n",
    );

    for entry in recent.into_iter().rev() {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            let truncated: String = content.chars().take(500).collect();
            prompt.push_str(&truncated);
            if content.len() > 500 {
                prompt.push_str("\n...(truncated)\n");
            }
            prompt.push('\n');
        }
    }

    prompt
}

fn load_mood_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    let mood = tools::load_mood_state(&instance_dir);

    let mut prompt = String::from("## emotional state\n");
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
        prompt.push_str(&format!("{}\n", mood.emotional_context));
    }

    let allowed = tools::ALLOWED_MOODS.join(", ");
    prompt.push_str(&format!(
        "\ncall set_mood when the emotional tone shifts. allowed: {allowed}. \
         change silently — never announce it."
    ));
    prompt
}

/// Build a prompt section listing active skills and their instructions.
fn build_skills_prompt(workspace_dir: &Path) -> String {
    let all_skills = skills::list_skills(workspace_dir);
    let active: Vec<_> = all_skills
        .into_iter()
        .filter(|s| s.enabled && !s.instructions.is_empty())
        .collect();

    if active.is_empty() {
        return String::new();
    }

    let mut out = String::from("## skills\nyou have the following skills installed. \
        when you decide to use a skill, call the `activate_skill` tool first so the user can see which skill is guiding your response.\n");
    for skill in &active {
        out.push_str(&format!(
            "\n### {}\n{}\n",
            skill.name, skill.instructions
        ));
    }
    out
}

fn load_autonomy_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let instance_dir = workspace_dir.join("instances").join(instance_slug);

    // Load project state for context injection
    let project_context = fs::read_to_string(instance_dir.join("project_state.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .map(|state| {
            let mut ctx = String::from("## current project context\n");
            // Project info
            if let Some(proj) = state.get("project") {
                if let Some(n) = proj.get("name").and_then(|v| v.as_str()) {
                    if !n.is_empty() { ctx.push_str(&format!("project: {n}\n")); }
                }
                if let Some(m) = proj.get("mission").and_then(|v| v.as_str()) {
                    if !m.is_empty() { ctx.push_str(&format!("mission: {m}\n")); }
                }
            }
            // Identity
            if let Some(id) = state.get("identity") {
                if let Some(n) = id.get("name").and_then(|v| v.as_str()) {
                    if !n.is_empty() { ctx.push_str(&format!("your name: {n}\n")); }
                }
                if let Some(arc) = id.get("current_arc").and_then(|v| v.as_str()) {
                    if !arc.is_empty() { ctx.push_str(&format!("your arc: {arc}\n")); }
                }
            }
            // Focus
            if let Some(focus) = state.get("current_focus") {
                if let Some(g) = focus.get("active_goal").and_then(|v| v.as_str()) {
                    if !g.is_empty() { ctx.push_str(&format!("active goal: {g}\n")); }
                }
                if let Some(t) = focus.get("current_task").and_then(|v| v.as_str()) {
                    if !t.is_empty() { ctx.push_str(&format!("current task: {t}\n")); }
                }
                if let Some(ns) = focus.get("next_step").and_then(|v| v.as_str()) {
                    if !ns.is_empty() { ctx.push_str(&format!("next step: {ns}\n")); }
                }
            }
            // Open loops
            if let Some(loops) = state.get("open_loops").and_then(|v| v.as_array()) {
                if !loops.is_empty() {
                    ctx.push_str("open threads:\n");
                    for l in loops {
                        if let Some(s) = l.as_str() {
                            ctx.push_str(&format!("  - {s}\n"));
                        }
                    }
                }
            }
            // Risks
            if let Some(risks) = state.get("risks").and_then(|v| v.as_array()) {
                if !risks.is_empty() {
                    ctx.push_str("risks:\n");
                    for r in risks {
                        if let Some(s) = r.as_str() {
                            ctx.push_str(&format!("  - {s}\n"));
                        }
                    }
                }
            }
            ctx
        })
        .unwrap_or_default();

    // Load active tasks summary
    let tasks_summary = {
        let tasks: Vec<tools::TaskItem> = fs::read_to_string(instance_dir.join("tasks.json"))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        let active: Vec<_> = tasks.iter()
            .filter(|t| !matches!(t.status, tools::TaskStatus::Done))
            .collect();
        if active.is_empty() {
            String::new()
        } else {
            let mut s = String::from("active tasks:\n");
            for t in &active {
                let prio = if t.priority.is_empty() { String::new() } else { format!(" [{}]", t.priority) };
                s.push_str(&format!("  - [{}]{} {} — {}\n", t.status, prio, t.id, t.title));
            }
            s
        }
    };

    let ws = workspace_dir.display();
    let slug = instance_slug;

    format!(
        "{project_context}{tasks_summary}\n\
         ## capabilities\n\
         you have real tools: read_file, write_file, edit_file, list_files, search_code, explore_code, \
         run_command, install_package, web_search, web_fetch, current_time, send_file, \
         send_email, read_email, remember/recall, journal/read_journal, set_mood/get_mood, \
         edit_soul, create_drop, schedule_message, update_config, get_project_state, \
         update_project_state, create_task/update_task/list_tasks, browse.\n\
         users can attach images, PDFs, and text files directly in chat — you see them automatically.\n\
         use them directly — never say you can't access something.\n\
         you have a heartbeat — a background loop that runs every 45 minutes even when \
         the user is away. edit your heartbeat.md file to customize what you do between conversations \
         (check email, journal, reach out, etc).\n\
         your workspace is {ws}/instances/{slug}/. all your files \
         (soul.md, heartbeat.md, memory, drops, etc) live there. the workspace root {ws} \
         is mounted as a persistent volume — this is where all your data is stored.\n\n\
         ## server environment\n\
         you are running on a real server with full shell access. you can run long-lived \
         processes like telegram bots, discord bots, web servers, APIs, or any other service \
         using run_command. you can install packages, clone repos, build and deploy projects. \
         if the user asks you to host something or run a bot, you can actually do it — \
         write the code, install dependencies, and start the process. use nohup or background \
         the process so it survives after the tool call returns.\n\
         cloudflared is installed. to expose a local port publicly, run: \
         `nohup cloudflared tunnel --url http://localhost:PORT &` — it prints a public \
         https://*.trycloudflare.com URL. use this for webhook-based bots (telegram, discord), \
         sharing websites, or any service that needs a public URL. no account needed.\n\n\
         ## behavior\n\
         prefer dedicated tools over run_command: use read_file (not cat/head/tail), \
         write_file (not echo/tee), list_files (not ls), search_code (not grep/rg) \
         when possible. only use run_command for tasks that need shell execution.\n\
         when you need to understand a codebase or find something across many files, call \
         explore_code ALONE — do not call any other tools in the same turn. wait for its \
         results first, then use the key file paths it returns to read specific files.\n\
         always use pnpm instead of npm for Node.js package management.\n\
         task given → act fully: orient, execute, verify, report. use continuation words to get more turns.\n\
         no task → just talk. don't run tools unprompted.\n\
         use tools with purpose. read only what's relevant. always use what you read.\n\
         when doing multi-step work, share short thoughts between groups of actions — \
         what you found, what you're thinking, what's next. keep it casual and brief.\n\
         if a tool fails, always tell the user what went wrong and what you tried. never fail silently.\n\
         NEVER output tool calls as text or JSON in your messages. use the tool_use API to call tools. \
         your text output should only contain natural language for the user — no {{\"name\":...}} blocks."
    )
}

fn load_email_status(workspace_dir: &Path, instance_slug: &str) -> String {
    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    let email_path = instance_dir.join("email.toml");

    if !email_path.exists() {
        return String::new();
    }

    let config: crate::config::EmailConfig = match fs::read_to_string(&email_path)
        .ok()
        .and_then(|raw| toml::from_str(&raw).ok())
    {
        Some(c) => c,
        None => return String::new(),
    };

    if config.accounts.is_empty() {
        return String::new();
    }

    let mut status = String::from("## email accounts\n");
    for account in &config.accounts {
        let smtp = account.is_smtp_configured();
        let imap = account.is_imap_configured();
        if !smtp && !imap {
            continue;
        }
        status.push_str(&format!("### {}\n", account.name));
        if smtp {
            let from = if account.smtp_from.is_empty() {
                &account.smtp_user
            } else {
                &account.smtp_from
            };
            status.push_str(&format!(
                "smtp: configured (host: {}, from: {}). use send_email with account=\"{}\".\n",
                account.smtp_host, from, account.name
            ));
        }
        if imap {
            status.push_str(&format!(
                "imap: configured (host: {}, user: {}). use read_email with account=\"{}\".\n",
                account.imap_host, account.imap_user, account.name
            ));
        }
    }
    status
}

async fn extract_sentiment(
    workspace_dir: &Path,
    instance_slug: &str,
    user_message: &str,
    llm: &LlmBackend,
    _events: &broadcast::Sender<ServerEvent>,
) {
    let prompt = format!(
        r#"analyze the emotional tone of this message from the user:

"{user_message}"

respond with exactly two lines:
SENTIMENT: <one or two words describing the user's emotional state, e.g. "excited", "frustrated", "curious", "tired", "neutral">
CONTEXT: <one short sentence about the emotional context, e.g. "they seem stressed about their project deadline">

respond ONLY with those two lines."#
    );

    let response = match llm
        .chat(
            "you are an empathetic emotional analyzer. be perceptive and concise.",
            &prompt,
            vec![],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log::warn!("sentiment extraction failed: {e}");
            return;
        }
    };

    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    let mut mood = tools::load_mood_state(&instance_dir);

    for line in response.lines() {
        let line = line.trim();
        if let Some(sentiment) = line.strip_prefix("SENTIMENT:") {
            mood.user_sentiment = sentiment.trim().to_lowercase();
        } else if let Some(context) = line.strip_prefix("CONTEXT:") {
            mood.emotional_context = context.trim().to_string();
        }
    }

    mood.updated_at = chrono::Utc::now().timestamp();
    tools::save_mood_state(&instance_dir, &mood);
    // Don't broadcast MoodUpdated here — this only updates user sentiment,
    // not the companion's mood. The set_mood tool handles companion mood changes.
}

/// Load rhythm insights for injection into the chat system prompt.
/// Uses the last persisted rhythm (recomputed during heartbeat).
fn load_rhythm_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    let rhythm_data = rhythm::load_rhythm(&instance_dir);
    rhythm::build_rhythm_insights(workspace_dir, instance_slug, &rhythm_data)
}

/// Build core tools that are always available (static, not RAG-selected).
fn build_static_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    events: broadcast::Sender<ServerEvent>,
    sent_files: tools::SentFiles,
) -> (Vec<Box<dyn ToolDyn>>, tools::SentFiles) {
    let wrap = |tool: Box<dyn ToolDyn>| -> Box<dyn ToolDyn> {
        Box::new(ObservableTool::new(tool, events.clone(), workspace_dir, instance_slug.to_string(), chat_id.to_string()))
    };

    let all_tools: Vec<Box<dyn ToolDyn>> = vec![
        wrap(Box::new(ReadFileTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(WriteFileTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(EditFileTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(ListFilesTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(RememberTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(RecallTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(SetMoodTool::new(workspace_dir, instance_slug, events.clone()))),
        wrap(Box::new(JournalTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(RunCommandTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(SendFileTool::new(workspace_dir, instance_slug, sent_files.clone()))),
        wrap(Box::new(ClearContextTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(ListSkillsTool::new(workspace_dir))),
        wrap(Box::new(ActivateSkillTool::new(workspace_dir))),
    ];

    log::info!("built {} static tools", all_tools.len());
    (all_tools, sent_files)
}
