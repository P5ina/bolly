use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use rig::providers::openai;

use rig::tool::ToolDyn;

use crate::{
    domain::chat::{ChatMessage, ChatRequest, ChatResponse, ChatRole},
    domain::instance::InstanceSummary,
    services::{
        llm::{self, LlmBackend},
        memory,
        tools::{
            self, CurrentTimeTool, EditSoulTool, GetMoodTool, JournalTool, ListFilesTool,
            ReadFileTool, ReadJournalTool, RecallTool, RememberTool, ScheduleMessageTool,
            SetApiKeyTool, SetMoodTool, WebSearchTool, WriteFileTool,
        },
        workspace,
    },
};

static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(0);


pub async fn append_chat_turn(
    workspace_dir: &Path,
    config_path: &Path,
    request: ChatRequest,
    llm: Option<&LlmBackend>,
    embedding_model: Option<&openai::EmbeddingModel>,
    brave_api_key: Option<&str>,
) -> io::Result<ChatResponse> {
    let instance_slug = sanitize_slug(&request.instance_slug);
    if instance_slug.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "instance_slug cannot be empty",
        ));
    }

    let content = request.content.trim().to_string();
    if content.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "content cannot be empty",
        ));
    }

    ensure_instance_layout(workspace_dir, &instance_slug)?;

    let user_message = ChatMessage {
        id: next_id(),
        role: ChatRole::User,
        content: content.clone(),
        created_at: timestamp(),
    };

    // Generate reply via LLM or fall back to stub
    let reply = if let Some(backend) = llm {
        let base_prompt = llm::load_system_prompt(workspace_dir, &instance_slug);
        let memory_prompt = memory::retrieve_and_format(
            workspace_dir,
            &instance_slug,
            &content,
            embedding_model,
        )
        .await;
        let journal_prompt = load_recent_journal(workspace_dir, &instance_slug);
        let mood_prompt = load_mood_prompt(workspace_dir, &instance_slug);
        let mut system_prompt = base_prompt;
        if !memory_prompt.is_empty() {
            system_prompt = format!("{system_prompt}\n\n{memory_prompt}");
        }
        if !journal_prompt.is_empty() {
            system_prompt = format!("{system_prompt}\n\n{journal_prompt}");
        }
        if !mood_prompt.is_empty() {
            system_prompt = format!("{system_prompt}\n\n{mood_prompt}");
        }

        let existing = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;
        // Keep only the most recent messages to stay within context limits.
        // Older context is preserved via the memory system (facts extraction).
        let max_history = 50;
        let trimmed = if existing.len() > max_history {
            &existing[existing.len() - max_history..]
        } else {
            &existing
        };
        let history = llm::to_rig_messages(trimmed);

        let tools = build_instance_tools(workspace_dir, &instance_slug, brave_api_key, config_path);

        backend
            .chat_with_tools(&system_prompt, &content, history, tools)
            .await
            .unwrap_or_else(|e| {
                log::warn!("LLM call failed, using stub: {e}");
                stub_reply(&instance_slug, &content)
            })
    } else {
        stub_reply(&instance_slug, &content)
    };

    let assistant_message = ChatMessage {
        id: next_id(),
        role: ChatRole::Assistant,
        content: reply,
        created_at: timestamp(),
    };

    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;
    messages.push(user_message.clone());
    messages.push(assistant_message.clone());
    save_messages(workspace_dir, &instance_slug, &messages)?;

    // Update last_interaction timestamp
    {
        let instance_dir = workspace_dir.join("instances").join(&instance_slug);
        let mut mood = tools::load_mood_state(&instance_dir);
        mood.last_interaction = chrono::Utc::now().timestamp();
        tools::save_mood_state(&instance_dir, &mood);
    }

    // Spawn background memory extraction + sentiment analysis
    if let Some(backend) = llm {
        let backend = backend.clone();
        let emb = embedding_model.cloned();
        let ws = workspace_dir.to_path_buf();
        let slug = instance_slug.clone();
        let user_content = content.clone();
        let recent = vec![user_message.clone(), assistant_message.clone()];
        tokio::spawn(async move {
            if let Err(e) =
                memory::extract_and_store(&ws, &slug, &recent, &backend, emb.as_ref()).await
            {
                log::warn!("memory extraction failed: {e}");
            }
            // Extract sentiment from the user's message
            extract_sentiment(&ws, &slug, &user_content, &backend).await;
        });
    }

    Ok(ChatResponse {
        instance_slug,
        messages: vec![user_message, assistant_message],
    })
}

pub fn load_messages(workspace_dir: &Path, instance_slug: &str) -> io::Result<ChatResponse> {
    let instance_slug = sanitize_slug(instance_slug);
    let messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;

    Ok(ChatResponse {
        instance_slug,
        messages,
    })
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

fn messages_path(workspace_dir: &Path, instance_slug: &str) -> PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("chat")
        .join("messages.json")
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
    messages: &[ChatMessage],
) -> io::Result<()> {
    let path = messages_path(workspace_dir, instance_slug);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let body = serde_json::to_string_pretty(messages)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
    fs::write(path, body)
}

fn stub_reply(instance_slug: &str, content: &str) -> String {
    format!(
        "i heard you, {instance_slug}. i just can't think yet — no language model is configured. \
         you said: \"{content}\""
    )
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

    // Take last 3 days of journal entries
    let recent: Vec<_> = files.into_iter().rev().take(3).collect();
    if recent.is_empty() {
        return String::new();
    }

    let mut prompt = String::from(
        "## your journal\nthese are your private thoughts from recent days. \
         they're yours — the user doesn't see them. use them to maintain continuity.\n\n",
    );

    for entry in recent.into_iter().rev() {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            // Limit each entry to avoid blowing context
            let truncated: String = content.chars().take(1000).collect();
            prompt.push_str(&truncated);
            if content.len() > 1000 {
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

    let has_mood = !mood.companion_mood.is_empty();
    let has_sentiment = !mood.user_sentiment.is_empty();
    let has_context = !mood.emotional_context.is_empty();

    if !has_mood && !has_sentiment && !has_context {
        return String::new();
    }

    let mut prompt = String::from("## emotional state\n");
    if has_mood {
        prompt.push_str(&format!("your current mood: {}\n", mood.companion_mood));
    }
    if has_sentiment {
        prompt.push_str(&format!(
            "last observed user sentiment: {}\n",
            mood.user_sentiment
        ));
    }
    if has_context {
        prompt.push_str(&format!("{}\n", mood.emotional_context));
    }
    prompt.push_str(
        "\nlet your mood color your tone subtly — don't announce it, just embody it. \
         notice the user's emotional state and respond to it, not just their words.",
    );
    prompt
}

async fn extract_sentiment(
    workspace_dir: &Path,
    instance_slug: &str,
    user_message: &str,
    llm: &LlmBackend,
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
}

fn build_instance_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    brave_api_key: Option<&str>,
    config_path: &Path,
) -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(EditSoulTool::new(workspace_dir, instance_slug)),
        Box::new(ReadFileTool::new(workspace_dir, instance_slug)),
        Box::new(WriteFileTool::new(workspace_dir, instance_slug)),
        Box::new(ListFilesTool::new(workspace_dir, instance_slug)),
        Box::new(RememberTool::new(workspace_dir, instance_slug)),
        Box::new(RecallTool::new(workspace_dir, instance_slug)),
        Box::new(JournalTool::new(workspace_dir, instance_slug)),
        Box::new(ReadJournalTool::new(workspace_dir, instance_slug)),
        Box::new(ScheduleMessageTool::new(workspace_dir, instance_slug)),
        Box::new(SetMoodTool::new(workspace_dir, instance_slug)),
        Box::new(GetMoodTool::new(workspace_dir, instance_slug)),
        Box::new(CurrentTimeTool),
        Box::new(WebSearchTool::new(brave_api_key, config_path)),
        Box::new(SetApiKeyTool::new(config_path)),
    ]
}
