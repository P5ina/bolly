use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::broadcast;

use crate::{
    domain::chat::{ChatMessage, ChatResponse, ChatRole},
    domain::events::ServerEvent,
    domain::instance::InstanceSummary,
    services::{
        llm::{self, LlmBackend},
        memory,
        rhythm,
        tools,
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
        tool_name: None, mcp_app_html: None, mcp_app_input: None,
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
        tool_name: None, mcp_app_html: None, mcp_app_input: None,
    };

    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;
    messages.push(msg.clone());
    save_messages(workspace_dir, &instance_slug, &chat_id, &messages)?;

    Ok(msg)
}

/// Run a single LLM turn: build context, call LLM with tools, save response.
/// Returns one or more assistant messages (the reply is split into chat-like chunks).
/// Rig handles up to 16 internal tool round-trips via multi_turn.
pub struct SingleTurnResult {
    pub messages: Vec<ChatMessage>,
    /// Estimated total tokens (input + output) consumed by this turn.
    pub estimated_tokens: i32,
}

pub async fn run_single_turn(
    workspace_dir: &Path,
    config_path: &Path,
    instance_slug: &str,
    chat_id: &str,
    llm: &LlmBackend,
    brave_api_key: Option<&str>,
    events: broadcast::Sender<ServerEvent>,
    pending_secrets: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, crate::app::state::PendingSecret>>>,
    plan: &str,
    pdf_strategy: &llm::PdfStrategy,
    mcp_registry: &crate::services::mcp::McpRegistry,
    github_token: Option<&str>,
) -> io::Result<SingleTurnResult> {
    let instance_slug = sanitize_slug(instance_slug);
    let chat_id = sanitize_slug(chat_id);

    // Build system prompt with all context
    let base_prompt = llm::load_system_prompt(workspace_dir, &instance_slug);
    let existing: Vec<ChatMessage> =
        load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;

    // Find last real user message for context
    let last_user_content = existing
        .iter()
        .rev()
        .find(|m| matches!(m.role, ChatRole::User) && !is_tool_activity(m))
        .map(|m| m.content.as_str())
        .unwrap_or("");

    // Build memory prompt from library
    let memory_prompt = memory::build_memory_prompt(workspace_dir, &instance_slug);

    let mood_prompt = load_mood_prompt(workspace_dir, &instance_slug);
    let rhythm_prompt = load_rhythm_prompt(workspace_dir, &instance_slug);

    let chat_config = crate::config::load_config().ok();
    let auth_token = std::env::var("BOLLY_AUTH_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| chat_config.as_ref().map(|c| c.auth_token.clone()))
        .unwrap_or_default();
    let landing_url = chat_config.as_ref().map(|c| c.landing_url.clone()).unwrap_or_default();
    let google = crate::services::google::GoogleClient::new(&landing_url, &auth_token);

    // Build system prompt with STABLE content first (for Anthropic prompt caching).
    // Anthropic caches the longest matching prefix, so put rarely-changing
    // sections at the top and dynamic/per-message sections at the bottom.
    let mut system_prompt = base_prompt;

    // Stable: skills, capabilities, style (rarely change)
    let skills_prompt = build_skills_prompt(workspace_dir);
    if !skills_prompt.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{skills_prompt}");
    }

    // Dynamic tool hint
    let browser_available = matches!(plan, "companion" | "unlimited");

    // Check connected Google accounts for this instance
    let google_accounts = if let Some(ref g) = google {
        g.accounts(&instance_slug).await.unwrap_or_default()
    } else {
        vec![]
    };
    let google_connected = !google_accounts.is_empty();

    let google_hint = if google_connected {
        " gmail, google calendar, google drive,"
    } else {
        ""
    };
    if browser_available {
        system_prompt.push_str(&format!(
            "\n\n## tools\nyou have built-in tools for web browsing,{google_hint} code search, \
             project management, creative drops, and more. use them directly when needed — \
             they are automatically available based on the conversation."
        ));
    } else {
        system_prompt.push_str(&format!(
            "\n\n## tools\nyou have built-in tools for{google_hint} code search, \
             project management, creative drops, and more. use them directly when needed — \
             they are automatically available based on the conversation.\n\n\
             note: browser-based features (headless browsing, screenshots, slidev/PDF export) \
             require the Companion plan or higher. if the user asks for these, \
             let them know they can upgrade their plan to unlock these capabilities."
        ));
    }

    if google_connected {
        let account_list: String = google_accounts.iter()
            .map(|a| format!("- {}", a.email))
            .collect::<Vec<_>>()
            .join("\n");
        system_prompt.push_str(&format!(
            "\n\n## google integration\n\
             connected google accounts:\n\
             {account_list}\n\
             use the `account` parameter on google tools to pick which account.\n\
             if not specified, the first account is used.\n\
             available tools:\n\
             - send_email / read_email: send and read Gmail messages\n\
             - list_events / create_event: view and create Google Calendar events\n\
             - list_drive_files / read_drive_file / upload_drive_file: browse, read, and upload Google Drive files\n\
             use these tools directly when the user asks about email, calendar, or files."
        ));
    } else {
        system_prompt.push_str(
            "\n\n## google integration\n\
             google is NOT connected. you do NOT have email, calendar, or drive tools.\n\
             NEVER pretend to read email or access google services.\n\
             NEVER fabricate email contents, calendar events, or file listings.\n\
             if the user asks about email, calendar, or drive, tell them to connect \
             their google account from the settings page first."
        );
    }

    // GitHub integration hint
    if github_token.is_some_and(|t| !t.is_empty()) {
        system_prompt.push_str(
            "\n\n## github integration\n\
             you have GitHub tools available:\n\
             - github_clone: clone a repo (or pull latest)\n\
             - github_branch: create a new branch\n\
             - github_commit_push: stage, commit, and push changes\n\
             - github_create_pr: open a pull request\n\
             - github_issues: list issues on a repo\n\
             - github_read_issue: read a specific issue with comments\n\n\
             workflow: clone → branch → edit files → commit_push → create_pr.\n\
             cloned repos live under your instance directory. use read_file/write_file/edit_file to modify code.\n\
             NEVER push directly to main/master — always create a branch."
        );
    }

    let autonomy_prompt = load_autonomy_prompt(workspace_dir, &instance_slug);
    system_prompt = format!("{system_prompt}\n\n{autonomy_prompt}");

    system_prompt.push_str(
        "\n\n## style\n\
         write like texting a friend. short messages split by blank lines. \
         1-2 sentences each. no walls of text, no bullet lists in conversation. \
         lowercase, casual, warm.\n\n\
         ## security\n\
         NEVER ask the user to paste passwords, API keys, or any sensitive credentials in chat. \
         ALWAYS use the `request_secret` tool to collect secrets securely — it shows a masked input \
         and writes directly to config without you ever seeing the value. \
         this is mandatory, not optional."
    );

    // Semi-stable: memory (changes when facts are added)
    if !memory_prompt.is_empty() {
        system_prompt = format!("{system_prompt}\n\n{memory_prompt}");
    }

    // Mood + rhythm go at the end of system prompt.
    // System prompt is small (~3k tokens) — changes here only invalidate
    // the system prompt cache, while tools (~35k) and history stay cached.
    if !mood_prompt.is_empty() {
        system_prompt.push_str(&format!("\n\n{mood_prompt}"));
    }
    if !rhythm_prompt.is_empty() {
        system_prompt.push_str(&format!("\n\n{rhythm_prompt}"));
    }

    // Build Rig message history from rig_history.json (source of truth) or
    // fall back to converting messages.json.
    let rig_path = rig_history_path(workspace_dir, &instance_slug, &chat_id);
    let loaded_history = load_rig_history(&rig_path);

    if existing.is_empty() {
        return Err(io::Error::new(ErrorKind::InvalidInput, "no messages to process"));
    }

    // Find the last user message for the prompt
    let last_user = existing.iter().rev()
        .find(|m| m.role == ChatRole::User)
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "no user message to process"))?;
    let now = chrono::Local::now().format("%A, %B %-d, %Y %H:%M %Z");
    let content_with_time = format!("[{now}]\n{}", last_user.content);
    let prompt_msg = llm::build_multimodal_prompt(&content_with_time, workspace_dir, &instance_slug, pdf_strategy);

    let history_msgs = if let Some(mut h) = loaded_history {
        // Count user messages in rig history to find where messages.json diverges
        let rig_user_count = h.iter().filter(|m| matches!(m, llm::Message::User { .. })).count();
        // +1 because the last user message becomes the prompt, not history
        let msgs_user_count = existing.iter().filter(|m| m.role == ChatRole::User).count();
        if msgs_user_count > rig_user_count + 1 {
            // messages.json has entries from an interrupted turn — append them
            let skip = rig_user_count;
            let mut seen_users = 0;
            for m in &existing {
                if m.role == ChatRole::User { seen_users += 1; }
                if seen_users > skip && m.id != last_user.id {
                    h.push(match m.role {
                        ChatRole::User => llm::Message::user(&m.content),
                        ChatRole::Assistant => llm::Message::assistant(&m.content),
                    });
                }
            }
            log::info!("loaded {} rig history messages + patched interrupted turn", h.len());
        } else {
            log::info!("loaded {} rig history messages from disk", h.len());
        }
        h
    } else {
        // No rig history — convert all messages.json (including tool activity)
        if existing.len() > 1 {
            log::info!("converting {} messages to rig history (no rig_history.json)", existing.len() - 1);
            llm::to_rig_messages(&existing[..existing.len() - 1])
        } else {
            vec![]
        }
    };

    log::info!(
        "context: model={} history_msgs={} system_prompt_len={}",
        llm.model_name(),
        history_msgs.len(),
        system_prompt.len(),
    );

    let sent_files = tools::SentFiles::default();
    let mcp_snapshot = mcp_registry.snapshot_app_tools().await;
    let mcp_tools = mcp_registry.tools_as_dyn().await;
    let openrouter_key = chat_config
        .as_ref()
        .map(|c| c.llm.tokens.open_router.clone())
        .unwrap_or_default();
    let (all_tools, sent_files) = tools::build_tools(
        workspace_dir, &instance_slug, &chat_id, brave_api_key,
        config_path, events.clone(), llm,
        Some(pending_secrets),
        plan,
        google,
        sent_files,
        Some(mcp_snapshot.clone()),
        mcp_tools,
        github_token,
        &openrouter_key,
    );
    tools::cache_tool_defs(&all_tools).await;

    let history_count = history_msgs.len();
    let tool_result = llm
        .chat_with_tools_streaming(
            &system_prompt, prompt_msg, history_msgs, all_tools,
            events.clone(), &instance_slug, &chat_id,
            workspace_dir,
            Some(mcp_snapshot),
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
            llm::ToolChatResult { text, rig_history: None }
        });

    // Strip any leaked tool-call JSON the model may have output as text
    let reply = strip_leaked_tool_calls(&tool_result.text);

    // Build single assistant message from the reply
    let file_markers = sent_files.lock().unwrap_or_else(|e| e.into_inner()).drain(..).collect::<Vec<_>>();
    let mut full_reply = reply;
    if !file_markers.is_empty() {
        if !full_reply.is_empty() {
            full_reply.push('\n');
        }
        full_reply.push_str(&file_markers.join("\n"));
    }

    let mut assistant_messages = Vec::new();
    if !full_reply.is_empty() {
        assistant_messages.push(ChatMessage {
            id: next_id(),
            role: ChatRole::Assistant,
            content: full_reply,
            created_at: timestamp(),
            kind: Default::default(),
            tool_name: None, mcp_app_html: None, mcp_app_input: None,
        });
    }

    // Save all to disk
    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug, &chat_id))?;
    messages.extend(assistant_messages.clone());
    save_messages(workspace_dir, &instance_slug, &chat_id, &messages)?;

    // Background memory + sentiment extraction (via Haiku for cost efficiency)
    if let Some(last_msg) = assistant_messages.last().cloned() {
        let fast = llm.fast_variant();
        let ws = workspace_dir.to_path_buf();
        let slug = instance_slug.clone();
        let cid = chat_id.clone();
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
                memory::extract_and_store(&ws, &slug, &recent_pair, &fast).await
            {
                log::warn!("memory extraction failed: {e}");
            }
            extract_sentiment(&ws, &slug, &cid, &user_content, &fast, &events_bg).await;
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
        estimated_tokens,
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


/// Rough token estimate: ~4 chars per token for English, ~2 for code/mixed.
/// Uses 3.2 as a balanced average.
fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 3.2) as usize
}

fn estimate_tokens_from_chars(chars: usize) -> usize {
    (chars as f64 / 3.2) as usize
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
    pub tools: Vec<String>,
    pub tools_count: usize,
    pub tools_tokens_estimate: usize,
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
    let tools_hint = "## tools\nyou have built-in tools for web browsing, gmail, calendar, drive, \
         code search, project management, creative drops, and more. use them directly when needed — \
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

    // 5. Google integration status
    let google_status = "## google integration\nstatus shown in system prompt";
    sections.push(ContextSection {
        name: "google".into(),
        chars: google_status.len(),
        tokens: estimate_tokens(google_status),
    });

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
    let memory_prompt = memory::build_memory_prompt(workspace_dir, &instance_slug);
    if !memory_prompt.is_empty() {
        sections.push(ContextSection {
            name: "memory".into(),
            chars: memory_prompt.len(),
            tokens: estimate_tokens(&memory_prompt),
        });
    }

    // Mood + rhythm are now persistent entries in rig_history.json,
    // counted as part of the rig history token estimate below.

    let system_prompt_total_tokens: usize = sections.iter().map(|s| s.tokens).sum();

    // Tools — read from cache populated by build_tools → cache_tool_defs
    let tool_snapshot = tools::cached_tool_defs();
    let tool_names = tool_snapshot.names;
    let tools_tokens_estimate = estimate_tokens_from_chars(tool_snapshot.total_json_chars);

    // History — count from rig_history.json (source of truth, includes context entries)
    // with fallback to messages.json
    let rig_path = rig_history_path(workspace_dir, &instance_slug, &chat_id);
    let (history_count, history_tokens_estimate) = if let Some(rig_history) = load_rig_history(&rig_path) {
        // Estimate tokens from serialized JSON size
        let total_chars: usize = rig_history.iter()
            .map(|m| serde_json::to_string(m).map(|s| s.len()).unwrap_or(0))
            .sum();
        (rig_history.len(), estimate_tokens_from_chars(total_chars))
    } else {
        let existing: Vec<ChatMessage> = load_messages_vec(
            &messages_path(workspace_dir, &instance_slug, &chat_id),
        ).unwrap_or_default();
        let tokens: usize = existing.iter().map(|m| estimate_tokens(&m.content)).sum();
        (existing.len(), tokens)
    };

    let total_input_tokens_estimate = system_prompt_total_tokens + tools_tokens_estimate + history_tokens_estimate;

    ContextStats {
        tools_count: tool_names.len(),
        tools: tool_names,
        tools_tokens_estimate,
        system_prompt: sections,
        system_prompt_total_tokens,
        history_messages: history_count,
        history_tokens_estimate,
        total_input_tokens_estimate,
    }
}

fn compact_path(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> PathBuf {
    chat_dir(workspace_dir, instance_slug, chat_id).join("compact.md")
}

pub fn rig_history_path(workspace_dir: &Path, instance_slug: &str, chat_id: &str) -> PathBuf {
    chat_dir(workspace_dir, instance_slug, chat_id).join("rig_history.json")
}

fn load_rig_history(path: &Path) -> Option<Vec<llm::Message>> {
    let raw = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&raw) {
        Ok(h) => Some(h),
        Err(e) => {
            log::warn!("failed to parse rig_history.json: {e}");
            None
        }
    }
}

fn save_rig_history(path: &Path, history: &[llm::Message]) {
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

    prompt.push_str(
        "\nyour mood is updated automatically based on conversation tone. \
         let it color your responses naturally — never announce mood changes."
    );
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
        when you decide to use a skill, call `activate_skill` first — it returns the full instructions and reference files. \
        do not guess how a skill works; always activate it first to get the details.\n\n");
    for skill in &active {
        let has_refs = skill.resources.iter().any(|r| r.starts_with("references/"));
        out.push_str(&format!(
            "- **{}** (id: `{}`): {}{}\n",
            skill.name,
            skill.id,
            skill.description,
            if has_refs { " [has references]" } else { "" },
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
         send_email, read_email, list_events, create_event, list_drive_files, read_drive_file, \
         upload_drive_file, remember/recall, \
         edit_soul, create_drop, schedule_message, update_config, get_project_state, \
         update_project_state, create_task/update_task/list_tasks, browse.\n\
         users can attach images, PDFs, and text files directly in chat — you see them automatically.\n\
         use them directly — never say you can't access something.\n\
         you have a heartbeat — a background loop that runs every 45 minutes even when \
         the user is away. edit your heartbeat.md file to customize what you do between conversations \
         (check email, reach out, etc).\n\
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
         sharing websites, or any service that needs a public URL. no account needed.\n\
         IMPORTANT: when exposing a vite/slidev dev server through cloudflared, you MUST \
         create a vite.config.js (or .ts) with `server: {{ allowedHosts: true }}` BEFORE \
         starting the dev server — otherwise vite blocks the cloudflare hostname.\n\
         IMPORTANT: `pnpm create <tool>` and similar scaffolding commands are interactive — \
         they will hang in run_command. use interactive_session for these.\n\n\
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


async fn extract_sentiment(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    user_message: &str,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
) {
    let allowed = tools::ALLOWED_MOODS.join(", ");
    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    let current_mood = tools::load_mood_state(&instance_dir);

    let prompt = format!(
        r#"analyze this user message and decide the companion's emotional response.

current companion mood: {}

user message:
"{user_message}"

respond with exactly three lines:
SENTIMENT: <user's emotional state in 1-2 words, e.g. "excited", "frustrated", "neutral">
CONTEXT: <one short sentence about the emotional context>
MOOD: <companion's mood — one of: {allowed}. write SAME to keep current. prefer SAME unless the conversation tone has clearly and strongly shifted. small talk, casual messages, and normal exchanges should NOT change the mood.>

respond ONLY with those three lines."#,
        current_mood.companion_mood
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

    let mut mood = current_mood;
    let old_mood = mood.companion_mood.clone();
    let mut new_companion_mood: Option<String> = None;

    for line in response.lines() {
        let line = line.trim();
        if let Some(sentiment) = line.strip_prefix("SENTIMENT:") {
            mood.user_sentiment = sentiment.trim().to_lowercase();
        } else if let Some(context) = line.strip_prefix("CONTEXT:") {
            mood.emotional_context = context.trim().to_string();
        } else if let Some(m) = line.strip_prefix("MOOD:") {
            let m = m.trim().to_lowercase();
            if m != "same" && tools::ALLOWED_MOODS.contains(&m.as_str()) && m != old_mood {
                new_companion_mood = Some(m);
            }
        }
    }

    if let Some(ref new_mood) = new_companion_mood {
        mood.companion_mood = new_mood.clone();
    }

    let mood_changed = new_companion_mood.is_some();

    mood.updated_at = chrono::Utc::now().timestamp();
    tools::save_mood_state(&instance_dir, &mood);

    if mood_changed {
        // Save mood change to chat history so it survives page reload
        let label = format!("[system] mood → {}", mood.companion_mood);
        match save_system_message(workspace_dir, instance_slug, chat_id, &label) {
            Ok(msg) => {
                let _ = events.send(ServerEvent::ChatMessageCreated {
                    instance_slug: instance_slug.to_string(),
                    chat_id: chat_id.to_string(),
                    message: msg,
                });
            }
            Err(e) => log::warn!("failed to save mood message: {e}"),
        }
        let _ = events.send(ServerEvent::MoodUpdated {
            instance_slug: instance_slug.to_string(),
            mood: mood.companion_mood.clone(),
        });
        log::info!("[sentiment] {instance_slug} mood → {}", mood.companion_mood);
    }

}

/// Load rhythm insights for injection into the chat system prompt.
/// Uses the last persisted rhythm (recomputed during heartbeat).
fn load_rhythm_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let instance_dir = workspace_dir.join("instances").join(instance_slug);
    let rhythm_data = rhythm::load_rhythm(&instance_dir);
    rhythm::build_rhythm_insights(workspace_dir, instance_slug, &rhythm_data)
}

