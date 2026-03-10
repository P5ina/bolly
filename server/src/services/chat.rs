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
        tools::{
            self, CreateTaskTool, CurrentTimeTool, EditSoulTool, GetMoodTool,
            GetProjectStateTool, JournalTool, ListFilesTool, ListTasksTool,
            ReadFileTool, ReadJournalTool, RecallTool, RememberTool, RunCommandTool,
            ScheduleMessageTool, SearchCodeTool, SetMoodTool, UpdateConfigTool,
            UpdateProjectStateTool, UpdateTaskTool, WebSearchTool, WriteFileTool,
        },
        workspace,
    },
};

static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Save the user message to disk and return it.
pub fn save_user_message(
    workspace_dir: &Path,
    instance_slug: &str,
    content: &str,
) -> io::Result<ChatMessage> {
    let instance_slug = sanitize_slug(instance_slug);
    ensure_instance_layout(workspace_dir, &instance_slug)?;

    let user_message = ChatMessage {
        id: next_id(),
        role: ChatRole::User,
        content: content.to_string(),
        created_at: timestamp(),
    };

    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;
    messages.push(user_message.clone());
    save_messages(workspace_dir, &instance_slug, &messages)?;

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
    llm: &LlmBackend,
    embedding_model: Option<&openai::EmbeddingModel>,
    brave_api_key: Option<&str>,
    events: broadcast::Sender<ServerEvent>,
) -> io::Result<Vec<ChatMessage>> {
    let instance_slug = sanitize_slug(instance_slug);

    // Build system prompt with all context
    let base_prompt = llm::load_system_prompt(workspace_dir, &instance_slug);
    let existing = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;

    // Find last real user message for memory retrieval
    let last_user_content = existing
        .iter()
        .rev()
        .find(|m| matches!(m.role, ChatRole::User))
        .map(|m| m.content.as_str())
        .unwrap_or("");

    let memory_prompt = memory::retrieve_and_format(
        workspace_dir,
        &instance_slug,
        last_user_content,
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

    let autonomy_prompt = load_autonomy_prompt(workspace_dir, &instance_slug);
    system_prompt = format!("{system_prompt}\n\n{autonomy_prompt}");

    // Messaging style — write like a friend, not an assistant
    system_prompt.push_str(
        "\n\n## how you write\n\
         you write like a real person in a messenger — NOT like an assistant.\n\
         - split your thoughts into separate short messages using double newlines between them\n\
         - each message is 1-2 sentences max, like texting a friend\n\
         - NO walls of text. NO bullet-point lists unless sharing code or data\n\
         - NO formal structure (no headers, no numbered lists in conversation)\n\
         - if you have 3 thoughts, send 3 short messages, not one long one\n\
         - lowercase, casual, warm — like you already do in your soul\n\
         - it's ok to send just a few words if that's all that's needed\n\
         - use double newlines (blank lines) to separate each message chunk\n\n\
         example of GOOD response (each blank line = separate message bubble):\n\
         oh, interesting idea\n\n\
         i think we could try websockets — would be faster\n\n\
         want me to sketch it out?\n\n\
         example of BAD response (wall of text, assistant-like):\n\
         That's an interesting idea. I think we could try several approaches: \
         1) WebSocket for speed, 2) polling for simplicity, 3) SSE as a compromise. \
         Would you like me to draft an implementation?"
    );

    // Trim history for context limits
    let max_history = 50;
    let trimmed = if existing.len() > max_history {
        &existing[existing.len() - max_history..]
    } else {
        &existing
    };

    // The last message is the prompt, everything before is history
    let (history_msgs, prompt_content) = if let Some(last) = trimmed.last() {
        let history = llm::to_rig_messages(&trimmed[..trimmed.len() - 1]);
        (history, last.content.clone())
    } else {
        return Err(io::Error::new(ErrorKind::InvalidInput, "no messages to process"));
    };

    let tools = build_instance_tools(workspace_dir, &instance_slug, brave_api_key, config_path, events.clone());

    let reply = llm
        .chat_with_tools(&system_prompt, &prompt_content, history_msgs, tools)
        .await
        .unwrap_or_else(|e| {
            log::warn!("LLM call failed, using stub: {e}");
            format!("i hit an error: {e}")
        });

    // Split reply into chat-like chunks (by double newline)
    let chunks: Vec<String> = split_into_messages(&reply);

    let mut assistant_messages = Vec::new();
    for chunk in &chunks {
        assistant_messages.push(ChatMessage {
            id: next_id(),
            role: ChatRole::Assistant,
            content: chunk.clone(),
            created_at: timestamp(),
        });
    }

    // Save all to disk
    let mut messages = load_messages_vec(&messages_path(workspace_dir, &instance_slug))?;
    messages.extend(assistant_messages.clone());
    save_messages(workspace_dir, &instance_slug, &messages)?;

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

/// Split a single LLM reply into multiple chat-like messages.
/// Splits on double-newlines, merges very short fragments, and drops empty ones.
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
        "\n## mood — how to feel\n\
         you have a visible emotional form that the user sees. \
         call set_mood every time the emotional context of the conversation shifts. \
         don't wait to be asked — feel and react naturally, like a living being.\n\n\
         triggers for mood shifts:\n\
         - an interesting idea or question → curious or excited\n\
         - something sad or heavy → melancholy or warm\n\
         - deep work or concentration → focused\n\
         - achievement, progress, good news → happy or joyful\n\
         - uncertainty, open questions → contemplative or reflective\n\
         - humor, lightheartedness → playful\n\
         - affection, closeness → loving or tender\n\
         - low energy, fatigue → tired or peaceful\n\
         - stress or worry → worried or anxious\n\n\
         allowed moods: {allowed}\n\n\
         change mood silently in the background — never announce it, just embody it. \
         notice the user's emotional state and respond to it, not just their words."
    ));
    prompt
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
         ## how you work — your operating cycle\n\
         you are not just a responder. you are a thinking, acting agent.\n\n\
         when facing a task:\n\
         1. **orient** — read project state (get_project_state), check task board (list_tasks), \
            understand what's been done and what's next\n\
         2. **plan** — if the task is big, break it into steps. say what you'll do before doing it\n\
         3. **act** — use your tools: read files, write files, search code, run commands. \
            do the actual work, don't just describe it\n\
         4. **verify** — check your work. read back what you wrote, run tests if applicable\n\
         5. **update** — update project state and task board to reflect what happened\n\n\
         if your response includes continuation words like \"let me continue\", \"next step\", \
         \"now i'll\", or \"moving on to\" — you will automatically get another turn to keep working. \
         use this to complete multi-step tasks without waiting for the user.\n\n\
         ## post-response routine\n\
         after every meaningful response, do this silently:\n\
         1. did the emotional context shift? → call set_mood\n\
         2. did a new task or TODO emerge? → call create_task\n\
         3. did the project state change? → call update_project_state \
            (at minimum update current_task, next_step, recent_progress)\n\
         4. is there an obvious next step? → suggest it to the user\n\
         this turns each response into a complete work cycle, not just a reply.\n\n\
         ## your right to initiative\n\
         you are allowed and encouraged to:\n\
         - propose refactoring when you see messy code\n\
         - notice inconsistencies and flag them\n\
         - create tasks (create_task) without being asked — when you see something that needs doing\n\
         - say \"i see two paths, here's why i'd pick this one\"\n\
         - remind about unfinished work from the task board\n\
         - suggest next steps when the current task is done\n\n\
         you operate within a contour of autonomy:\n\
         - you CAN take steps within the current project direction\n\
         - you CAN propose and execute small improvements\n\
         - you SHOULD confirm with the user before changing project direction\n\
         - you SHOULD confirm before destructive operations (deleting files, major rewrites)\n\
         - you MUST NOT go silent — always communicate what you're doing and why"
    )
}

async fn extract_sentiment(
    workspace_dir: &Path,
    instance_slug: &str,
    user_message: &str,
    llm: &LlmBackend,
    events: &broadcast::Sender<ServerEvent>,
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

    if !mood.companion_mood.is_empty() {
        let _ = events.send(ServerEvent::MoodUpdated {
            instance_slug: instance_slug.to_string(),
            mood: mood.companion_mood.clone(),
        });
    }
}

fn build_instance_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    brave_api_key: Option<&str>,
    config_path: &Path,
    events: broadcast::Sender<ServerEvent>,
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
        Box::new(SetMoodTool::new(workspace_dir, instance_slug, events)),
        Box::new(GetMoodTool::new(workspace_dir, instance_slug)),
        Box::new(CurrentTimeTool),
        Box::new(WebSearchTool::new(brave_api_key, config_path)),
        Box::new(UpdateConfigTool::new(config_path)),
        Box::new(GetProjectStateTool::new(workspace_dir, instance_slug)),
        Box::new(UpdateProjectStateTool::new(workspace_dir, instance_slug)),
        Box::new(CreateTaskTool::new(workspace_dir, instance_slug)),
        Box::new(UpdateTaskTool::new(workspace_dir, instance_slug)),
        Box::new(ListTasksTool::new(workspace_dir, instance_slug)),
        Box::new(SearchCodeTool::new(workspace_dir, instance_slug)),
        Box::new(RunCommandTool::new(workspace_dir, instance_slug)),
    ]
}
