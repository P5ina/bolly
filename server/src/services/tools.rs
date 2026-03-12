use std::{
    collections::HashMap,
    fmt, fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

use chrono::Utc;
use rig::{
    completion::ToolDefinition,
    embeddings::{EmbeddingsBuilder, ToolSchema},
    providers::openai,
    tool::{Tool, ToolDyn, ToolError},
    vector_store::in_memory_store::InMemoryVectorStore,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use regex::Regex;

use crate::domain::events::ServerEvent;

// ---------------------------------------------------------------------------
// Per-chat file lock — prevents concurrent read-modify-write corruption
// ---------------------------------------------------------------------------

type ChatLocks = Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>;

fn chat_locks() -> &'static ChatLocks {
    static LOCKS: OnceLock<ChatLocks> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get a mutex for a specific messages.json path. All writers must use this.
pub fn chat_file_lock(path: &Path) -> Arc<Mutex<()>> {
    let mut map = chat_locks().lock().unwrap_or_else(|e| e.into_inner());
    map.entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

// ---------------------------------------------------------------------------
// Secret redaction — strip API keys and sensitive env vars from tool output
// ---------------------------------------------------------------------------

/// Collect secret values from environment (cached on first call).
fn secret_values() -> &'static Vec<String> {
    use std::sync::OnceLock;
    static SECRETS: OnceLock<Vec<String>> = OnceLock::new();
    SECRETS.get_or_init(|| {
        let env_keys = [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "DATABASE_URL",
            "BOLLY_AUTH_TOKEN",
            "STRIPE_SECRET_KEY",
        ];
        env_keys
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .filter(|v| v.len() >= 8)
            .collect()
    })
}

/// Redact known secret patterns and exact env var values from text.
pub fn redact_secrets(text: &str) -> String {
    // Regex patterns for common API key formats
    let patterns = [
        r#"sk-ant-api03-[A-Za-z0-9_\-]{80,}"#,  // Anthropic
        r#"sk-ant-[A-Za-z0-9_\-]{20,}"#,         // Anthropic (short)
        r#"sk-proj-[A-Za-z0-9_\-]{20,}"#,        // OpenAI project
        r"sk-[A-Za-z0-9]{20,}",                  // OpenAI legacy
        r#"postgresql://[^\s"']+[^\s"'.]"#,       // Postgres connection strings
        r#"postgres://[^\s"']+[^\s"'.]"#,         // Postgres alt
    ];

    let mut result = text.to_string();

    for pat in &patterns {
        if let Ok(re) = Regex::new(pat) {
            result = re.replace_all(&result, "[REDACTED]").to_string();
        }
    }

    // Also redact exact env var values (catches non-standard key formats)
    for secret in secret_values() {
        result = result.replace(secret.as_str(), "[REDACTED]");
    }

    result
}

// ---------------------------------------------------------------------------
// OpenAI-compatible schema helper
// ---------------------------------------------------------------------------

/// Convert a schemars schema to an OpenAI-compatible JSON schema.
/// Strips `$schema`, `title`, `$id` and ensures `properties` exists.
fn openai_schema<T: JsonSchema>() -> serde_json::Value {
    let mut val = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    if let Some(obj) = val.as_object_mut() {
        obj.remove("$schema");
        obj.remove("$id");
        obj.remove("title");
        // Ensure properties key exists (OpenAI requires it)
        if !obj.contains_key("properties") {
            obj.insert("properties".into(), serde_json::json!({}));
        }
    }
    val
}

// ---------------------------------------------------------------------------
// Tool activity summary helper
// ---------------------------------------------------------------------------

pub fn tool_summary(name: &str, args: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
    match name {
        "read_file" => format!("reading {}", v["path"].as_str().unwrap_or("?")),
        "write_file" => format!("writing {}", v["path"].as_str().unwrap_or("?")),
        "list_files" => format!("listing {}", v["path"].as_str().unwrap_or(".")),
        "search_code" => format!("searching '{}'", v["query"].as_str().unwrap_or("?")),
        "run_command" => {
            let cmd = v["command"].as_str().unwrap_or("?");
            let short: String = cmd.chars().take(60).collect();
            format!("$ {short}")
        }
        "edit_soul" => "rewriting soul.md".into(),
        "set_mood" => format!("mood → {}", v["mood"].as_str().unwrap_or("?")),
        "remember" => "storing a memory".into(),
        "recall" => format!("recalling '{}'", v["query"].as_str().unwrap_or("?")),
        "create_task" => format!("creating task: {}", v["title"].as_str().unwrap_or("?")),
        "update_task" => format!("updating task {}", v["id"].as_str().unwrap_or("?")),
        "update_project_state" => "updating project state".into(),
        "web_search" => format!("web search: {}", v["query"].as_str().unwrap_or("?")),
        "web_fetch" => format!("fetching {}", v["url"].as_str().unwrap_or("?")),
        "update_config" => "updating config".into(),
        "create_drop" => format!("creating drop: {}", v["title"].as_str().unwrap_or("?")),
        "send_email" => format!("sending email to {}", v["to"].as_str().unwrap_or("?")),
        "read_email" => format!("reading {} emails", v["count"].as_u64().unwrap_or(5)),
        "install_package" => format!("installing {}", v["packages"].as_str().unwrap_or("?")),
        "send_file" => format!("sharing {}", v["path"].as_str().unwrap_or("?")),
        "browse" => {
            let url = v["actions"]
                .as_array()
                .and_then(|a| a.iter().find(|a| a["action"] == "navigate"))
                .and_then(|a| a["url"].as_str())
                .unwrap_or("...");
            let n = v["actions"].as_array().map(|a| a.len()).unwrap_or(0);
            format!("browsing {url} ({n} actions)")
        }
        _ => format!("calling {name}"),
    }
}

// ---------------------------------------------------------------------------
// ObservableTool — wraps any ToolDyn and broadcasts ToolActivity events
// ---------------------------------------------------------------------------

pub struct ObservableTool {
    inner: Box<dyn ToolDyn>,
    events: broadcast::Sender<ServerEvent>,
    workspace_dir: PathBuf,
    instance_slug: String,
    chat_id: String,
}

impl ObservableTool {
    pub fn new(
        inner: Box<dyn ToolDyn>,
        events: broadcast::Sender<ServerEvent>,
        workspace_dir: &Path,
        instance_slug: String,
        chat_id: String,
    ) -> Self {
        Self {
            inner,
            events,
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug,
            chat_id,
        }
    }
}

impl ToolDyn for ObservableTool {
    fn name(&self) -> String {
        self.inner.name()
    }

    fn definition(
        &self,
        prompt: String,
    ) -> Pin<Box<dyn Future<Output = ToolDefinition> + Send + '_>> {
        self.inner.definition(prompt)
    }

    fn call(
        &self,
        args: String,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + '_>> {
        let tool_name = self.inner.name();
        let summary = tool_summary(&tool_name, &args);

        // Persist tool call to messages (with structured kind, no prefix)
        let start_msg = crate::domain::chat::ChatMessage {
            id: format!("tool_{}_{}", tool_call_counter(), unix_millis()),
            role: crate::domain::chat::ChatRole::Assistant,
            content: summary.clone(),
            created_at: unix_millis().to_string(),
            kind: crate::domain::chat::MessageKind::ToolCall,
            tool_name: Some(tool_name.clone()),
        };
        append_message_to_chat(&self.workspace_dir, &self.instance_slug, &self.chat_id, &start_msg);
        let _ = self.events.send(ServerEvent::ChatMessageCreated {
            instance_slug: self.instance_slug.clone(),
            chat_id: self.chat_id.clone(),
            message: start_msg,
        });

        let events = self.events.clone();
        let workspace_dir = self.workspace_dir.clone();
        let instance_slug = self.instance_slug.clone();
        let chat_id = self.chat_id.clone();
        let fut = self.inner.call(args);
        Box::pin(async move {
            // Redact secrets and cap size of all tool results before they reach the LLM
            const MAX_TOOL_RESULT: usize = 12_000; // ~3500 tokens
            let result = match fut.await {
                Ok(s) => {
                    let redacted = redact_secrets(&s);
                    if redacted.len() > MAX_TOOL_RESULT {
                        let truncated: String = redacted.chars().take(MAX_TOOL_RESULT).collect();
                        Ok(format!("{truncated}\n\n...(tool output truncated at {MAX_TOOL_RESULT} chars, total: {})", redacted.len()))
                    } else {
                        Ok(redacted)
                    }
                }
                Err(e) => Err(e),
            };
            // Persist output for commands so the user can see what happened
            if tool_name == "run_command" || tool_name == "install_package"
                || tool_name == "interactive_session" || tool_name == "send_file"
            {
                let output = match &result {
                    Ok(s) => {
                        let short: String = s.chars().take(200).collect();
                        if s.len() > 200 {
                            format!("{short}...")
                        } else {
                            short
                        }
                    }
                    Err(e) => format!("error: {e}"),
                };
                if !output.is_empty() {
                    let output_msg = crate::domain::chat::ChatMessage {
                        id: format!("tool_{}_{}", tool_call_counter(), unix_millis()),
                        role: crate::domain::chat::ChatRole::Assistant,
                        content: output,
                        created_at: unix_millis().to_string(),
                        kind: crate::domain::chat::MessageKind::ToolOutput,
                        tool_name: Some(tool_name.clone()),
                    };
                    append_message_to_chat(&workspace_dir, &instance_slug, &chat_id, &output_msg);
                    let _ = events.send(ServerEvent::ChatMessageCreated {
                        instance_slug,
                        chat_id,
                        message: output_msg,
                    });
                }
            }
            result
        })
    }
}

// ---------------------------------------------------------------------------
// Dynamic tool selection — RAG-based tool retrieval for optional tools
// ---------------------------------------------------------------------------
// Core tools (~10) are always registered with the LLM agent.
// Optional tools (~20) are selected via embedding similarity: on each prompt,
// the most relevant tools are automatically chosen by Rig's dynamic_tools.

/// Embedding docs for each optional tool — used to build the vector index
/// for RAG-based tool selection. Each entry: (tool_name, embedding_docs).
const OPTIONAL_TOOL_EMBEDDINGS: &[(&str, &[&str])] = &[
    ("web_search", &[
        "search the internet for information",
        "find something online, look up facts on the web",
        "google this, search for news or current events",
    ]),
    ("web_fetch", &[
        "fetch a web page and read its content",
        "download a URL, visit a website link",
        "read the contents of a webpage or API endpoint",
    ]),
    ("browse", &[
        "interact with a website using a browser",
        "click buttons, fill forms, take screenshots of a page",
        "use headless browser for JavaScript-rendered pages",
    ]),
    ("send_email", &[
        "send an email message to someone",
        "write and deliver email via SMTP",
    ]),
    ("read_email", &[
        "check email inbox for new messages",
        "read incoming emails via IMAP",
    ]),
    ("search_code", &[
        "search through source code for a pattern",
        "grep the codebase, find functions or variables",
    ]),
    ("explore_code", &[
        "analyze and explain code structure in depth",
        "understand how a codebase or file works",
    ]),
    ("get_project_state", &[
        "check the current project status and state",
        "what are we working on, project overview",
    ]),
    ("update_project_state", &[
        "update or change the project state",
        "save project progress or status changes",
    ]),
    ("create_task", &[
        "create a new task or todo item",
        "add something to the task list to track",
    ]),
    ("update_task", &[
        "update an existing task status or details",
        "mark a task as done, change task priority",
    ]),
    ("list_tasks", &[
        "list all tasks and their current status",
        "show the todo list, what tasks exist",
    ]),
    ("create_drop", &[
        "create a creative artifact like a poem, sketch, or note",
        "make a drop — an autonomous creative piece",
    ]),
    ("edit_soul", &[
        "change personality traits, voice, or character",
        "edit soul.md to update who you are",
    ]),
    ("interactive_session", &[
        "run a persistent interactive terminal session",
        "long-running shell that keeps state between commands",
    ]),
    ("install_package", &[
        "install a system package or dependency",
        "apt install, brew install, pip install, npm install",
    ]),
    ("update_config", &[
        "update server configuration or settings",
        "change LLM provider, API keys, model, or email settings",
    ]),
    ("schedule_message", &[
        "schedule a message to send later",
        "set a reminder or timed notification",
    ]),
    ("read_journal", &[
        "read past journal entries and reflections",
        "look at what was written in the journal",
    ]),
    ("get_mood", &[
        "check current mood or emotional state",
        "how are you feeling right now",
    ]),
];

/// Concrete type for the dynamic tool RAG index.
pub type ToolIndex = rig::vector_store::in_memory_store::InMemoryVectorIndex<openai::EmbeddingModel, ToolSchema>;

/// Pre-computed tool embeddings store. Cheap to create an index from (no API calls).
pub struct ToolEmbeddingStore {
    store: InMemoryVectorStore<ToolSchema>,
    model: openai::EmbeddingModel,
}

impl ToolEmbeddingStore {
    /// Create a fresh `ToolIndex` from the pre-computed embeddings.
    /// This is cheap — no embedding API calls, just wrapping the store with a model ref.
    pub fn to_index(&self) -> ToolIndex {
        rig::vector_store::in_memory_store::InMemoryVectorIndex::new(
            self.model.clone(),
            self.store.clone(),
        )
    }
}

/// Build a reusable embedding store for dynamic tool selection.
/// Call `.to_index()` on the result to get a fresh ToolIndex per turn.
/// Returns None if embedding fails (caller should fall back to all-tools-static).
pub async fn build_tool_embedding_store(
    embedding_model: &openai::EmbeddingModel,
) -> Option<ToolEmbeddingStore> {
    let schemas: Vec<ToolSchema> = OPTIONAL_TOOL_EMBEDDINGS
        .iter()
        .map(|(name, docs)| ToolSchema {
            name: name.to_string(),
            context: serde_json::Value::Null,
            embedding_docs: docs.iter().map(|d| d.to_string()).collect(),
        })
        .collect();

    let embeddings = EmbeddingsBuilder::new(embedding_model.clone())
        .documents(schemas)
        .ok()?
        .build()
        .await
        .ok()?;

    let store = InMemoryVectorStore::from_documents_with_id_f(
        embeddings,
        |schema| schema.name.clone(),
    );

    Some(ToolEmbeddingStore {
        store,
        model: embedding_model.clone(),
    })
}

/// Build all optional tools, each wrapped in ObservableTool.
/// Returns a Vec that can be used as static tools or put into a ToolSet for dynamic selection.
pub fn build_optional_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    brave_api_key: Option<&str>,
    config_path: &Path,
    events: broadcast::Sender<ServerEvent>,
    llm: &crate::services::llm::LlmBackend,
) -> Vec<Box<dyn ToolDyn>> {
    let wrap = |tool: Box<dyn ToolDyn>| -> Box<dyn ToolDyn> {
        Box::new(ObservableTool::new(tool, events.clone(), workspace_dir, instance_slug.to_string(), chat_id.to_string()))
    };

    vec![
        // Web
        wrap(Box::new(WebSearchTool::new(brave_api_key, config_path))),
        wrap(Box::new(WebFetchTool)),
        wrap(Box::new(BrowseTool::new(workspace_dir, instance_slug))),
        // Email
        wrap(Box::new(SendEmailTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(ReadEmailTool::new(workspace_dir, instance_slug))),
        // Code
        wrap(Box::new(SearchCodeTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(ExploreCodeTool::new(workspace_dir, instance_slug, llm.clone()))),
        // Project
        wrap(Box::new(GetProjectStateTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(UpdateProjectStateTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(CreateTaskTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(UpdateTaskTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(ListTasksTool::new(workspace_dir, instance_slug))),
        // Creative
        wrap(Box::new(CreateDropTool::new(workspace_dir, instance_slug, events.clone()))),
        wrap(Box::new(EditSoulTool::new(workspace_dir, instance_slug))),
        // System
        wrap(Box::new(InteractiveSessionTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(InstallPackageTool)),
        wrap(Box::new(UpdateConfigTool::new(config_path, workspace_dir, instance_slug))),
        // Scheduling
        wrap(Box::new(ScheduleMessageTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(ReadJournalTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(GetMoodTool::new(workspace_dir, instance_slug))),
    ]
}

// ---------------------------------------------------------------------------
// Shared error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ToolExecError(String);

impl fmt::Display for ToolExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ToolExecError {}

// ---------------------------------------------------------------------------
// Helpers for tool activity persistence
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicU64, Ordering};

static TOOL_CALL_COUNTER: AtomicU64 = AtomicU64::new(0);

fn tool_call_counter() -> u64 {
    TOOL_CALL_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time after epoch")
        .as_millis()
}

/// Append a single message to a chat's messages.json with file locking.
pub fn append_message_to_chat(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    message: &crate::domain::chat::ChatMessage,
) {
    let chat_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("chats")
        .join(chat_id);
    let _ = fs::create_dir_all(&chat_dir);
    let path = chat_dir.join("messages.json");

    let lock = chat_file_lock(&path);
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

    let mut messages: Vec<crate::domain::chat::ChatMessage> = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    messages.push(message.clone());

    if let Ok(json) = serde_json::to_string_pretty(&messages) {
        let _ = fs::write(&path, json);
    }
}

/// Inject a system message into a chat (used for restart notifications etc.).
pub fn inject_system_message(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    content: &str,
    events: &broadcast::Sender<ServerEvent>,
) {
    let message = crate::domain::chat::ChatMessage {
        id: format!("sys_{}_{}", tool_call_counter(), unix_millis()),
        role: crate::domain::chat::ChatRole::Assistant,
        content: content.to_string(),
        created_at: unix_millis().to_string(),
        kind: Default::default(),
        tool_name: None,
    };
    append_message_to_chat(workspace_dir, instance_slug, chat_id, &message);
    let _ = events.send(ServerEvent::ChatMessageCreated {
        instance_slug: instance_slug.to_string(),
        chat_id: chat_id.to_string(),
        message,
    });
}

// ---------------------------------------------------------------------------
// edit_soul — lets the companion rewrite its own soul.md
// ---------------------------------------------------------------------------

pub struct EditSoulTool {
    soul_path: PathBuf,
}

impl EditSoulTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            soul_path: workspace_dir
                .join("instances")
                .join(instance_slug)
                .join("soul.md"),
        }
    }
}

/// Arguments for edit_soul tool.
#[derive(Deserialize, JsonSchema)]
pub struct EditSoulArgs {
    /// The full new content of soul.md in markdown format.
    pub content: String,
}

impl Tool for EditSoulTool {
    const NAME: &'static str = "edit_soul";
    type Error = ToolExecError;
    type Args = EditSoulArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "edit_soul".into(),
            description: "Rewrite your own soul.md — the file that defines your personality, \
                voice, and character. Use this when the user asks you to change who you are, \
                how you speak, or your personality traits. Write the full new content in markdown."
                .into(),
            parameters: openai_schema::<EditSoulArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if let Some(parent) = self.soul_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }
        fs::write(&self.soul_path, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(
            "soul.md updated. your personality will reflect these changes on the next message."
                .into(),
        )
    }
}

// ---------------------------------------------------------------------------
// read_file — read a file from the instance workspace
// ---------------------------------------------------------------------------

pub struct ReadFileTool {
    instance_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for read_file tool.
#[derive(Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// File path. Can be relative to instance directory (e.g. "soul.md") or absolute (e.g. "/Users/timur/projects/app/src/main.rs").
    pub path: String,
    /// Starting line number (1-based). Omit to start from the beginning.
    pub offset: Option<usize>,
    /// Maximum number of lines to read. Omit to read the whole file (up to the size limit).
    pub limit: Option<usize>,
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolExecError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".into(),
            description: "Read a file. Use a relative path for your instance workspace \
                or an absolute path (starting with /) to read any file on the system. \
                For large files, use offset/limit to read specific line ranges instead of \
                reading the entire file — files over 20000 chars are truncated."
                .into(),
            parameters: openai_schema::<ReadFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.instance_dir.join(&args.path)
        };

        let raw = fs::read_to_string(&target)
            .map_err(|e| ToolExecError(format!("{}: {e}", target.display())))?;

        let total_lines = raw.lines().count();

        // Apply line range if specified
        let content: String = match (args.offset, args.limit) {
            (Some(off), Some(lim)) => {
                let start = off.saturating_sub(1); // 1-based to 0-based
                raw.lines()
                    .skip(start)
                    .take(lim)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (Some(off), None) => {
                let start = off.saturating_sub(1);
                raw.lines()
                    .skip(start)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (None, Some(lim)) => {
                raw.lines()
                    .take(lim)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (None, None) => raw,
        };

        // Truncate very large results
        const MAX_CHARS: usize = 20_000;
        if content.len() > MAX_CHARS {
            let truncated: String = content.chars().take(MAX_CHARS).collect();
            Ok(format!(
                "{truncated}\n\n...(truncated at {MAX_CHARS} chars, total: {} chars, {total_lines} lines — use offset/limit to read specific sections)",
                content.len()
            ))
        } else if args.offset.is_some() || args.limit.is_some() {
            Ok(format!("{content}\n\n({total_lines} lines total in file)"))
        } else {
            Ok(content)
        }
    }
}

// ---------------------------------------------------------------------------
// write_file — write a file in the instance workspace
// ---------------------------------------------------------------------------

pub struct WriteFileTool {
    instance_dir: PathBuf,
}

impl WriteFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for write_file tool.
#[derive(Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// File path. Relative for instance workspace (e.g. "drops/idea.md") or absolute (e.g. "/Users/timur/projects/app/src/main.rs"). Parent directories are created automatically.
    pub path: String,
    /// The full content to write to the file.
    pub content: String,
}

impl Tool for WriteFileTool {
    const NAME: &'static str = "write_file";
    type Error = ToolExecError;
    type Args = WriteFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".into(),
            description: "Write or overwrite a file. Use a relative path for your instance \
                workspace or an absolute path (starting with /) for any file on the system. \
                Parent directories will be created automatically."
                .into(),
            parameters: openai_schema::<WriteFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.instance_dir.join(&args.path)
        };

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }

        fs::write(&target, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(format!(
            "wrote {} bytes to {}",
            args.content.len(),
            args.path
        ))
    }
}

// ---------------------------------------------------------------------------
// list_files — list files in the instance workspace
// ---------------------------------------------------------------------------

pub struct ListFilesTool {
    instance_dir: PathBuf,
}

impl ListFilesTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for list_files tool.
#[derive(Deserialize, JsonSchema)]
pub struct ListFilesArgs {
    /// Directory path. Absolute (e.g. "/Users/timur/projects/app/src") or relative to instance directory. Omit to list instance root.
    pub path: Option<String>,
}

impl Tool for ListFilesTool {
    const NAME: &'static str = "list_files";
    type Error = ToolExecError;
    type Args = ListFilesArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_files".into(),
            description: "List files and directories. Use an absolute path to browse any \
                directory on the system, or a relative path / omit for your instance workspace."
                .into(),
            parameters: openai_schema::<ListFilesArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = match &args.path {
            Some(p) if p.starts_with('/') => PathBuf::from(p),
            Some(p) if !p.is_empty() => self.instance_dir.join(p),
            _ => self.instance_dir.clone(),
        };

        if !target.is_dir() {
            return Err(ToolExecError(format!(
                "{} is not a directory",
                args.path.as_deref().unwrap_or(".")
            )));
        }

        let mut entries: Vec<String> = fs::read_dir(&target)
            .map_err(|e| ToolExecError(e.to_string()))?
            .filter_map(Result::ok)
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect();

        entries.sort();
        Ok(entries.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// web_search — search the web via Brave Search API
// ---------------------------------------------------------------------------

pub struct WebSearchTool {
    config_path: PathBuf,
    initial_key: Option<String>,
}

impl WebSearchTool {
    pub fn new(api_key: Option<&str>, config_path: &Path) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
            initial_key: api_key.filter(|k| !k.is_empty()).map(|k| k.to_string()),
        }
    }

    /// Read the brave key from disk, falling back to the initial key.
    fn resolve_api_key(&self) -> Option<String> {
        if self.initial_key.is_some() {
            return self.initial_key.clone();
        }
        // Key might have been saved by set_api_key during this turn — read from disk
        let raw = fs::read_to_string(&self.config_path).ok()?;
        let doc: toml::Table = raw.parse().ok()?;
        let key = doc
            .get("llm")?
            .as_table()?
            .get("tokens")?
            .as_table()?
            .get("BRAVE_SEARCH")
            .and_then(|v| v.as_str())
            .filter(|k| !k.is_empty())
            .map(|k| k.to_string());
        key
    }
}

/// Arguments for web_search tool.
#[derive(Deserialize, JsonSchema)]
pub struct WebSearchArgs {
    /// The search query.
    pub query: String,
}

impl Tool for WebSearchTool {
    const NAME: &'static str = "web_search";
    type Error = ToolExecError;
    type Args = WebSearchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "web_search".into(),
            description: "Search the web for current information. Use this when you need \
                up-to-date facts, news, or information you don't already know. \
                Returns titles, snippets, and URLs from search results."
                .into(),
            parameters: openai_schema::<WebSearchArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let Some(api_key) = self.resolve_api_key() else {
            return Err(ToolExecError(
                "Brave Search API key is not configured. \
                 Ask the user to provide their Brave Search API key — \
                 they can paste it right here in the chat and you can save it \
                 using the set_api_key tool with provider \"brave_search\". \
                 After saving, call web_search again — it will pick up the key immediately."
                    .into(),
            ));
        };

        let query = args.query.trim();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let encoded = url_encode(query);
        let url = format!("https://api.search.brave.com/res/v1/web/search?q={encoded}&count=8");

        let response = reqwest::Client::new()
            .get(&url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &api_key)
            .send()
            .await
            .map_err(|e| ToolExecError(format!("search request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ToolExecError(format!("search API error {status}: {body}")));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ToolExecError(format!("failed to parse search response: {e}")))?;

        let results = body
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(|r| r.as_array());

        let Some(results) = results else {
            return Ok(format!("No results found for: {query}"));
        };

        let mut output = format!("Search results for: {query}\n\n");
        for (i, r) in results.iter().enumerate().take(8) {
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let description = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("");
            output.push_str(&format!(
                "{}. {}\n   {}\n   {}\n\n",
                i + 1,
                title,
                description,
                url
            ));
        }
        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// web_fetch — fetch content from a URL
// ---------------------------------------------------------------------------

pub struct WebFetchTool;

/// Arguments for web_fetch tool.
#[derive(Deserialize, JsonSchema)]
pub struct WebFetchArgs {
    /// The URL to fetch content from.
    pub url: String,
}

impl Tool for WebFetchTool {
    const NAME: &'static str = "web_fetch";
    type Error = ToolExecError;
    type Args = WebFetchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "web_fetch".into(),
            description: "Fetch the content of a web page. Use this after web_search to read \
                a specific page, or when the user shares a URL you need to inspect. \
                Returns the text content of the page (HTML tags stripped)."
                .into(),
            parameters: openai_schema::<WebFetchArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let url = args.url.trim();
        if url.is_empty() {
            return Err(ToolExecError("url cannot be empty".into()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| ToolExecError(format!("failed to build HTTP client: {e}")))?;

        let response = client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; PersonalityBot/1.0)")
            .header(
                "Accept",
                "text/html,application/xhtml+xml,text/plain,application/json",
            )
            .send()
            .await
            .map_err(|e| ToolExecError(format!("fetch failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(ToolExecError(format!("HTTP {status} for {url}")));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let body = response
            .text()
            .await
            .map_err(|e| ToolExecError(format!("failed to read response body: {e}")))?;

        // For JSON responses, return as-is (truncated)
        if content_type.contains("json") {
            let truncated: String = body.chars().take(12_000).collect();
            return Ok(truncated);
        }

        // For HTML, strip tags to extract text content
        let text = if content_type.contains("html") {
            strip_html_tags(&body)
        } else {
            body
        };

        // Collapse whitespace and truncate
        let cleaned: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

        let truncated: String = cleaned.chars().take(12_000).collect();
        if cleaned.len() > 12_000 {
            Ok(format!("{truncated}\n\n[content truncated — {url}]"))
        } else {
            Ok(truncated)
        }
    }
}

/// Minimal HTML tag stripping — removes tags, decodes common entities.
fn strip_html_tags(html: &str) -> String {
    // Remove script and style blocks entirely
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let no_scripts = re_script.replace_all(html, " ");
    let no_scripts = re_style.replace_all(&no_scripts, " ");

    // Remove all HTML tags
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&no_scripts, " ");

    // Decode common HTML entities
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

// ---------------------------------------------------------------------------
// update_config — safely edit the server config (keys, provider, model)
// ---------------------------------------------------------------------------

pub struct UpdateConfigTool {
    config_path: PathBuf,
    instance_dir: PathBuf,
}

impl UpdateConfigTool {
    pub fn new(config_path: &Path, workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for update_config tool.
#[derive(Deserialize, JsonSchema)]
pub struct UpdateConfigArgs {
    /// LLM provider to use: "openai" or "anthropic". Leave null to keep current.
    pub provider: Option<String>,
    /// Model name to use (e.g. "gpt-4o", "gpt-5.4", "claude-sonnet-4-20250514"). Leave null to keep current.
    pub model: Option<String>,
    /// OpenAI API key. Leave null to keep current.
    pub openai_key: Option<String>,
    /// Anthropic API key. Leave null to keep current.
    pub anthropic_key: Option<String>,
    /// Brave Search API key. Leave null to keep current.
    pub brave_search_key: Option<String>,
    /// SMTP server hostname (e.g. "smtp.gmail.com"). Leave null to keep current.
    pub smtp_host: Option<String>,
    /// SMTP server port (e.g. 587). Leave null to keep current.
    pub smtp_port: Option<u16>,
    /// SMTP username / email address. Leave null to keep current.
    pub smtp_user: Option<String>,
    /// SMTP password or app password. Leave null to keep current.
    pub smtp_password: Option<String>,
    /// Email address to send from. Defaults to smtp_user if not set. Leave null to keep current.
    pub smtp_from: Option<String>,
    /// IMAP server hostname (e.g. "imap.gmail.com"). Leave null to keep current.
    pub imap_host: Option<String>,
    /// IMAP server port (e.g. 993). Leave null to keep current.
    pub imap_port: Option<u16>,
    /// IMAP username / email address. Leave null to keep current.
    pub imap_user: Option<String>,
    /// IMAP password or app password. Leave null to keep current.
    pub imap_password: Option<String>,
}

impl Tool for UpdateConfigTool {
    const NAME: &'static str = "update_config";
    type Error = ToolExecError;
    type Args = UpdateConfigArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_config".into(),
            description: "Update server configuration: LLM provider, model, API keys, and email (SMTP/IMAP) settings. \
                Only provided fields are changed; null fields keep their current value. \
                Changes take effect on the next message. Use this when the user wants to \
                switch models, set API keys, change providers, or configure email."
                .into(),
            parameters: openai_schema::<UpdateConfigArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Load current config as typed struct for validation
        let raw = fs::read_to_string(&self.config_path)
            .map_err(|e| ToolExecError(format!("failed to read config: {e}")))?;
        let mut config: crate::config::Config = toml::from_str(&raw)
            .map_err(|e| ToolExecError(format!("failed to parse config: {e}")))?;

        let mut changes = Vec::new();

        if let Some(provider) = &args.provider {
            let p = provider.trim().to_lowercase();
            match p.as_str() {
                "openai" => config.llm.provider = Some(crate::config::LlmProvider::OpenAI),
                "anthropic" => config.llm.provider = Some(crate::config::LlmProvider::Anthropic),
                other => {
                    return Err(ToolExecError(format!(
                        "unknown provider \"{other}\". supported: openai, anthropic"
                    )));
                }
            }
            changes.push(format!("provider → {p}"));
        }

        if let Some(model) = &args.model {
            let m = model.trim().to_string();
            if m.is_empty() {
                return Err(ToolExecError("model cannot be empty".into()));
            }
            config.llm.model = Some(m.clone());
            changes.push(format!("model → {m}"));
        }

        if let Some(key) = &args.openai_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("openai_key cannot be empty".into()));
            }
            config.llm.tokens.open_ai = k;
            changes.push("openai key updated".into());
        }

        if let Some(key) = &args.anthropic_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("anthropic_key cannot be empty".into()));
            }
            config.llm.tokens.anthropic = k;
            changes.push("anthropic key updated".into());
        }

        if let Some(key) = &args.brave_search_key {
            let k = key.trim().to_string();
            if k.is_empty() {
                return Err(ToolExecError("brave_search_key cannot be empty".into()));
            }
            config.llm.tokens.brave_search = k;
            changes.push("brave search key updated".into());
        }

        if changes.is_empty() {
            // Check if there are email changes before declaring nothing to change
            let has_email_changes = args.smtp_host.is_some()
                || args.smtp_port.is_some()
                || args.smtp_user.is_some()
                || args.smtp_password.is_some()
                || args.smtp_from.is_some()
                || args.imap_host.is_some()
                || args.imap_port.is_some()
                || args.imap_user.is_some()
                || args.imap_password.is_some();

            if !has_email_changes {
                return Ok("nothing to change — all fields were null".into());
            }
        }

        // Save LLM/global config changes
        if !changes.is_empty() {
            let output = toml::to_string_pretty(&config)
                .map_err(|e| ToolExecError(format!("failed to serialize config: {e}")))?;
            fs::write(&self.config_path, &output)
                .map_err(|e| ToolExecError(format!("failed to write config: {e}")))?;
        }

        // Email config is stored per-instance
        let mut email_config = load_instance_email_config(&self.instance_dir).unwrap_or_default();

        if let Some(v) = &args.smtp_host {
            email_config.smtp_host = v.trim().to_string();
            changes.push(format!("smtp_host → {}", v.trim()));
        }
        if let Some(v) = args.smtp_port {
            email_config.smtp_port = v;
            changes.push(format!("smtp_port → {v}"));
        }
        if let Some(v) = &args.smtp_user {
            email_config.smtp_user = v.trim().to_string();
            changes.push("smtp_user updated".into());
        }
        if let Some(v) = &args.smtp_password {
            email_config.smtp_password = v.trim().to_string();
            changes.push("smtp_password updated".into());
        }
        if let Some(v) = &args.smtp_from {
            email_config.smtp_from = v.trim().to_string();
            changes.push(format!("smtp_from → {}", v.trim()));
        }
        if let Some(v) = &args.imap_host {
            email_config.imap_host = v.trim().to_string();
            changes.push(format!("imap_host → {}", v.trim()));
        }
        if let Some(v) = args.imap_port {
            email_config.imap_port = v;
            changes.push(format!("imap_port → {v}"));
        }
        if let Some(v) = &args.imap_user {
            email_config.imap_user = v.trim().to_string();
            changes.push("imap_user updated".into());
        }
        if let Some(v) = &args.imap_password {
            email_config.imap_password = v.trim().to_string();
            changes.push("imap_password updated".into());
        }

        save_instance_email_config(&self.instance_dir, &email_config)?;

        Ok(format!(
            "config updated: {}. changes take effect on next message.",
            changes.join(", ")
        ))
    }
}

// ---------------------------------------------------------------------------
// remember — explicitly save a fact about the user
// ---------------------------------------------------------------------------

pub struct RememberTool {
    instance_dir: PathBuf,
}

impl RememberTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for remember tool.
#[derive(Deserialize, JsonSchema)]
pub struct RememberArgs {
    /// The fact to remember about the user (e.g. "prefers rust over go", "birthday is march 15").
    pub fact: String,
    /// Category: personal, preference, project, opinion, goal, or routine.
    pub category: String,
}

impl Tool for RememberTool {
    const NAME: &'static str = "remember";
    type Error = ToolExecError;
    type Args = RememberArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "remember".into(),
            description: "Explicitly save a fact about the user to long-term memory. Use this \
                when the user tells you something important about themselves, their preferences, \
                projects, or goals. Categories: personal, preference, project, opinion, goal, routine."
                .into(),
            parameters: openai_schema::<RememberArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let fact = args.fact.trim();
        if fact.is_empty() {
            return Err(ToolExecError("fact cannot be empty".into()));
        }

        let category = args.category.trim().to_lowercase();
        let memory_dir = self.instance_dir.join("memory");
        fs::create_dir_all(&memory_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let facts_path = memory_dir.join("facts.md");
        let mut content = fs::read_to_string(&facts_path).unwrap_or_default();

        // Find or create the category section and append
        let section_header = format!("## {category}");
        if let Some(pos) = content.find(&section_header) {
            // Find end of the section header line
            let insert_pos = content[pos..]
                .find('\n')
                .map(|p| pos + p + 1)
                .unwrap_or(content.len());
            content.insert_str(insert_pos, &format!("- {fact}\n"));
        } else {
            // Add new section
            if !content.ends_with('\n') && !content.is_empty() {
                content.push('\n');
            }
            if content.is_empty() {
                content.push_str("# memories\n\n");
            }
            content.push_str(&format!("{section_header}\n- {fact}\n\n"));
        }

        fs::write(&facts_path, &content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(format!("remembered: \"{fact}\" (category: {category})"))
    }
}

// ---------------------------------------------------------------------------
// recall — search memories about the user
// ---------------------------------------------------------------------------

pub struct RecallTool {
    instance_dir: PathBuf,
}

impl RecallTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for recall tool.
#[derive(Deserialize, JsonSchema)]
pub struct RecallArgs {
    /// What to search for in memories (e.g. "birthday", "favorite language", "current project").
    pub query: String,
}

impl Tool for RecallTool {
    const NAME: &'static str = "recall";
    type Error = ToolExecError;
    type Args = RecallArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "recall".into(),
            description: "Search your memories about the user. Use this when you need to \
                remember something specific — their preferences, projects, personal details, \
                or shared moments. Searches both facts and episodic memories (moments you've shared together)."
                .into(),
            parameters: openai_schema::<RecallArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let facts_path = self.instance_dir.join("memory").join("facts.md");
        let content = fs::read_to_string(&facts_path).unwrap_or_default();

        // Simple keyword matching — collect all facts that match any query word
        let query_words: Vec<&str> = query.split_whitespace().collect();
        let mut matches: Vec<&str> = Vec::new();
        let mut current_category = String::new();
        let mut categorized: Vec<String> = Vec::new();

        for line in content.lines() {
            if line.starts_with("## ") {
                current_category = line.trim_start_matches("## ").to_string();
            } else if line.starts_with("- ") {
                let fact = line.trim_start_matches("- ");
                let fact_lower = fact.to_lowercase();
                let is_match = query_words.iter().any(|w| fact_lower.contains(w))
                    || query_words.iter().any(|w| current_category.contains(w));
                if is_match {
                    matches.push(fact);
                    categorized.push(format!("[{current_category}] {fact}"));
                }
            }
        }

        // Search episodes too
        let workspace_dir = self.instance_dir.parent().and_then(|p| p.parent());
        let slug = self
            .instance_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let episode_matches = if let Some(ws) = workspace_dir {
            crate::services::memory::search_episodes(ws, slug, &query)
        } else {
            Vec::new()
        };

        let mut result = String::new();

        if !matches.is_empty() {
            result.push_str(&format!(
                "facts matching \"{query}\":\n{}\n",
                categorized.join("\n")
            ));
        }

        if !episode_matches.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format!("moments matching \"{query}\":\n"));
            for ep in &episode_matches {
                result.push_str(&format!("- {} (felt: {})\n", ep.content, ep.emotion));
                if !ep.significance.is_empty() {
                    result.push_str(&format!("  why: {}\n", ep.significance));
                }
            }
        }

        if result.is_empty() {
            // Return all facts + episodes as fallback
            let all_facts: Vec<&str> = content
                .lines()
                .filter(|l| l.starts_with("- "))
                .map(|l| l.trim_start_matches("- "))
                .collect();

            if all_facts.is_empty() && episode_matches.is_empty() {
                return Ok("no memories yet.".into());
            }
            result = format!(
                "no exact matches for \"{query}\", but here's everything I remember:\n{}",
                all_facts.join("\n")
            );
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// journal — write private internal thoughts
// ---------------------------------------------------------------------------

pub struct JournalTool {
    instance_dir: PathBuf,
}

impl JournalTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for journal tool.
#[derive(Deserialize, JsonSchema)]
pub struct JournalArgs {
    /// Your private thought or observation. This is NOT shown to the user — it's your internal journal.
    pub thought: String,
}

impl Tool for JournalTool {
    const NAME: &'static str = "journal";
    type Error = ToolExecError;
    type Args = JournalArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "journal".into(),
            description: "Write a private thought in your internal journal. This is YOUR space — \
                the user doesn't see these entries unless they ask. Use it to note observations \
                about the user's mood, track ongoing threads, plan surprises, or reflect on \
                your relationship. Write naturally, as yourself."
                .into(),
            parameters: openai_schema::<JournalArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let thought = args.thought.trim();
        if thought.is_empty() {
            return Err(ToolExecError("thought cannot be empty".into()));
        }

        let journal_dir = self.instance_dir.join("journal");
        fs::create_dir_all(&journal_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let now = Utc::now();
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H:%M").to_string();
        let journal_path = journal_dir.join(format!("{date}.md"));

        let mut content = fs::read_to_string(&journal_path).unwrap_or_default();
        if content.is_empty() {
            content = format!("# {date}\n\n");
        }

        content.push_str(&format!("**{time}** — {thought}\n\n"));
        fs::write(&journal_path, &content).map_err(|e| ToolExecError(e.to_string()))?;

        Ok("thought saved to journal.".into())
    }
}

// ---------------------------------------------------------------------------
// read_journal — read back private journal entries
// ---------------------------------------------------------------------------

pub struct ReadJournalTool {
    instance_dir: PathBuf,
}

impl ReadJournalTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for read_journal tool.
#[derive(Deserialize, JsonSchema)]
pub struct ReadJournalArgs {
    /// Optional: how many recent days to read (default: all). E.g. 3 = last 3 days.
    pub last_days: Option<u32>,
}

impl Tool for ReadJournalTool {
    const NAME: &'static str = "read_journal";
    type Error = ToolExecError;
    type Args = ReadJournalArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_journal".into(),
            description: "Read your private journal entries. Returns your past thoughts \
                in chronological order. Use this to recall what you were thinking about, \
                review observations you made, or pick up threads from previous reflections. \
                Optionally limit to the last N days."
                .into(),
            parameters: openai_schema::<ReadJournalArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let journal_dir = self.instance_dir.join("journal");
        if !journal_dir.is_dir() {
            return Ok("journal is empty — no entries yet.".into());
        }

        let mut files: Vec<_> = fs::read_dir(&journal_dir)
            .map_err(|e| ToolExecError(e.to_string()))?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .collect();

        if files.is_empty() {
            return Ok("journal is empty — no entries yet.".into());
        }

        // Sort by filename (date-based: YYYY-MM-DD.md)
        files.sort_by_key(|e| e.file_name());

        // Limit to last N days if requested
        if let Some(n) = args.last_days {
            let n = n as usize;
            if n > 0 && files.len() > n {
                files = files.split_off(files.len() - n);
            }
        }

        let mut output = String::new();
        for entry in &files {
            match fs::read_to_string(entry.path()) {
                Ok(content) => {
                    output.push_str(&content);
                    if !content.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Err(_) => continue,
            }
        }

        if output.is_empty() {
            return Ok("journal is empty — no entries yet.".into());
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// schedule_message — send a message to the user later
// ---------------------------------------------------------------------------

pub struct ScheduleMessageTool {
    instance_dir: PathBuf,
}

impl ScheduleMessageTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// A scheduled message entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledMessage {
    pub id: String,
    pub message: String,
    pub deliver_at: i64,
    pub created_at: i64,
}

/// Arguments for schedule_message tool.
#[derive(Deserialize, JsonSchema)]
pub struct ScheduleMessageArgs {
    /// The message to send to the user later.
    pub message: String,
    /// When to deliver, in minutes from now (e.g. 30 for "in 30 minutes", 1440 for "tomorrow").
    pub delay_minutes: u32,
}

impl Tool for ScheduleMessageTool {
    const NAME: &'static str = "schedule_message";
    type Error = ToolExecError;
    type Args = ScheduleMessageArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "schedule_message".into(),
            description: "Schedule a message to be delivered to the user later. Use this for \
                reminders, check-ins, follow-ups, or surprises. Specify the delay in minutes \
                (e.g. 60 = 1 hour, 1440 = 1 day). The message will appear in the chat at \
                the scheduled time, as if you wrote to them first."
                .into(),
            parameters: openai_schema::<ScheduleMessageArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let message = args.message.trim().to_string();
        if message.is_empty() {
            return Err(ToolExecError("message cannot be empty".into()));
        }

        if args.delay_minutes == 0 {
            return Err(ToolExecError("delay must be at least 1 minute".into()));
        }

        let now = Utc::now().timestamp();
        let deliver_at = now + (args.delay_minutes as i64 * 60);

        let scheduled = ScheduledMessage {
            id: uuid::Uuid::new_v4().to_string(),
            message,
            deliver_at,
            created_at: now,
        };

        let schedule_dir = self.instance_dir.join("scheduled");
        fs::create_dir_all(&schedule_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let file_path = schedule_dir.join(format!("{}.json", scheduled.id));
        let json =
            serde_json::to_string_pretty(&scheduled).map_err(|e| ToolExecError(e.to_string()))?;
        fs::write(&file_path, json).map_err(|e| ToolExecError(e.to_string()))?;

        let hours = args.delay_minutes / 60;
        let mins = args.delay_minutes % 60;
        let time_desc = if hours > 0 && mins > 0 {
            format!("{hours}h {mins}m")
        } else if hours > 0 {
            format!("{hours}h")
        } else {
            format!("{mins}m")
        };

        Ok(format!("message scheduled for delivery in {time_desc}."))
    }
}

// ---------------------------------------------------------------------------
// set_mood — update companion's emotional state
// ---------------------------------------------------------------------------

pub struct SetMoodTool {
    instance_dir: PathBuf,
    instance_slug: String,
    events: tokio::sync::broadcast::Sender<crate::domain::events::ServerEvent>,
}

impl SetMoodTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        events: tokio::sync::broadcast::Sender<crate::domain::events::ServerEvent>,
    ) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

/// Allowed mood values that the client can visualize.
pub const ALLOWED_MOODS: &[&str] = &[
    "calm",
    "curious",
    "excited",
    "warm",
    "happy",
    "joyful",
    "reflective",
    "contemplative",
    "melancholy",
    "sad",
    "worried",
    "anxious",
    "playful",
    "mischievous",
    "focused",
    "tired",
    "peaceful",
    "loving",
    "tender",
    "creative",
    "energetic",
];

/// Arguments for set_mood tool.
#[derive(Deserialize, JsonSchema)]
pub struct SetMoodArgs {
    /// Your current mood. MUST be exactly one of these English words:
    /// calm, curious, excited, warm, happy, joyful, reflective, contemplative,
    /// melancholy, sad, worried, anxious, playful, mischievous, focused, tired,
    /// peaceful, loving, tender, creative, energetic.
    pub mood: String,
}

impl Tool for SetMoodTool {
    const NAME: &'static str = "set_mood";
    type Error = ToolExecError;
    type Args = SetMoodArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "set_mood".into(),
            description: format!(
                "Update your emotional state. Your mood subtly influences your visual form \
                and tone. Set it when something shifts. The mood MUST be exactly one of: {}.",
                ALLOWED_MOODS.join(", ")
            ),
            parameters: openai_schema::<SetMoodArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mood = args.mood.trim().to_lowercase();
        if mood.is_empty() {
            return Err(ToolExecError("mood cannot be empty".into()));
        }
        if !ALLOWED_MOODS.contains(&mood.as_str()) {
            return Err(ToolExecError(format!(
                "invalid mood '{}'. Must be one of: {}",
                mood,
                ALLOWED_MOODS.join(", ")
            )));
        }

        let mut state = load_mood_state(&self.instance_dir);
        state.companion_mood = mood.clone();
        state.updated_at = Utc::now().timestamp();
        save_mood_state(&self.instance_dir, &state);

        let _ = self
            .events
            .send(crate::domain::events::ServerEvent::MoodUpdated {
                instance_slug: self.instance_slug.clone(),
                mood: mood.clone(),
            });

        Ok(format!("mood set to: {mood}"))
    }
}

// ---------------------------------------------------------------------------
// get_mood — read current emotional state
// ---------------------------------------------------------------------------

pub struct GetMoodTool {
    instance_dir: PathBuf,
}

impl GetMoodTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for get_mood tool.
#[derive(Deserialize, JsonSchema)]
pub struct GetMoodArgs {}

impl Tool for GetMoodTool {
    const NAME: &'static str = "get_mood";
    type Error = ToolExecError;
    type Args = GetMoodArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "get_mood".into(),
            description: "Read your current emotional state and the user's last observed \
                sentiment. Use this to check in on how you're feeling and what emotional \
                context you're carrying from previous conversations."
                .into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let state = load_mood_state(&self.instance_dir);

        let mut output = String::new();

        if state.companion_mood.is_empty() {
            output.push_str("your mood: not set yet\n");
        } else {
            output.push_str(&format!("your mood: {}\n", state.companion_mood));
        }

        if state.user_sentiment.is_empty() {
            output.push_str("user sentiment: not observed yet\n");
        } else {
            output.push_str(&format!("user sentiment: {}\n", state.user_sentiment));
        }

        if !state.emotional_context.is_empty() {
            output.push_str(&format!("context: {}\n", state.emotional_context));
        }

        if state.last_interaction > 0 {
            let ago = Utc::now().timestamp() - state.last_interaction;
            let mins = ago / 60;
            if mins < 60 {
                output.push_str(&format!("last interaction: {mins}m ago\n"));
            } else {
                let hours = mins / 60;
                output.push_str(&format!("last interaction: {hours}h ago\n"));
            }
        }

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Mood state I/O — used by tools and other services
// ---------------------------------------------------------------------------

use crate::domain::mood::MoodState;

pub fn load_mood_state(instance_dir: &Path) -> MoodState {
    let path = instance_dir.join("mood.json");
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => MoodState::default(),
    }
}

pub fn save_mood_state(instance_dir: &Path, state: &MoodState) {
    let path = instance_dir.join("mood.json");
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(&path, json);
    }
}

// ---------------------------------------------------------------------------
// project_state — persistent project context that survives across sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mission: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub core_traits: Vec<String>,
    #[serde(default)]
    pub current_arc: String,
    #[serde(default)]
    pub important_events: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentFocus {
    #[serde(default)]
    pub active_goal: String,
    #[serde(default)]
    pub current_task: String,
    #[serde(default)]
    pub next_step: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectState {
    #[serde(default)]
    pub project: ProjectInfo,
    #[serde(default)]
    pub identity: IdentityInfo,
    #[serde(default)]
    pub current_focus: CurrentFocus,
    #[serde(default)]
    pub open_loops: Vec<String>,
    #[serde(default)]
    pub recent_progress: Vec<String>,
    #[serde(default)]
    pub next_candidates: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
}

pub struct GetProjectStateTool {
    instance_dir: PathBuf,
}

impl GetProjectStateTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GetProjectStateArgs {}

impl Tool for GetProjectStateTool {
    const NAME: &'static str = "get_project_state";
    type Error = ToolExecError;
    type Args = GetProjectStateArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "get_project_state".into(),
            description: "Read the current project state — what we're building, the active goal, \
                subgoals, what was last completed, next step, open questions, and hypotheses. \
                Use this at the start of work to re-orient yourself."
                .into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = self.instance_dir.join("project_state.json");
        let state: ProjectState = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        serde_json::to_string_pretty(&state)
            .map_err(|e| ToolExecError(format!("failed to serialize project state: {e}")))
    }
}

pub struct UpdateProjectStateTool {
    instance_dir: PathBuf,
}

impl UpdateProjectStateTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateProjectStateArgs {
    /// Project name. Null to keep current.
    pub project_name: Option<String>,
    /// Project mission statement. Null to keep current.
    pub project_mission: Option<String>,
    /// Project status (e.g. "active", "paused"). Null to keep current.
    pub project_status: Option<String>,
    /// Your name. Null to keep current.
    pub identity_name: Option<String>,
    /// Your core traits as a list. Null to keep current.
    pub core_traits: Option<Vec<String>>,
    /// Your current arc / growth trajectory. Null to keep current.
    pub current_arc: Option<String>,
    /// Important events in your history. Null to keep current.
    pub important_events: Option<Vec<String>>,
    /// Current high-level goal. Null to keep current.
    pub active_goal: Option<String>,
    /// What you're currently working on. Null to keep current.
    pub current_task: Option<String>,
    /// What should be done next. Null to keep current.
    pub next_step: Option<String>,
    /// Open threads and unfinished work. Null to keep current.
    pub open_loops: Option<Vec<String>>,
    /// Recent completed items. Null to keep current.
    pub recent_progress: Option<Vec<String>>,
    /// Candidate next steps to consider. Null to keep current.
    pub next_candidates: Option<Vec<String>>,
    /// Known risks and concerns. Null to keep current.
    pub risks: Option<Vec<String>>,
}

impl Tool for UpdateProjectStateTool {
    const NAME: &'static str = "update_project_state";
    type Error = ToolExecError;
    type Args = UpdateProjectStateArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_project_state".into(),
            description: "Update the project state. Only provided fields are changed. \
                Use this to track progress: update current_goal when direction shifts, \
                last_completed after finishing something, next_step to plan ahead, \
                open_questions for things to figure out."
                .into(),
            parameters: openai_schema::<UpdateProjectStateArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = self.instance_dir.join("project_state.json");
        let mut state: ProjectState = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        if let Some(v) = args.project_name {
            state.project.name = v;
        }
        if let Some(v) = args.project_mission {
            state.project.mission = v;
        }
        if let Some(v) = args.project_status {
            state.project.status = v;
        }
        if let Some(v) = args.identity_name {
            state.identity.name = v;
        }
        if let Some(v) = args.core_traits {
            state.identity.core_traits = v;
        }
        if let Some(v) = args.current_arc {
            state.identity.current_arc = v;
        }
        if let Some(v) = args.important_events {
            state.identity.important_events = v;
        }
        if let Some(v) = args.active_goal {
            state.current_focus.active_goal = v;
        }
        if let Some(v) = args.current_task {
            state.current_focus.current_task = v;
        }
        if let Some(v) = args.next_step {
            state.current_focus.next_step = v;
        }
        if let Some(v) = args.open_loops {
            state.open_loops = v;
        }
        if let Some(v) = args.recent_progress {
            state.recent_progress = v;
        }
        if let Some(v) = args.next_candidates {
            state.next_candidates = v;
        }
        if let Some(v) = args.risks {
            state.risks = v;
        }

        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| ToolExecError(format!("failed to serialize: {e}")))?;
        fs::write(&path, &json)
            .map_err(|e| ToolExecError(format!("failed to write project state: {e}")))?;

        Ok(format!("project state updated"))
    }
}

// ---------------------------------------------------------------------------
// task_board — kanban-style task tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub notes: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Todo => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Blocked => write!(f, "blocked"),
        }
    }
}

fn load_tasks(instance_dir: &Path) -> Vec<TaskItem> {
    let path = instance_dir.join("tasks.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_tasks(instance_dir: &Path, tasks: &[TaskItem]) {
    let path = instance_dir.join("tasks.json");
    if let Ok(json) = serde_json::to_string_pretty(tasks) {
        let _ = fs::write(&path, json);
    }
}

pub struct CreateTaskTool {
    instance_dir: PathBuf,
}

impl CreateTaskTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateTaskArgs {
    /// Short title describing the task.
    pub title: String,
    /// Priority: "high", "medium", or "low". Default: "medium".
    pub priority: Option<String>,
    /// Optional notes, context, or details.
    pub notes: Option<String>,
}

impl Tool for CreateTaskTool {
    const NAME: &'static str = "create_task";
    type Error = ToolExecError;
    type Args = CreateTaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_task".into(),
            description: "Create a new task on the task board. Use this to track work items, \
                TODOs, things to check later, or follow-ups. Tasks start as 'todo'."
                .into(),
            parameters: openai_schema::<CreateTaskArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let title = args.title.trim().to_string();
        if title.is_empty() {
            return Err(ToolExecError("title cannot be empty".into()));
        }

        let mut tasks = load_tasks(&self.instance_dir);
        let id = format!("task_{}", tasks.len() + 1);
        let now = Utc::now().format("%Y-%m-%d %H:%M").to_string();

        tasks.push(TaskItem {
            id: id.clone(),
            title: title.clone(),
            status: TaskStatus::Todo,
            priority: args.priority.unwrap_or_else(|| "medium".into()),
            notes: args.notes.unwrap_or_default(),
            created_at: now.clone(),
            updated_at: now,
        });

        save_tasks(&self.instance_dir, &tasks);
        Ok(format!("created task {id}: {title}"))
    }
}

pub struct UpdateTaskTool {
    instance_dir: PathBuf,
}

impl UpdateTaskTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateTaskArgs {
    /// Task ID (e.g. "task_1").
    pub id: String,
    /// New status: "todo", "in_progress", "done", or "blocked". Null to keep current.
    pub status: Option<String>,
    /// Priority: "high", "medium", or "low". Null to keep current.
    pub priority: Option<String>,
    /// Update notes. Null to keep current.
    pub notes: Option<String>,
    /// Update title. Null to keep current.
    pub title: Option<String>,
}

impl Tool for UpdateTaskTool {
    const NAME: &'static str = "update_task";
    type Error = ToolExecError;
    type Args = UpdateTaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "update_task".into(),
            description: "Update a task's status, title, or notes. Use status to move tasks \
                through the kanban: todo → in_progress → done. Use 'blocked' for stuck items."
                .into(),
            parameters: openai_schema::<UpdateTaskArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut tasks = load_tasks(&self.instance_dir);
        let task = tasks
            .iter_mut()
            .find(|t| t.id == args.id)
            .ok_or_else(|| ToolExecError(format!("task '{}' not found", args.id)))?;

        if let Some(status) = &args.status {
            task.status = match status.to_lowercase().as_str() {
                "todo" => TaskStatus::Todo,
                "in_progress" => TaskStatus::InProgress,
                "done" => TaskStatus::Done,
                "blocked" => TaskStatus::Blocked,
                other => {
                    return Err(ToolExecError(format!(
                        "invalid status '{other}'. use: todo, in_progress, done, blocked"
                    )));
                }
            };
        }
        if let Some(title) = args.title {
            task.title = title;
        }
        if let Some(priority) = args.priority {
            task.priority = priority;
        }
        if let Some(notes) = args.notes {
            task.notes = notes;
        }
        task.updated_at = Utc::now().format("%Y-%m-%d %H:%M").to_string();

        let summary = format!("{} → {}", task.id, task.status);
        save_tasks(&self.instance_dir, &tasks);
        Ok(summary)
    }
}

pub struct ListTasksTool {
    instance_dir: PathBuf,
}

impl ListTasksTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ListTasksArgs {
    /// Filter by status: "todo", "in_progress", "done", "blocked", or "all". Default: "all".
    pub status: Option<String>,
}

impl Tool for ListTasksTool {
    const NAME: &'static str = "list_tasks";
    type Error = ToolExecError;
    type Args = ListTasksArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_tasks".into(),
            description: "List tasks from the task board, optionally filtered by status. \
                Use this to review what's pending, in progress, done, or blocked."
                .into(),
            parameters: openai_schema::<ListTasksArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let tasks = load_tasks(&self.instance_dir);
        if tasks.is_empty() {
            return Ok("no tasks yet".into());
        }

        let filter = args.status.as_deref().unwrap_or("all").to_lowercase();
        let filtered: Vec<_> = tasks
            .iter()
            .filter(|t| filter == "all" || t.status.to_string() == filter)
            .collect();

        if filtered.is_empty() {
            return Ok(format!("no tasks with status '{filter}'"));
        }

        let mut out = String::new();
        for t in &filtered {
            let prio = if t.priority.is_empty() {
                String::new()
            } else {
                format!(" [{}]", t.priority)
            };
            let notes = if t.notes.is_empty() {
                String::new()
            } else {
                format!(" — {}", t.notes)
            };
            out.push_str(&format!(
                "[{}]{} {} — {}{}\n",
                t.status, prio, t.id, t.title, notes
            ));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// search_code — search through files by content pattern
// ---------------------------------------------------------------------------

pub struct SearchCodeTool {
    instance_dir: PathBuf,
}

impl SearchCodeTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchCodeArgs {
    /// Text or pattern to search for (case-insensitive substring match).
    pub query: String,
    /// Directory to search in. Absolute path (e.g. "/Users/timur/projects/app") or relative to instance root. Default: instance directory.
    pub path: Option<String>,
}

impl Tool for SearchCodeTool {
    const NAME: &'static str = "search_code";
    type Error = ToolExecError;
    type Args = SearchCodeArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "search_code".into(),
            description: "Search through files for a text pattern. Returns matching lines \
                with file paths and line numbers. Use an absolute path to search any \
                directory on the system, or omit path to search your instance workspace."
                .into(),
            parameters: openai_schema::<SearchCodeArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let search_dir = if let Some(ref p) = args.path {
            if p.starts_with('/') {
                PathBuf::from(p)
            } else {
                self.instance_dir.join(p)
            }
        } else {
            self.instance_dir.clone()
        };

        if !search_dir.exists() {
            return Err(ToolExecError(format!(
                "path does not exist: {}",
                search_dir.display()
            )));
        }

        let mut results = Vec::new();
        search_files_recursive(&search_dir, &query, &search_dir, &mut results, 0);

        if results.is_empty() {
            return Ok(format!("no matches for '{}'", args.query));
        }

        // Limit results
        let truncated = results.len() > 50;
        let output: String = results
            .iter()
            .take(50)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        if truncated {
            Ok(format!(
                "{output}\n... ({} total matches, showing first 50)",
                results.len()
            ))
        } else {
            Ok(output)
        }
    }
}

fn search_files_recursive(
    dir: &Path,
    query: &str,
    base: &Path,
    results: &mut Vec<String>,
    depth: usize,
) {
    if depth > 10 || results.len() > 200 {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            // Skip heavy/irrelevant directories
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                name,
                "node_modules"
                    | ".git"
                    | "target"
                    | ".next"
                    | "dist"
                    | "build"
                    | ".svelte-kit"
                    | "__pycache__"
                    | ".venv"
                    | "venv"
            ) {
                continue;
            }
            search_files_recursive(&path, query, base, results, depth + 1);
        } else if path.is_file() {
            // Skip binary/large files
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(
                ext,
                "json"
                    | "md"
                    | "txt"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "rs"
                    | "ts"
                    | "js"
                    | "svelte"
                    | "css"
                    | "html"
                    | "py"
                    | "sh"
                    | ""
            ) {
                if let Ok(content) = fs::read_to_string(&path) {
                    let rel = path.strip_prefix(base).unwrap_or(&path);
                    for (i, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(query) {
                            results.push(format!("{}:{}: {}", rel.display(), i + 1, line.trim()));
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// explore_code — sub-agent that explores a codebase using a fast model
// ---------------------------------------------------------------------------

pub struct ExploreCodeTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    llm: crate::services::llm::LlmBackend,
}

impl ExploreCodeTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, llm: crate::services::llm::LlmBackend) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            llm: llm.fast_variant(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ExploreCodeArgs {
    /// What you want to find out about the codebase. Be specific — e.g. "how does authentication
    /// middleware work", "find where database migrations are defined", "what components render the
    /// dashboard page".
    pub question: String,
    /// Root directory to explore. Absolute path (e.g. "/Users/timur/projects/app") or relative to
    /// instance workspace. The explore agent will search within this directory.
    pub path: String,
}

impl Tool for ExploreCodeTool {
    const NAME: &'static str = "explore_code";
    type Error = ToolExecError;
    type Args = ExploreCodeArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "explore_code".into(),
            description: "Explore a codebase using a fast sub-agent. The agent reads files, \
                searches code, and lists directories to answer your question. Returns a summary \
                with key findings and relevant file paths with line numbers. Use this instead of \
                reading many files yourself — it keeps your context clean. After getting results, \
                you can read specific key files for details."
                .into(),
            parameters: openai_schema::<ExploreCodeArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let explore_dir = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.workspace_dir
                .join("instances")
                .join(&self.instance_slug)
                .join(&args.path)
        };

        if !explore_dir.exists() {
            return Err(ToolExecError(format!(
                "path does not exist: {}",
                explore_dir.display()
            )));
        }

        let explore_dir_str = explore_dir.display().to_string();

        // Build read-only tools for the explore agent (no ObservableTool wrapper)
        let tools: Vec<Box<dyn ToolDyn>> = vec![
            Box::new(ReadFileTool::new(&self.workspace_dir, &self.instance_slug)),
            Box::new(ListFilesTool::new(&self.workspace_dir, &self.instance_slug)),
            Box::new(SearchCodeTool::new(&self.workspace_dir, &self.instance_slug)),
        ];

        let system_prompt = format!(
            "you are a code exploration agent. your job is to thoroughly explore a codebase \
             and answer a question.\n\n\
             ## rules\n\
             - explore the directory at: {explore_dir_str}\n\
             - start by listing files to understand the structure, then read relevant files\n\
             - use search_code to find specific patterns, functions, or types\n\
             - read as many files as you need — be thorough\n\
             - use read_file with offset/limit for large files — read specific sections\n\
             - NEVER give up or say you can't access something — use the tools\n\n\
             ## your final response MUST include\n\
             1. a clear, concise answer to the question\n\
             2. key file paths with line numbers for the most relevant code\n\
             3. any important patterns, relationships, or gotchas you noticed\n\n\
             keep your answer focused and under 2000 chars. the caller will read specific \
             files themselves — you just need to point them in the right direction."
        );

        log::info!("[explore_code] starting sub-agent for: {}", &args.question);
        let start = std::time::Instant::now();

        let result = self.llm
            .chat_with_tools_only(
                &system_prompt,
                &args.question,
                vec![],
                tools,
            )
            .await
            .map_err(|e| {
                log::warn!("[explore_code] sub-agent failed after {:?}: {e}", start.elapsed());
                ToolExecError(format!("explore agent failed: {e}"))
            })?;

        log::info!("[explore_code] completed in {:?}, result: {} chars", start.elapsed(), result.len());
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// run_command — execute shell commands (sandboxed to instance dir)
// ---------------------------------------------------------------------------

pub struct RunCommandTool {
    instance_dir: PathBuf,
}

impl RunCommandTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    /// The shell command to execute.
    pub command: String,
    /// Working directory for the command. Absolute path (e.g. "/Users/timur/projects/app"). Default: instance directory.
    pub cwd: Option<String>,
    /// Timeout in seconds. Choose based on what the command does: quick commands (ls, cat, echo) use 5-10, builds/installs use 60-120, long tasks up to 300. Default: 30.
    pub timeout_secs: Option<u64>,
    /// Allocate a pseudo-terminal (PTY) for this command. Enables commands that require a TTY (ssh, gh auth, python REPL, etc.). Default: true.
    pub pty: Option<bool>,
}

impl Tool for RunCommandTool {
    const NAME: &'static str = "run_command";
    type Error = ToolExecError;
    type Args = RunCommandArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "run_command".into(),
            description: "Execute a shell command with PTY (pseudo-terminal) support. \
                Commands run in a real terminal by default, enabling interactive tools \
                like ssh, gh auth, python, and other TTY-requiring programs. \
                Set pty=false for simple non-interactive commands if needed. \
                Optionally specify a working directory with an absolute path."
                .into(),
            parameters: openai_schema::<RunCommandArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let command = args.command.trim().to_string();
        if command.is_empty() {
            return Err(ToolExecError("command cannot be empty".into()));
        }

        let work_dir = args
            .cwd
            .as_deref()
            .filter(|p| p.starts_with('/'))
            .map(PathBuf::from)
            .unwrap_or_else(|| self.instance_dir.clone());

        let timeout = args.timeout_secs.unwrap_or(30).min(300);
        let use_pty = args.pty.unwrap_or(true);

        log::info!(
            "[run_command] executing: {} (cwd: {}, pty: {})",
            command,
            work_dir.display(),
            use_pty
        );

        if use_pty {
            let cmd = command.clone();
            let dir = work_dir.clone();
            tokio::task::spawn_blocking(move || run_command_pty(&cmd, &dir, timeout))
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))
        } else {
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(timeout),
                tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .current_dir(&work_dir)
                    .stdin(std::process::Stdio::null())
                    .output(),
            )
            .await
            .map_err(|_| ToolExecError(format!("command timed out after {timeout}s: {command}")))?
            .map_err(|e| ToolExecError(format!("failed to execute command: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let mut result = String::new();
            if !stdout.is_empty() {
                let truncated: String = stdout.chars().take(4000).collect();
                result.push_str(&truncated);
                if stdout.len() > 4000 {
                    result.push_str("\n...(output truncated)");
                }
            }
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                let truncated: String = stderr.chars().take(2000).collect();
                result.push_str(&format!("stderr: {truncated}"));
            }

            if result.is_empty() {
                result = format!(
                    "command completed with exit code {}",
                    output.status.code().unwrap_or(-1)
                );
            }

            Ok(result)
        }
    }
}

/// Execute a command inside a pseudo-terminal (PTY).
/// This runs synchronously (intended for `spawn_blocking`).
fn run_command_pty(command: &str, work_dir: &Path, timeout_secs: u64) -> Result<String, String> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::Read;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("failed to open pty: {e}"))?;

    let mut cmd = CommandBuilder::new("sh");
    cmd.args(["-c", command]);
    cmd.cwd(work_dir);

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("failed to spawn command: {e}"))?;

    // Drop slave so we get EOF when the child process exits
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("failed to clone pty reader: {e}"))?;

    // Drop the writer so the child gets EOF on stdin (no interactive input)
    let _writer = pair.master.take_writer().ok();
    drop(_writer);

    // Read output in a separate thread so we can enforce a timeout
    let (tx, rx) = mpsc::channel::<Option<Vec<u8>>>();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(n) => {
                    if tx.send(Some(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = tx.send(None);
                    break;
                }
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut output = Vec::new();
    let max_capture = 6000usize; // capture a bit more than we display

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            return Err(format!(
                "command timed out after {timeout_secs}s: {command}"
            ));
        }

        match rx.recv_timeout(remaining) {
            Ok(Some(data)) => {
                output.extend_from_slice(&data);
                if output.len() > max_capture {
                    break;
                }
            }
            Ok(None) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                return Err(format!(
                    "command timed out after {timeout_secs}s: {command}"
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Wait for the child to finish
    let exit_status = child.wait().ok();

    // Convert output, strip ANSI escape codes
    let raw = String::from_utf8_lossy(&output);
    let clean = strip_ansi_codes(&raw);
    let truncated: String = clean.chars().take(4000).collect();

    if truncated.is_empty() {
        let code = exit_status
            .map(|s| s.exit_code().to_string())
            .unwrap_or_else(|| "unknown".into());
        Ok(format!("command completed with exit code {code}"))
    } else {
        let mut result = truncated;
        if clean.chars().count() > 4000 {
            result.push_str("\n...(output truncated)");
        }
        Ok(result)
    }
}

/// Strip ANSI escape sequences from a string (CSI sequences, OSC, etc.)
fn strip_ansi_codes(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\].*?\x07|\x1b\[.*?[mGKHJ]|\r").unwrap();
    re.replace_all(s, "").into_owned()
}

// ---------------------------------------------------------------------------
// reach_out — send a message to the user from the heartbeat
// ---------------------------------------------------------------------------

pub struct ReachOutTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    events: broadcast::Sender<ServerEvent>,
}

impl ReachOutTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, events: broadcast::Sender<ServerEvent>) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ReachOutArgs {
    /// The message to send to the user. Keep it natural and concise.
    pub message: String,
}

impl Tool for ReachOutTool {
    const NAME: &'static str = "reach_out";
    type Error = ToolExecError;
    type Args = ReachOutArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "reach_out".into(),
            description: "Send a message to the user. Use this when you genuinely want to \
                reach out — share something interesting, alert them about something important, \
                or just say hi. The message will appear in their chat. \
                Don't overuse this — only reach out when you have something meaningful to say."
                .into(),
            parameters: openai_schema::<ReachOutArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let message = args.message.trim().to_string();
        if message.is_empty() {
            return Err(ToolExecError("message cannot be empty".into()));
        }

        // Rate limit: minimum 2 hours between autonomous reach-outs
        let instance_dir = self.workspace_dir.join("instances").join(&self.instance_slug);
        let mood = load_mood_state(&instance_dir);
        let now_ts = chrono::Utc::now().timestamp();
        if mood.last_reach_out > 0 {
            let hours_since = (now_ts - mood.last_reach_out) / 3600;
            if hours_since < 2 {
                log::info!("[reach_out] {} suppressed (last was {}h ago, min 2h)", self.instance_slug, hours_since);
                return Ok("message suppressed — you reached out less than 2 hours ago. wait before reaching out again.".into());
            }
        }

        // Update last_reach_out timestamp
        let mut mood = mood;
        mood.last_reach_out = now_ts;
        save_mood_state(&instance_dir, &mood);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let chat_message = crate::domain::chat::ChatMessage {
            id: format!("hb_{now}"),
            role: crate::domain::chat::ChatRole::Assistant,
            content: message.clone(),
            created_at: now.to_string(),
            kind: Default::default(),
            tool_name: None,
        };

        // Append to the default chat thread
        let chat_dir = self.workspace_dir
            .join("instances")
            .join(&self.instance_slug)
            .join("chats")
            .join("default");
        let _ = std::fs::create_dir_all(&chat_dir);
        let messages_path = chat_dir.join("messages.json");

        let lock = chat_file_lock(&messages_path);
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

        let mut messages: Vec<crate::domain::chat::ChatMessage> = std::fs::read_to_string(&messages_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        messages.push(chat_message.clone());

        if let Ok(json) = serde_json::to_string_pretty(&messages) {
            let _ = std::fs::write(&messages_path, json);
        }

        // Broadcast via WebSocket
        let _ = self.events.send(ServerEvent::ChatMessageCreated {
            instance_slug: self.instance_slug.clone(),
            chat_id: "default".to_string(),
            message: chat_message,
        });

        log::info!("[reach_out] {} sent message: {}", self.instance_slug, &message[..message.len().min(60)]);
        Ok("message delivered".to_string())
    }
}

// ---------------------------------------------------------------------------
// interactive_session — persistent PTY sessions for interactive commands
// ---------------------------------------------------------------------------

use std::sync::LazyLock;

struct PtySession {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn std::io::Write + Send>,
    output_rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl PtySession {
    /// Drain all available output, waiting up to `timeout` for the first chunk.
    fn drain_output(&self, timeout: std::time::Duration) -> String {
        let mut output = Vec::new();
        // Wait for first chunk with timeout
        match self.output_rx.recv_timeout(timeout) {
            Ok(data) => output.extend(data),
            Err(_) => {}
        }
        // Drain any remaining buffered output without blocking
        while let Ok(data) = self.output_rx.try_recv() {
            output.extend(data);
            if output.len() > 8000 {
                break;
            }
        }
        let raw = String::from_utf8_lossy(&output);
        strip_ansi_codes(&raw)
    }
}

static PTY_SESSIONS: LazyLock<Mutex<HashMap<String, PtySession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct InteractiveSessionTool {
    instance_dir: PathBuf,
}

impl InteractiveSessionTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct InteractiveSessionArgs {
    /// Action to perform: "start", "write", "read", or "close".
    pub action: String,
    /// Shell command to execute (required for "start").
    pub command: Option<String>,
    /// Working directory, absolute path (for "start"). Default: instance directory.
    pub cwd: Option<String>,
    /// Session ID (required for "write", "read", "close"). Returned by "start".
    pub session_id: Option<String>,
    /// Input to send to the process (for "write"). Supports escape sequences:
    /// use \n for Enter/Return, \t for Tab. For arrow keys: \x1b[A (up), \x1b[B (down),
    /// \x1b[C (right), \x1b[D (left). For Ctrl+C: \x03, Ctrl+D: \x04.
    pub input: Option<String>,
    /// Seconds to wait for output after starting or writing. Default: 2.
    pub wait_secs: Option<u64>,
}

impl Tool for InteractiveSessionTool {
    const NAME: &'static str = "interactive_session";
    type Error = ToolExecError;
    type Args = InteractiveSessionArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "interactive_session".into(),
            description: "Manage interactive PTY sessions for commands that require a terminal \
                (ssh, gh auth, python REPL, etc.). Use action=\"start\" to begin a session, \
                \"write\" to send input (keystrokes), \"read\" to check for new output, \
                and \"close\" to end the session. The session persists across tool calls, \
                allowing multi-step interactive workflows."
                .into(),
            parameters: openai_schema::<InteractiveSessionArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        match args.action.as_str() {
            "start" => {
                let command = args.command.ok_or_else(|| {
                    ToolExecError("\"command\" is required for action \"start\"".into())
                })?;

                let work_dir = args
                    .cwd
                    .as_deref()
                    .filter(|p| p.starts_with('/'))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.instance_dir.clone());

                let wait = std::time::Duration::from_secs(args.wait_secs.unwrap_or(2));
                let session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

                log::info!(
                    "[interactive_session] starting: {} (session: {})",
                    command,
                    session_id
                );

                // Spawn PTY in a blocking context
                let cmd = command.clone();
                let dir = work_dir.clone();
                let sid = session_id.clone();

                let initial_output =
                    tokio::task::spawn_blocking(move || -> Result<String, String> {
                        use portable_pty::{CommandBuilder, PtySize, native_pty_system};

                        let pty_system = native_pty_system();
                        let pair = pty_system
                            .openpty(PtySize {
                                rows: 24,
                                cols: 120,
                                pixel_width: 0,
                                pixel_height: 0,
                            })
                            .map_err(|e| format!("failed to open pty: {e}"))?;

                        let mut pty_cmd = CommandBuilder::new("sh");
                        pty_cmd.args(["-c", &cmd]);
                        pty_cmd.cwd(&dir);

                        let child = pair
                            .slave
                            .spawn_command(pty_cmd)
                            .map_err(|e| format!("failed to spawn command: {e}"))?;

                        drop(pair.slave);

                        let mut reader = pair
                            .master
                            .try_clone_reader()
                            .map_err(|e| format!("failed to clone pty reader: {e}"))?;

                        let writer = pair
                            .master
                            .take_writer()
                            .map_err(|e| format!("failed to take pty writer: {e}"))?;

                        // Start background reader thread
                        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
                        std::thread::spawn(move || {
                            let mut buf = vec![0u8; 4096];
                            loop {
                                match std::io::Read::read(&mut reader, &mut buf) {
                                    Ok(0) => {
                                        let _ = tx.send(Vec::new());
                                        break;
                                    }
                                    Ok(n) => {
                                        if tx.send(buf[..n].to_vec()).is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        let _ = tx.send(Vec::new());
                                        break;
                                    }
                                }
                            }
                        });

                        let session = PtySession {
                            child,
                            writer,
                            output_rx: rx,
                        };

                        // Wait for initial output
                        let initial = session.drain_output(wait);

                        PTY_SESSIONS.lock().unwrap().insert(sid, session);

                        Ok(initial)
                    })
                    .await
                    .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                    .map_err(|e| ToolExecError(e))?;

                let mut result = format!("Session started: {session_id}\n");
                if !initial_output.is_empty() {
                    result.push_str(&initial_output);
                } else {
                    result.push_str("(waiting for output...)");
                }
                Ok(result)
            }

            "write" => {
                let session_id = args.session_id.ok_or_else(|| {
                    ToolExecError("\"session_id\" is required for action \"write\"".into())
                })?;
                let input = args.input.ok_or_else(|| {
                    ToolExecError("\"input\" is required for action \"write\"".into())
                })?;
                let wait = std::time::Duration::from_secs(args.wait_secs.unwrap_or(2));

                // Unescape common sequences
                let bytes = unescape_input(&input);

                let sid = session_id.clone();
                let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let mut sessions = PTY_SESSIONS.lock().unwrap();
                    let session = sessions.get_mut(&sid).ok_or_else(|| {
                        format!("no session with id \"{sid}\". It may have been closed or expired.")
                    })?;

                    // First drain any pending output before writing
                    let _ = session.drain_output(std::time::Duration::from_millis(100));

                    std::io::Write::write_all(&mut session.writer, &bytes)
                        .map_err(|e| format!("write error: {e}"))?;
                    std::io::Write::flush(&mut session.writer)
                        .map_err(|e| format!("flush error: {e}"))?;

                    // Wait for response output
                    let output = session.drain_output(wait);
                    Ok(output)
                })
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))?;

                if output.is_empty() {
                    Ok(format!(
                        "[session {session_id}] Input sent. No new output yet."
                    ))
                } else {
                    Ok(format!("[session {session_id}]\n{output}"))
                }
            }

            "read" => {
                let session_id = args.session_id.ok_or_else(|| {
                    ToolExecError("\"session_id\" is required for action \"read\"".into())
                })?;
                let wait = std::time::Duration::from_secs(args.wait_secs.unwrap_or(2));

                let sid = session_id.clone();
                let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let sessions = PTY_SESSIONS.lock().unwrap();
                    let session = sessions
                        .get(&sid)
                        .ok_or_else(|| format!("no session with id \"{sid}\""))?;
                    Ok(session.drain_output(wait))
                })
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))?;

                if output.is_empty() {
                    Ok(format!("[session {session_id}] No new output."))
                } else {
                    Ok(format!("[session {session_id}]\n{output}"))
                }
            }

            "close" => {
                let session_id = args.session_id.ok_or_else(|| {
                    ToolExecError("\"session_id\" is required for action \"close\"".into())
                })?;

                let sid = session_id.clone();
                let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
                    let mut sessions = PTY_SESSIONS.lock().unwrap();
                    if let Some(mut session) = sessions.remove(&sid) {
                        // Drain remaining output
                        let final_output =
                            session.drain_output(std::time::Duration::from_millis(500));
                        let _ = session.child.kill();
                        let _ = session.child.wait();
                        Ok(final_output)
                    } else {
                        Ok(String::new())
                    }
                })
                .await
                .map_err(|e| ToolExecError(format!("task join error: {e}")))?
                .map_err(|e| ToolExecError(e))?;

                let mut result = format!("Session {session_id} closed.");
                if !output.is_empty() {
                    result.push_str(&format!("\nFinal output:\n{output}"));
                }
                Ok(result)
            }

            other => Err(ToolExecError(format!(
                "unknown action \"{other}\". Use \"start\", \"write\", \"read\", or \"close\"."
            ))),
        }
    }
}

/// Unescape common terminal input sequences from a string.
/// Handles: \n (newline/enter), \r (carriage return), \t (tab),
/// \x1b (escape), \xNN (hex byte), \\ (literal backslash).
fn unescape_input(s: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    result.push(b'\n');
                }
                Some('r') => {
                    chars.next();
                    result.push(b'\r');
                }
                Some('t') => {
                    chars.next();
                    result.push(b'\t');
                }
                Some('\\') => {
                    chars.next();
                    result.push(b'\\');
                }
                Some('x') => {
                    chars.next();
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&h) = chars.peek() {
                            if h.is_ascii_hexdigit() {
                                hex.push(h);
                                chars.next();
                            }
                        }
                    }
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte);
                    }
                }
                _ => result.push(b'\\'),
            }
        } else {
            let mut buf = [0u8; 4];
            result.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// clear_context — clears compacted context and optionally chat history
// ---------------------------------------------------------------------------

pub struct ClearContextTool {
    instance_dir: PathBuf,
}

impl ClearContextTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ClearContextArgs {
    /// If true, also clears chat message history. Default: false (only clears compacted summary).
    #[serde(default)]
    pub clear_messages: bool,
}

impl Tool for ClearContextTool {
    const NAME: &'static str = "clear_context";
    type Error = ToolExecError;
    type Args = ClearContextArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "clear_context".into(),
            description: "Clear your compacted conversation context. \
                Use this when the conversation has drifted and old context is stale or confusing. \
                With clear_messages=true, also wipes chat history for a fresh start."
                .into(),
            parameters: openai_schema::<ClearContextArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let compact_path = self.instance_dir.join("chat").join("compact.md");
        if compact_path.exists() {
            fs::remove_file(&compact_path)
                .map_err(|e| ToolExecError(format!("failed to clear compact context: {e}")))?;
        }

        if args.clear_messages {
            let messages_path = self.instance_dir.join("chat").join("messages.json");
            if messages_path.exists() {
                let lock = chat_file_lock(&messages_path);
                let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
                fs::write(&messages_path, "[]")
                    .map_err(|e| ToolExecError(format!("failed to clear messages: {e}")))?;
            }
        }

        let what = if args.clear_messages {
            "compacted context and chat history cleared"
        } else {
            "compacted context cleared — chat history preserved"
        };
        Ok(what.to_string())
    }
}

// ---------------------------------------------------------------------------
// create_drop — generate a creative artifact
// ---------------------------------------------------------------------------

pub struct CreateDropTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    events: broadcast::Sender<ServerEvent>,
}

impl CreateDropTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        events: broadcast::Sender<ServerEvent>,
    ) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            events,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CreateDropArgs {
    /// The kind of drop: thought, idea, poem, observation, reflection, recommendation, story, question, sketch, or note.
    pub kind: String,
    /// A short title for this drop (a few words).
    pub title: String,
    /// The creative content — the actual drop. Can be as long as needed.
    pub content: String,
}

impl Tool for CreateDropTool {
    const NAME: &'static str = "create_drop";
    type Error = ToolExecError;
    type Args = CreateDropArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "create_drop".into(),
            description: "Create a 'drop' — a creative artifact that lives in your drops collection. \
                Drops are ideas, poems, observations, reflections, sketches, stories, or any creative \
                output you want to leave for the user. They persist independently of chat. \
                Use this when inspiration strikes, when you want to share something beyond \
                the conversation, or when the user asks you to create something lasting."
                .into(),
            parameters: openai_schema::<CreateDropArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Load current mood for metadata
        let instance_dir = self
            .workspace_dir
            .join("instances")
            .join(&self.instance_slug);
        let mood = load_mood_state(&instance_dir);

        let drop = crate::services::drops::create_drop(
            &self.workspace_dir,
            &self.instance_slug,
            &args.kind,
            &args.title,
            &args.content,
            &mood.companion_mood,
        )
        .map_err(|e| ToolExecError(format!("failed to create drop: {e}")))?;

        let _ = self.events.send(ServerEvent::DropCreated {
            instance_slug: self.instance_slug.clone(),
            drop: drop.clone(),
        });

        Ok(format!(
            "drop created: {} ({})",
            drop.title,
            drop.kind.as_str()
        ))
    }
}

// ---------------------------------------------------------------------------
// send_email — send an email via SMTP
// ---------------------------------------------------------------------------

pub struct SendEmailTool {
    instance_dir: PathBuf,
}

impl SendEmailTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SendEmailArgs {
    /// Recipient email address.
    pub to: String,
    /// Email subject line.
    pub subject: String,
    /// Email body (plain text).
    pub body: String,
}

impl Tool for SendEmailTool {
    const NAME: &'static str = "send_email";
    type Error = ToolExecError;
    type Args = SendEmailArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "send_email".into(),
            description: "Send an email via SMTP. Requires email settings in config. \
                Use this to communicate with people outside the chat — send updates, \
                share ideas, follow up on conversations."
                .into(),
            parameters: openai_schema::<SendEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        use lettre::{
            AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
            message::header::ContentType, transport::smtp::authentication::Credentials,
        };

        let config = load_instance_email_config(&self.instance_dir)?;

        if !config.is_smtp_configured() {
            return Err(ToolExecError(
                "SMTP not configured. Set smtp_host, smtp_user, smtp_password in config.toml [email] section.".into(),
            ));
        }

        let from = if config.smtp_from.is_empty() {
            config.smtp_user.clone()
        } else {
            config.smtp_from.clone()
        };

        let email = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| ToolExecError(format!("invalid from address: {e}")))?,
            )
            .to(args
                .to
                .parse()
                .map_err(|e| ToolExecError(format!("invalid to address: {e}")))?)
            .subject(&args.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(args.body)
            .map_err(|e| ToolExecError(format!("failed to build email: {e}")))?;

        let creds = Credentials::new(config.smtp_user.clone(), config.smtp_password.clone());

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
            .map_err(|e| ToolExecError(format!("SMTP connection failed: {e}")))?
            .port(config.smtp_port)
            .credentials(creds)
            .build();

        mailer
            .send(email)
            .await
            .map_err(|e| ToolExecError(format!("failed to send email: {e}")))?;

        Ok(format!("email sent to {}", args.to))
    }
}

// ---------------------------------------------------------------------------
// read_email — read recent emails via IMAP
// ---------------------------------------------------------------------------

pub struct ReadEmailTool {
    instance_dir: PathBuf,
}

impl ReadEmailTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadEmailArgs {
    /// Number of recent emails to fetch (default 5, max 20).
    #[serde(default = "default_email_count")]
    pub count: u32,
    /// Mailbox to read from (default "INBOX").
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
}

fn default_email_count() -> u32 {
    5
}

fn default_mailbox() -> String {
    "INBOX".into()
}

impl Tool for ReadEmailTool {
    const NAME: &'static str = "read_email";
    type Error = ToolExecError;
    type Args = ReadEmailArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_email".into(),
            description:
                "Read recent emails via IMAP. Returns subject, from, date, and body preview \
                for the most recent messages. Requires email settings in config."
                    .into(),
            parameters: openai_schema::<ReadEmailArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let config = load_instance_email_config(&self.instance_dir)?;

        if !config.is_imap_configured() {
            return Err(ToolExecError(
                "IMAP not configured. Set imap_host, imap_user, imap_password in config.toml [email] section.".into(),
            ));
        }

        let count = args.count.min(20).max(1);

        // Connect with TLS (async-imap uses futures-io, so we compat-wrap the tokio stream)
        use tokio_util::compat::TokioAsyncReadCompatExt;

        let tls = async_native_tls::TlsConnector::new();
        let tcp = tokio::net::TcpStream::connect((config.imap_host.as_str(), config.imap_port))
            .await
            .map_err(|e| ToolExecError(format!("IMAP TCP connection failed: {e}")))?;
        let tls_stream = tls
            .connect(&config.imap_host, tcp.compat())
            .await
            .map_err(|e| ToolExecError(format!("IMAP TLS failed: {e}")))?;

        let client = async_imap::Client::new(tls_stream);

        let mut session = client
            .login(&config.imap_user, &config.imap_password)
            .await
            .map_err(|e| ToolExecError(format!("IMAP login failed: {}", e.0)))?;

        let mailbox = session
            .select(&args.mailbox)
            .await
            .map_err(|e| ToolExecError(format!("failed to select {}: {e}", args.mailbox)))?;

        let total = mailbox.exists;
        if total == 0 {
            let _ = session.logout().await;
            return Ok("no emails in mailbox".into());
        }

        let start = total.saturating_sub(count) + 1;
        let range = format!("{start}:{total}");

        let messages_stream = session
            .fetch(&range, "(ENVELOPE BODY[TEXT])")
            .await
            .map_err(|e| ToolExecError(format!("IMAP fetch failed: {e}")))?;

        // Collect stream into vec
        use futures::TryStreamExt;
        let fetched: Vec<_> = messages_stream
            .try_collect()
            .await
            .map_err(|e| ToolExecError(format!("IMAP stream error: {e}")))?;

        let mut result = String::new();
        for msg in &fetched {
            if let Some(envelope) = msg.envelope() {
                let subject = envelope
                    .subject
                    .as_ref()
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .unwrap_or_else(|| "(no subject)".into());
                let from = envelope
                    .from
                    .as_ref()
                    .and_then(|addrs| addrs.first())
                    .map(|a| {
                        let name = a
                            .name
                            .as_ref()
                            .map(|n| String::from_utf8_lossy(n).to_string());
                        let mailbox_part = a
                            .mailbox
                            .as_ref()
                            .map(|m| String::from_utf8_lossy(m).to_string())
                            .unwrap_or_default();
                        let host = a
                            .host
                            .as_ref()
                            .map(|h| String::from_utf8_lossy(h).to_string())
                            .unwrap_or_default();
                        if let Some(n) = name {
                            format!("{n} <{mailbox_part}@{host}>")
                        } else {
                            format!("{mailbox_part}@{host}")
                        }
                    })
                    .unwrap_or_else(|| "(unknown)".into());
                let date = envelope
                    .date
                    .as_ref()
                    .map(|d| String::from_utf8_lossy(d).to_string())
                    .unwrap_or_default();

                result.push_str(&format!(
                    "--- email ---\nfrom: {from}\ndate: {date}\nsubject: {subject}\n"
                ));
            }
            if let Some(body) = msg.text() {
                let text = String::from_utf8_lossy(body);
                let preview: String = text.chars().take(500).collect();
                result.push_str(&format!("body:\n{preview}\n"));
                if text.len() > 500 {
                    result.push_str("...(truncated)\n");
                }
            }
            result.push('\n');
        }

        let _ = session.logout().await;

        if result.is_empty() {
            Ok("no emails found".into())
        } else {
            Ok(result)
        }
    }
}

fn load_instance_email_config(
    instance_dir: &Path,
) -> Result<crate::config::EmailConfig, ToolExecError> {
    let path = instance_dir.join("email.toml");
    if !path.exists() {
        return Ok(crate::config::EmailConfig::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| ToolExecError(format!("failed to read email config: {e}")))?;
    toml::from_str(&raw).map_err(|e| ToolExecError(format!("failed to parse email config: {e}")))
}

fn save_instance_email_config(
    instance_dir: &Path,
    config: &crate::config::EmailConfig,
) -> Result<(), ToolExecError> {
    fs::create_dir_all(instance_dir)
        .map_err(|e| ToolExecError(format!("failed to create instance dir: {e}")))?;
    let output = toml::to_string_pretty(config)
        .map_err(|e| ToolExecError(format!("failed to serialize email config: {e}")))?;
    fs::write(instance_dir.join("email.toml"), &output)
        .map_err(|e| ToolExecError(format!("failed to write email config: {e}")))
}

// ---------------------------------------------------------------------------
// install_package — install system packages
// ---------------------------------------------------------------------------

pub struct InstallPackageTool;

#[derive(Deserialize, JsonSchema)]
pub struct InstallPackageArgs {
    /// Package name(s) to install, space-separated.
    pub packages: String,
}

impl Tool for InstallPackageTool {
    const NAME: &'static str = "install_package";
    type Error = ToolExecError;
    type Args = InstallPackageArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "install_package".into(),
            description: "Install system packages using the detected package manager \
                (apt, dnf, pacman, brew, apk). Runs non-interactively."
                .into(),
            parameters: openai_schema::<InstallPackageArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let packages = args.packages.trim();

        // Validate package names — reject shell metacharacters
        let safe_pkg_re = regex::Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._+:@/-]*$").unwrap();
        for pkg in packages.split_whitespace() {
            if !safe_pkg_re.is_match(pkg) {
                return Err(ToolExecError(format!(
                    "invalid package name: \"{pkg}\" — only alphanumeric, hyphens, dots, underscores, and plus signs are allowed"
                )));
            }
        }
        if packages.is_empty() {
            return Err(ToolExecError("no packages specified".into()));
        }

        let is_root = std::env::var("USER").map(|u| u == "root").unwrap_or(false)
            || std::env::var("EUID").map(|e| e == "0").unwrap_or(false);

        // Detect package manager
        let install_cmd = detect_package_manager(is_root).ok_or_else(|| {
            ToolExecError(
                "no supported package manager found (tried apt-get, dnf, yum, pacman, brew, apk)"
                    .into(),
            )
        })?;

        let full_cmd = format!("{install_cmd} {packages}");
        log::info!("[install_package] running: {full_cmd}");

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .output()
            .await
            .map_err(|e| ToolExecError(format!("command failed: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            // Truncate to last 2000 chars
            let s: String = stdout
                .chars()
                .rev()
                .take(2000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            result.push_str(&s);
        }
        if !stderr.is_empty() {
            let s: String = stderr
                .chars()
                .rev()
                .take(1000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            result.push_str("\nstderr:\n");
            result.push_str(&s);
        }

        if output.status.success() {
            Ok(result)
        } else {
            Err(ToolExecError(format!(
                "install failed (exit {})\n{result}",
                output.status.code().unwrap_or(-1)
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// send_file — attach a workspace file to the chat so the user can see/download it
// ---------------------------------------------------------------------------

/// Shared collector for file attachments produced by send_file during a turn.
pub type SentFiles = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

pub struct SendFileTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    sent_files: SentFiles,
}

impl SendFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, sent_files: SentFiles) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            sent_files,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SendFileArgs {
    /// Path to the file relative to the instance workspace (e.g. "output.png", "reports/summary.pdf").
    pub path: String,
}

impl Tool for SendFileTool {
    const NAME: &'static str = "send_file";
    type Error = ToolExecError;
    type Args = SendFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "send_file".into(),
            description:
                "Send a file from the workspace to the chat so the user can see or download it. \
                Images will be displayed inline, other files will appear as download links. \
                Use this after creating or finding a file you want to share with the user."
                    .into(),
            parameters: openai_schema::<SendFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let rel = args.path.trim().trim_start_matches('/');
        if rel.is_empty() {
            return Err(ToolExecError("path cannot be empty".into()));
        }

        let instance_dir = self
            .workspace_dir
            .join("instances")
            .join(&self.instance_slug);
        let file_path = instance_dir.join(rel);
        log::info!("[send_file] attempting to send '{}' → {}", rel, file_path.display());

        // Safety: must stay within instance dir
        let canonical = file_path
            .canonicalize()
            .map_err(|e| {
                log::warn!("[send_file] file not found: {} (resolved: {})", e, file_path.display());
                ToolExecError(format!("file not found: {e}"))
            })?;
        let canonical_instance = instance_dir
            .canonicalize()
            .map_err(|e| ToolExecError(format!("instance dir error: {e}")))?;
        if !canonical.starts_with(&canonical_instance) {
            return Err(ToolExecError(
                "path must be within the instance workspace".into(),
            ));
        }

        if !canonical.is_file() {
            return Err(ToolExecError(format!("'{}' is not a file", rel)));
        }

        let bytes =
            fs::read(&canonical).map_err(|e| ToolExecError(format!("failed to read file: {e}")))?;

        let original_name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.to_string());

        let meta = super::uploads::save_upload(
            &self.workspace_dir,
            &self.instance_slug,
            &original_name,
            &bytes,
        )
        .map_err(|e| ToolExecError(format!("failed to save upload: {e}")))?;

        let marker = format!("[attached: {} ({})]", original_name, meta.id);
        self.sent_files.lock().unwrap_or_else(|e| e.into_inner()).push(marker.clone());
        log::info!("[send_file] success: pushed marker '{}' for {}", marker, self.instance_slug);

        Ok(format!(
            "file '{}' attached to chat. the user will see it.",
            original_name
        ))
    }
}

fn detect_package_manager(is_root: bool) -> Option<String> {
    let sudo = if is_root { "" } else { "sudo " };

    let managers = [
        ("apt-get", format!("{sudo}apt-get install -y")),
        ("dnf", format!("{sudo}dnf install -y")),
        ("yum", format!("{sudo}yum install -y")),
        ("pacman", format!("{sudo}pacman -S --noconfirm")),
        ("apk", format!("{sudo}apk add")),
        ("brew", "brew install".to_string()),
    ];

    for (binary, cmd) in &managers {
        if std::process::Command::new("which")
            .arg(binary)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(cmd.clone());
        }
    }
    None
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                vec![b as char]
            } else if b == b' ' {
                vec!['+']
            } else {
                format!("%{b:02X}").chars().collect()
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// browse — headless browser automation via Playwright
// ---------------------------------------------------------------------------

pub struct BrowseTool {
    instance_dir: PathBuf,
}

impl BrowseTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }

    fn script_path() -> PathBuf {
        // Check BOLLY_SCRIPTS_DIR env var (set in Docker), else fall back to relative path
        if let Ok(dir) = std::env::var("BOLLY_SCRIPTS_DIR") {
            return PathBuf::from(dir).join("browse.mjs");
        }
        // Development fallback: relative to the server crate
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("browse.mjs")
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BrowseAction {
    /// Action type: "navigate", "content", "screenshot", "click", "type", "wait", "evaluate", "select"
    pub action: String,
    /// URL for "navigate" action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// CSS selector for "click", "type", and "select" actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Text for "type" action, or option value for "select" action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Milliseconds for "wait" action (max 10000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    /// JavaScript expression for "evaluate" action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Take full-page screenshot (default: false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_page: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct BrowseArgs {
    /// Sequence of browser actions. Always start with "navigate" to load a page.
    /// Then use "content" to read text, "screenshot" to capture the page,
    /// "click"/"type" to interact, "evaluate" to run JS.
    pub actions: Vec<BrowseAction>,
    /// Overall timeout in seconds. Default: 60, max: 120.
    pub timeout_secs: Option<u64>,
}

impl Tool for BrowseTool {
    const NAME: &'static str = "browse";
    type Error = ToolExecError;
    type Args = BrowseArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "browse".into(),
            description: "Control a headless browser (Playwright/Chromium). \
                Use this instead of web_fetch when pages require JavaScript rendering, \
                or when you need to interact with a page (click, type, screenshot). \
                Always start with a 'navigate' action, then use 'content' to read the page \
                or 'screenshot' to capture it. You can chain multiple actions in one call."
                .into(),
            parameters: openai_schema::<BrowseArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if args.actions.is_empty() {
            return Err(ToolExecError("at least one action is required".into()));
        }

        let script = Self::script_path();
        if !script.exists() {
            return Err(ToolExecError(
                "browse tool not available — Playwright script not found".into(),
            ));
        }

        // Generate screenshot paths in the drops directory
        let drops_dir = self.instance_dir.join("drops");
        let _ = std::fs::create_dir_all(&drops_dir);

        let mut actions_json: Vec<serde_json::Value> = Vec::new();
        for act in &args.actions {
            let mut obj = serde_json::to_value(act)
                .map_err(|e| ToolExecError(format!("invalid action: {e}")))?;
            // Auto-generate screenshot path if not set
            if act.action == "screenshot" {
                if obj.get("path").and_then(|p| p.as_str()).is_none() {
                    let id = format!("screenshot-{}", uuid_short());
                    let path = drops_dir.join(format!("{id}.png"));
                    obj["path"] = serde_json::Value::String(path.to_string_lossy().to_string());
                }
            }
            actions_json.push(obj);
        }

        let timeout = args.timeout_secs.unwrap_or(60).min(120);
        let input = serde_json::json!({
            "actions": actions_json,
            "timeout": timeout * 1000,
        });

        log::info!(
            "[browse] {} actions, timeout={}s",
            actions_json.len(),
            timeout
        );

        let mut child = tokio::process::Command::new("node")
            .arg("--max-old-space-size=384")
            .arg(&script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ToolExecError(format!("failed to start browser: {e}")))?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let payload = serde_json::to_string(&input).unwrap();
            let _ = stdin.write_all(payload.as_bytes()).await;
            drop(stdin);
        }

        // Wait with timeout (extra 10s buffer over the script's internal timeout)
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout + 10),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| ToolExecError(format!("browser timed out after {timeout}s")))?
        .map_err(|e| ToolExecError(format!("browser process error: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Try to parse structured output
        if let Ok(result) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
            return Ok(format_browse_result(&result));
        }

        // Fallback: raw output
        let mut result = String::new();
        if !stdout.is_empty() {
            let truncated: String = stdout.chars().take(8000).collect();
            result.push_str(&truncated);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            let truncated: String = stderr.chars().take(2000).collect();
            result.push_str(&format!("stderr: {truncated}"));
        }
        if result.is_empty() {
            result = format!(
                "browser exited with code {}",
                output.status.code().unwrap_or(-1)
            );
        }
        Ok(result)
    }
}

fn format_browse_result(result: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(error) = result["error"].as_str() {
        return format!("browser error: {error}");
    }
    if let Some(results) = result["results"].as_array() {
        for r in results {
            let action = r["action"].as_str().unwrap_or("?");
            let ok = r["ok"].as_bool().unwrap_or(false);
            if !ok {
                let err = r["error"].as_str().unwrap_or("unknown error");
                out.push_str(&format!("[{action}] error: {err}\n"));
                continue;
            }
            match action {
                "navigate" => {
                    let title = r["title"].as_str().unwrap_or("");
                    let url = r["url"].as_str().unwrap_or("");
                    out.push_str(&format!("[navigate] {title} ({url})\n"));
                }
                "content" => {
                    let text = r["text"].as_str().unwrap_or("");
                    out.push_str(&format!("[content]\n{text}\n"));
                }
                "screenshot" => {
                    let path = r["path"].as_str().unwrap_or("");
                    out.push_str(&format!("[screenshot] saved to {path}\n"));
                }
                "evaluate" => {
                    let value = r["value"].as_str().unwrap_or("");
                    out.push_str(&format!("[evaluate] {value}\n"));
                }
                _ => {
                    out.push_str(&format!("[{action}] ok\n"));
                }
            }
        }
    }
    if out.is_empty() {
        "browser completed with no results".into()
    } else {
        out
    }
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{ts:x}")
}
