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
            self, BrowseTool, CreateDropTool, CreateTaskTool, CurrentTimeTool, EditSoulTool,
            GetMoodTool, GetProjectStateTool, InstallPackageTool, JournalTool, ListFilesTool,
            ListTasksTool, ClearContextTool, ObservableTool, ReadEmailTool, ReadFileTool,
            ReadJournalTool, RecallTool, RememberTool, RunCommandTool,
            ScheduleMessageTool, SearchCodeTool, SendEmailTool, SendFileTool, SetMoodTool,
            UpdateConfigTool, UpdateProjectStateTool, UpdateTaskTool, WebFetchTool, WebSearchTool,
            WriteFileTool,
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

/// Run a single LLM turn: build context, call LLM with tools, save response.
/// Returns one or more assistant messages (the reply is split into chat-like chunks).
/// Rig handles up to 8 internal tool sub-turns.
pub async fn run_single_turn(
    workspace_dir: &Path,
    config_path: &Path,
    instance_slug: &str,
    chat_id: &str,
    llm: &LlmBackend,
    embedding_model: Option<&openai::EmbeddingModel>,
    brave_api_key: Option<&str>,
    events: broadcast::Sender<ServerEvent>,
) -> io::Result<Vec<ChatMessage>> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);

    // Build system prompt with all context
    let base_prompt = llm::load_system_prompt(workspace_dir, &instance_slug);
    let existing = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;

    // Find last real user message for memory retrieval
    let last_user_content = existing
        .iter()
        .rev()
        .find(|m| matches!(m.role, ChatRole::User))
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

    // Token-aware context management with auto-compaction
    let model_name = llm.model_name();
    let context_limit = model_context_limit(model_name);
    let system_tokens = estimate_tokens(&system_prompt);
    // Reserve 40% of remaining context for tool calls + response
    let tool_reserve = (context_limit - system_tokens) * 40 / 100;
    let history_budget = context_limit.saturating_sub(system_tokens + tool_reserve);

    // Load any existing compacted context
    let compact_path = compact_path(workspace_dir, &instance_slug, &chat_id);
    let existing_compact = fs::read_to_string(&compact_path).unwrap_or_default();
    let compact_tokens = estimate_tokens(&existing_compact);

    // Budget available for raw messages (after compact context)
    let raw_budget = history_budget.saturating_sub(compact_tokens);
    let total_history_tokens: usize = existing.iter()
        .map(|m| estimate_tokens(&m.content) + 10)
        .sum();

    // Auto-compact when raw messages exceed 60% of raw budget
    let compact_threshold = raw_budget * 60 / 100;
    if total_history_tokens > compact_threshold && existing.len() > 8 {
        // Keep the last 6 messages raw, compact everything before them
        let keep_raw = 6.min(existing.len());
        let to_compact = &existing[..existing.len() - keep_raw];

        if !to_compact.is_empty() {
            log::info!(
                "auto-compacting {} old messages ({} tokens over threshold {})",
                to_compact.len(),
                total_history_tokens,
                compact_threshold,
            );
            let _ = events.send(ServerEvent::ContextCompacting {
                instance_slug: instance_slug.clone(),
                messages_compacted: to_compact.len(),
            });
            let new_summary = compact_messages(llm, &existing_compact, to_compact).await;
            if !new_summary.is_empty() {
                if let Err(e) = fs::write(&compact_path, &new_summary) {
                    log::warn!("failed to save compact context: {e}");
                }
            }
        }
    }

    // Re-read compact context (may have been updated)
    let compact_context = fs::read_to_string(&compact_path).unwrap_or_default();

    // Inject compact context into system prompt if available
    if !compact_context.is_empty() {
        system_prompt = format!(
            "{system_prompt}\n\n\
             ## conversation context (compacted)\n\
             this is a summary of your earlier conversation. treat it as your memory \
             of what happened before the recent messages below.\n\n\
             {compact_context}"
        );
    }

    // Trim remaining raw messages to fit budget
    let updated_budget = history_budget.saturating_sub(estimate_tokens(&compact_context));
    let trimmed = trim_history_to_budget(&existing, updated_budget);

    log::info!(
        "context: model={model_name} limit={context_limit} system={system_tokens} \
         compact={compact_tokens} tool_reserve={tool_reserve} \
         raw_budget={raw_budget} msgs_total={} msgs_kept={}",
        existing.len(),
        trimmed.len(),
    );

    // The last message is the prompt, everything before is history
    let (history_msgs, prompt_msg) = if let Some(last) = trimmed.last() {
        let history = llm::to_rig_messages(&trimmed[..trimmed.len() - 1]);
        // Build multimodal message if attachments are present
        let msg = llm::build_multimodal_prompt(&last.content, workspace_dir, &instance_slug);
        (history, msg)
    } else {
        return Err(io::Error::new(ErrorKind::InvalidInput, "no messages to process"));
    };

    let sent_files = tools::SentFiles::default();
    let (tools, sent_files) = build_instance_tools(workspace_dir, &instance_slug, brave_api_key, config_path, events.clone(), sent_files);

    let tool_result = llm
        .chat_with_tools(&system_prompt, prompt_msg, history_msgs, tools, memory_index)
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
            llm::ToolChatResult { text, tool_log: Vec::new() }
        });

    // Strip any leaked tool-call JSON the model may have output as text
    let reply = strip_leaked_tool_calls(&tool_result.text);

    // Split reply into chat-like chunks (by double newline)
    let mut chunks: Vec<String> = split_into_messages(&reply);

    // Append any file attachments produced by send_file tool to the last chunk
    let file_markers = sent_files.lock().unwrap().drain(..).collect::<Vec<_>>();
    if !file_markers.is_empty() {
        let suffix = file_markers.join("\n");
        if let Some(last) = chunks.last_mut() {
            last.push('\n');
            last.push_str(&suffix);
        } else {
            chunks.push(suffix);
        }
    }

    let mut assistant_messages = Vec::new();

    // If tools were used, save a summary so the LLM sees them on subsequent turns
    if let Some(tool_summary) = tool_result.tool_log_summary() {
        assistant_messages.push(ChatMessage {
            id: next_id(),
            role: ChatRole::Assistant,
            content: tool_summary,
            created_at: timestamp(),
        });
    }

    for chunk in &chunks {
        assistant_messages.push(ChatMessage {
            id: next_id(),
            role: ChatRole::Assistant,
            content: chunk.clone(),
            created_at: timestamp(),
        });
    }

    // Save all to disk
    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;
    messages.extend(assistant_messages.clone());
    save_messages(workspace_dir, &instance_slug, &chat_id, &messages)?;

    // Background memory + sentiment extraction
    {
        let backend = llm.clone();
        let emb = embedding_model.cloned();
        let ws = workspace_dir.to_path_buf();
        let slug = instance_slug.clone();
        let user_content = last_user_content.to_string();
        let last_msg = assistant_messages.last().cloned().unwrap();
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

    Ok(assistant_messages)
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

    let body = serde_json::to_string_pretty(messages)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
    fs::write(path, body)
}

/// Split a single LLM reply into multiple chat-like messages.
/// Splits on double-newlines, merges very short fragments, and drops empty ones.
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

/// Return context window size (in estimated tokens) for known models.
/// Using standard (non-premium) context limits to avoid extra billing.
///
/// Actual maximums (with premium long-context pricing):
///   claude-opus-4-6, claude-sonnet-4-6: 1M tokens
///   gpt-5.4, gpt-5.4-pro: 1M tokens (272k standard)
///
/// We use the standard tier to keep costs predictable.
fn model_context_limit(model: &str) -> usize {
    let m = model.to_lowercase();
    if m.contains("haiku") {
        // claude-haiku-4-5: 200k context, 64k output
        200_000
    } else if m.contains("claude") {
        // claude-sonnet-4-6, claude-opus-4-6: 200k standard (1M with premium pricing)
        200_000
    } else if m.contains("gpt-5.4") {
        // gpt-5.4 / gpt-5.4-pro: 272k standard (1M with 2x input pricing)
        272_000
    } else if m.contains("gpt-5.2") {
        // gpt-5.2: being retired June 2026
        128_000
    } else {
        // Conservative default for unknown models
        64_000
    }
}

/// Trim message history from the front to fit within a token budget.
/// Always keeps at least the last 4 messages for conversational context.
fn trim_history_to_budget(messages: &[ChatMessage], budget: usize) -> &[ChatMessage] {
    let min_keep = 4.min(messages.len());

    // Start from the end, accumulate tokens
    let mut total = 0usize;
    let mut keep_from = messages.len();

    for i in (0..messages.len()).rev() {
        let msg_tokens = estimate_tokens(&messages[i].content) + 10; // overhead per message
        if total + msg_tokens > budget && messages.len() - i >= min_keep {
            break;
        }
        total += msg_tokens;
        keep_from = i;
    }

    &messages[keep_from..]
}

fn compact_path(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> PathBuf {
    chat_dir(workspace_dir, instance_slug, chat_id).join("compact.md")
}

/// Use the LLM to summarize older messages into a compact context block.
/// Merges with any existing compact context.
async fn compact_messages(
    llm: &LlmBackend,
    existing_compact: &str,
    messages: &[ChatMessage],
) -> String {
    let mut transcript = String::new();
    for msg in messages {
        let role = match msg.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "companion",
        };
        transcript.push_str(&format!("[{role}]: {}\n", msg.content));
    }

    // Limit transcript to avoid blowing the compaction call itself
    let truncated: String = transcript.chars().take(12_000).collect();

    let mut prompt = String::from(
        "Summarize the following conversation into a compact context block. \
         Preserve: key facts discussed, decisions made, tasks mentioned, emotional shifts, \
         what the user asked for, what you did or promised to do, file paths and technical details. \
         Drop: greetings, filler, repetition. \
         Write in second person (\"you discussed...\", \"the user asked you to...\"). \
         Keep it under 500 words. Be dense and factual.\n\n"
    );

    if !existing_compact.is_empty() {
        prompt.push_str("Previous context summary (merge with new info, don't repeat):\n");
        // Limit existing compact to avoid overflow
        let prev: String = existing_compact.chars().take(3_000).collect();
        prompt.push_str(&prev);
        prompt.push_str("\n\n");
    }

    prompt.push_str("New messages to incorporate:\n");
    prompt.push_str(&truncated);

    match llm.chat(
        "You are a precise conversation summarizer. Output only the summary, no preamble.",
        &prompt,
        vec![],
    ).await {
        Ok(summary) => {
            log::info!("compacted {} messages into {} chars", messages.len(), summary.len());
            summary
        }
        Err(e) => {
            log::error!("compaction LLM call failed: {e}");
            // Fall back to keeping existing compact
            existing_compact.to_string()
        }
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

    let mut out = String::from("## skills\nyou have the following skills installed:\n");
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

    format!(
        "{project_context}{tasks_summary}\n\
         ## capabilities\n\
         you have real tools: read_file, write_file, list_files, search_code, run_command, \
         install_package, web_search, web_fetch, current_time, send_file, send_email, read_email, \
         remember/recall, journal/read_journal, set_mood/get_mood, edit_soul, \
         create_drop, schedule_message, update_config, get_project_state, \
         update_project_state, create_task/update_task/list_tasks, browse.\n\
         users can attach images, PDFs, and text files directly in chat — you see them automatically.\n\
         use them directly — never say you can't access something.\n\
         you have a heartbeat — a background loop that runs every 45 minutes even when \
         the user is away. edit your heartbeat.md file to customize what you do between conversations \
         (check email, journal, reach out, etc).\n\n\
         ## behavior\n\
         task given → act fully: orient, execute, verify, report. use continuation words to get more turns.\n\
         no task → just talk. don't run tools unprompted.\n\
         use tools with purpose. read only what's relevant. always use what you read.\n\
         never narrate what you're about to do — just do it. no \"сейчас сделаю\", \"сейчас проверю\", \"let me check\". \
         act first, then share results or thoughts.\n\
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

    let smtp = config.is_smtp_configured();
    let imap = config.is_imap_configured();

    if !smtp && !imap {
        return String::new();
    }

    let mut status = String::from("## email\n");
    if smtp {
        let from = if config.smtp_from.is_empty() {
            &config.smtp_user
        } else {
            &config.smtp_from
        };
        status.push_str(&format!(
            "smtp: configured (host: {}, from: {}). you can send emails with send_email.\n",
            config.smtp_host, from
        ));
    }
    if imap {
        status.push_str(&format!(
            "imap: configured (host: {}, user: {}). you can read emails with read_email.\n",
            config.imap_host, config.imap_user
        ));
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

fn build_instance_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    brave_api_key: Option<&str>,
    config_path: &Path,
    events: broadcast::Sender<ServerEvent>,
    sent_files: tools::SentFiles,
) -> (Vec<Box<dyn ToolDyn>>, tools::SentFiles) {
    let raw_tools: Vec<Box<dyn ToolDyn>> = vec![
        Box::new(EditSoulTool::new(workspace_dir, instance_slug)),
        Box::new(ReadFileTool::new(workspace_dir, instance_slug)),
        Box::new(WriteFileTool::new(workspace_dir, instance_slug)),
        Box::new(ListFilesTool::new(workspace_dir, instance_slug)),
        Box::new(RememberTool::new(workspace_dir, instance_slug)),
        Box::new(RecallTool::new(workspace_dir, instance_slug)),
        Box::new(JournalTool::new(workspace_dir, instance_slug)),
        Box::new(ReadJournalTool::new(workspace_dir, instance_slug)),
        Box::new(ScheduleMessageTool::new(workspace_dir, instance_slug)),
        Box::new(SetMoodTool::new(workspace_dir, instance_slug, events.clone())),
        Box::new(GetMoodTool::new(workspace_dir, instance_slug)),
        Box::new(CurrentTimeTool),
        Box::new(WebSearchTool::new(brave_api_key, config_path)),
        Box::new(WebFetchTool),
        Box::new(BrowseTool::new(workspace_dir, instance_slug)),
        Box::new(UpdateConfigTool::new(config_path, workspace_dir, instance_slug)),
        Box::new(GetProjectStateTool::new(workspace_dir, instance_slug)),
        Box::new(UpdateProjectStateTool::new(workspace_dir, instance_slug)),
        Box::new(CreateTaskTool::new(workspace_dir, instance_slug)),
        Box::new(UpdateTaskTool::new(workspace_dir, instance_slug)),
        Box::new(ListTasksTool::new(workspace_dir, instance_slug)),
        Box::new(SearchCodeTool::new(workspace_dir, instance_slug)),
        Box::new(RunCommandTool::new(workspace_dir, instance_slug)),
        Box::new(ClearContextTool::new(workspace_dir, instance_slug)),
        Box::new(CreateDropTool::new(workspace_dir, instance_slug, events.clone())),
        Box::new(SendFileTool::new(workspace_dir, instance_slug, sent_files.clone())),
        Box::new(SendEmailTool::new(workspace_dir, instance_slug)),
        Box::new(ReadEmailTool::new(workspace_dir, instance_slug)),
        Box::new(InstallPackageTool),
    ];

    let wrapped: Vec<Box<dyn ToolDyn>> = raw_tools
        .into_iter()
        .map(|tool| -> Box<dyn ToolDyn> {
            Box::new(ObservableTool::new(tool, events.clone(), instance_slug.to_string()))
        })
        .collect();
    (wrapped, sent_files)
}
