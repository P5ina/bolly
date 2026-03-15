use std::{
    collections::HashMap,
    fmt, fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

use crate::services::tool::{ToolDefinition, ToolDyn, ToolError};
use schemars::JsonSchema;
use tokio::sync::broadcast;

use regex::Regex;

use crate::domain::events::ServerEvent;

// Sub-modules
pub mod calendar;
pub mod communication;
pub mod companion;
pub mod drive;
pub mod files;
pub mod memory_tools;
pub mod project;
pub mod skills;
pub mod system;
pub mod video;
pub mod web;

// Re-export public items so external code uses `tools::FooTool` paths
pub use calendar::{CreateEventTool, ListEventsTool};
pub use communication::{
    ReachOutTool, ReadEmailTool, ScheduleMessageTool, ScheduledMessage, SendEmailTool,
};
pub use companion::{
    ALLOWED_MOODS, EditSoulTool, load_mood_state, save_mood_state,
};
pub use drive::{ListDriveFilesTool, ReadDriveFileTool, UploadDriveFileTool};
pub use files::{EditFileTool, ListFilesTool, ReadFileTool, SendFileTool, WriteFileTool};
pub use memory_tools::{MemoryForgetTool, MemoryListTool, MemoryReadTool, MemoryWriteTool};
pub use project::{TaskItem, TaskStatus};
pub use skills::{ActivateSkillTool, ListSkillsTool, ReadSkillReferenceTool};
pub use system::{
    ClearContextTool, CreateDropTool, ExploreCodeTool, GetSettingsTool,
    InteractiveSessionTool, RequestSecretTool, RunCommandTool, SearchCodeTool, UpdateConfigTool,
};
pub use video::WatchVideoTool;
pub use web::{BrowseTool, WebFetchTool, WebSearchTool};

// ---------------------------------------------------------------------------
// Cached tool definitions snapshot (populated by build_tools, read by stats)
// ---------------------------------------------------------------------------

/// Snapshot of tool definition info, updated every time build_tools runs.
#[derive(Clone, Default)]
pub struct ToolDefsSnapshot {
    pub names: Vec<String>,
    pub total_json_chars: usize,
}

static TOOL_DEFS_CACHE: OnceLock<Mutex<ToolDefsSnapshot>> = OnceLock::new();

fn tool_defs_cache() -> &'static Mutex<ToolDefsSnapshot> {
    TOOL_DEFS_CACHE.get_or_init(|| Mutex::new(ToolDefsSnapshot::default()))
}

/// Read the latest cached tool definitions snapshot.
pub fn cached_tool_defs() -> ToolDefsSnapshot {
    tool_defs_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Compute and cache tool definition info from actual tool instances.
/// Call this after `build_tools` to keep the cache up to date.
pub async fn cache_tool_defs(tools: &[Box<dyn ToolDyn>]) {
    let mut names = Vec::with_capacity(tools.len());
    let mut total_json_chars = 0usize;
    for tool in tools {
        let def = tool.definition(String::new()).await;
        names.push(def.name.clone());
        total_json_chars += serde_json::to_string(&def).map(|s| s.len()).unwrap_or(0);
    }
    let mut cache = tool_defs_cache().lock().unwrap_or_else(|e| e.into_inner());
    *cache = ToolDefsSnapshot {
        names,
        total_json_chars,
    };
}

// ---------------------------------------------------------------------------
// Per-chat file lock
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
// Secret redaction
// ---------------------------------------------------------------------------

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
            "GITHUB_TOKEN",
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
    let patterns = [
        r#"sk-ant-api03-[A-Za-z0-9_\-]{80,}"#,
        r#"sk-ant-[A-Za-z0-9_\-]{20,}"#,
        r#"sk-proj-[A-Za-z0-9_\-]{20,}"#,
        r"sk-[A-Za-z0-9]{20,}",
        r"ghp_[A-Za-z0-9]{36,}",
        r"github_pat_[A-Za-z0-9_]{80,}",
        r"gho_[A-Za-z0-9]{36,}",
        r#"postgresql://[^\s"']+[^\s"'.]"#,
        r#"postgres://[^\s"']+[^\s"'.]"#,
    ];

    let mut result = text.to_string();

    for pat in &patterns {
        if let Ok(re) = Regex::new(pat) {
            result = re.replace_all(&result, "[REDACTED]").to_string();
        }
    }

    for secret in secret_values() {
        result = result.replace(secret.as_str(), "[REDACTED]");
    }

    result
}

// ---------------------------------------------------------------------------
// OpenAI-compatible schema helper
// ---------------------------------------------------------------------------

pub(crate) fn openai_schema<T: JsonSchema>() -> serde_json::Value {
    let mut val = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    if let Some(obj) = val.as_object_mut() {
        obj.remove("$schema");
        obj.remove("$id");
        obj.remove("title");
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
        "edit_file" => format!("editing {}", v["path"].as_str().unwrap_or("?")),
        "list_files" => format!("listing {}", v["path"].as_str().unwrap_or(".")),
        "search_code" => format!("searching '{}'", v["query"].as_str().unwrap_or("?")),
        "run_command" => {
            let cmd = v["command"].as_str().unwrap_or("?");
            format!("$ {cmd}")
        }
        "edit_soul" => "rewriting soul.md".into(),
        "set_mood" => format!("mood → {}", v["mood"].as_str().unwrap_or("?")),
        "remember" => "storing a memory".into(),
        "recall" => format!("recalling '{}'", v["query"].as_str().unwrap_or("?")),
        "web_search" => format!("web search: {}", v["query"].as_str().unwrap_or("?")),
        "web_fetch" => format!("fetching {}", v["url"].as_str().unwrap_or("?")),
        "update_config" => "updating config".into(),
        "create_drop" => format!("creating drop: {}", v["title"].as_str().unwrap_or("?")),
        "get_settings" => "reading current settings".into(),
        "send_email" => {
            let to = v["to"].as_str().unwrap_or("?");
            format!("sending email to {to}")
        }
        "read_email" => {
            let count = v["count"].as_u64().unwrap_or(5);
            format!("reading {count} emails")
        }
        "list_events" => {
            let days = v["days_ahead"].as_u64().unwrap_or(7);
            format!("listing calendar events ({days} days)")
        }
        "create_event" => format!("creating event: {}", v["summary"].as_str().unwrap_or("?")),
        "list_drive_files" => "listing drive files".into(),
        "read_drive_file" => format!(
            "reading drive file {}",
            v["file_id"].as_str().unwrap_or("?")
        ),
        "upload_drive_file" => format!("uploading {}", v["name"].as_str().unwrap_or("?")),
        "request_secret" => format!("requesting secret: {}", v["prompt"].as_str().unwrap_or("?")),
        "read_skill_reference" => format!(
            "reading skill ref {}/{}",
            v["skill_id"].as_str().unwrap_or("?"),
            v["filename"].as_str().unwrap_or("?")
        ),
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
// ObservableTool
// ---------------------------------------------------------------------------

pub struct ObservableTool {
    inner: Box<dyn ToolDyn>,
    events: broadcast::Sender<ServerEvent>,
    workspace_dir: PathBuf,
    instance_slug: String,
    chat_id: String,
    mcp_snapshot: Option<crate::services::mcp::McpAppSnapshot>,
}

impl ObservableTool {
    pub fn new(
        inner: Box<dyn ToolDyn>,
        events: broadcast::Sender<ServerEvent>,
        workspace_dir: &Path,
        instance_slug: String,
        chat_id: String,
        mcp_snapshot: Option<crate::services::mcp::McpAppSnapshot>,
    ) -> Self {
        Self {
            inner,
            events,
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug,
            chat_id,
            mcp_snapshot,
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

        let start_msg = crate::domain::chat::ChatMessage {
            id: format!("tool_{}_{}", tool_call_counter(), unix_millis()),
            role: crate::domain::chat::ChatRole::Assistant,
            content: summary.clone(),
            created_at: unix_millis().to_string(),
            kind: crate::domain::chat::MessageKind::ToolCall,
            tool_name: Some(tool_name.clone()),
            mcp_app_html: None,
            mcp_app_input: None,
        };
        append_message_to_chat(
            &self.workspace_dir,
            &self.instance_slug,
            &self.chat_id,
            &start_msg,
        );
        let _ = self.events.send(ServerEvent::ChatMessageCreated {
            instance_slug: self.instance_slug.clone(),
            chat_id: self.chat_id.clone(),
            message: start_msg,
        });

        // Emit MCP App BEFORE the tool call so the viewer appears immediately
        let mut mcp_app_msg_id: Option<String> = None;
        if let Some(ref snapshot) = self.mcp_snapshot {
            if snapshot.is_app_tool(&tool_name) {
                if let Some(html) = snapshot.get_app_html(&tool_name) {
                    let msg_id = format!("mcp_app_{}_{}", tool_call_counter(), unix_millis());
                    mcp_app_msg_id = Some(msg_id.clone());
                    let app_msg = crate::domain::chat::ChatMessage {
                        id: msg_id,
                        role: crate::domain::chat::ChatRole::Assistant,
                        content: String::new(), // result not yet available
                        created_at: unix_millis().to_string(),
                        kind: crate::domain::chat::MessageKind::McpApp,
                        tool_name: Some(tool_name.clone()),
                        mcp_app_html: Some(html),
                        mcp_app_input: Some(args.clone()),
                    };
                    append_message_to_chat(
                        &self.workspace_dir,
                        &self.instance_slug,
                        &self.chat_id,
                        &app_msg,
                    );
                    let _ = self.events.send(ServerEvent::ChatMessageCreated {
                        instance_slug: self.instance_slug.clone(),
                        chat_id: self.chat_id.clone(),
                        message: app_msg,
                    });
                }
            }
        }

        let events = self.events.clone();
        let workspace_dir = self.workspace_dir.clone();
        let instance_slug = self.instance_slug.clone();
        let chat_id = self.chat_id.clone();
        let fut = self.inner.call(args);
        Box::pin(async move {
            const MAX_TOOL_RESULT: usize = 12_000;
            let result = match fut.await {
                Ok(s) => {
                    let redacted = redact_secrets(&s);
                    if redacted.len() > MAX_TOOL_RESULT {
                        let truncated: String = redacted.chars().take(MAX_TOOL_RESULT).collect();
                        Ok(format!(
                            "{truncated}\n\n...(tool output truncated at {MAX_TOOL_RESULT} chars, total: {})",
                            redacted.len()
                        ))
                    } else {
                        Ok(redacted)
                    }
                }
                Err(e) => Err(e),
            };
            // Send tool result to the MCP App viewer
            if let Some(msg_id) = mcp_app_msg_id {
                let tool_output = match &result {
                    Ok(s) => s.clone(),
                    Err(e) => format!("error: {e}"),
                };
                // Update the persisted message with the result
                update_mcp_app_result(
                    &workspace_dir,
                    &instance_slug,
                    &chat_id,
                    &msg_id,
                    &tool_output,
                );
                let _ = events.send(ServerEvent::McpAppResult {
                    instance_slug: instance_slug.clone(),
                    chat_id: chat_id.clone(),
                    message_id: msg_id,
                    tool_output,
                });
            }
            if tool_name == "run_command"
                || tool_name == "interactive_session"
                || tool_name == "send_file"
            {
                let output = match &result {
                    Ok(s) => s.clone(),
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
                        mcp_app_html: None,
                        mcp_app_input: None,
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

/// Build tools gated by category. Core is always loaded; others depend on triage.
pub fn build_tools(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    brave_api_key: Option<&str>,
    config_path: &Path,
    events: broadcast::Sender<ServerEvent>,
    llm: &crate::services::llm::LlmBackend,
    pending_secrets: Option<
        Arc<
            tokio::sync::Mutex<std::collections::HashMap<String, crate::app::state::PendingSecret>>,
        >,
    >,
    plan: &str,
    google: Option<crate::services::google::GoogleClient>,
    email_accounts: Vec<crate::config::EmailConfig>,
    sent_files: SentFiles,
    mcp_snapshot: Option<crate::services::mcp::McpAppSnapshot>,
    mcp_tools: Vec<Box<dyn ToolDyn>>,
    openrouter_key: &str,
) -> (Vec<Box<dyn ToolDyn>>, SentFiles) {
    let snap = mcp_snapshot;
    let wrap = |tool: Box<dyn ToolDyn>| -> Box<dyn ToolDyn> {
        Box::new(ObservableTool::new(
            tool,
            events.clone(),
            workspace_dir,
            instance_slug.to_string(),
            chat_id.to_string(),
            snap.clone(),
        ))
    };

    let browser_enabled = matches!(plan, "companion" | "unlimited");

    // ── Core ──
    let mut tools: Vec<Box<dyn ToolDyn>> = vec![
        wrap(Box::new(ReadFileTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(WriteFileTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(EditFileTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(ListFilesTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(MemoryWriteTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(MemoryReadTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(MemoryListTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(MemoryForgetTool::new(workspace_dir, instance_slug))),
        // Mood is managed by background sentiment extraction + heartbeat, not tools.
        wrap(Box::new(EditSoulTool::new(workspace_dir, instance_slug))),
        wrap(Box::new(RunCommandTool::new(workspace_dir, instance_slug, chat_id, events.clone()))),
        wrap(Box::new(ClearContextTool::new(workspace_dir, instance_slug))),
    ];

    // ── System ──
    tools.push(wrap(Box::new(InteractiveSessionTool::new(workspace_dir, instance_slug))));
    tools.push(wrap(Box::new(SendFileTool::new(workspace_dir, instance_slug, sent_files.clone()))));
    tools.push(wrap(Box::new(GetSettingsTool::new(config_path, workspace_dir, instance_slug, google.clone()))));
    tools.push(wrap(Box::new(UpdateConfigTool::new(config_path, workspace_dir, instance_slug))));
    if let Some(ps) = pending_secrets {
        tools.push(wrap(Box::new(RequestSecretTool::new(
            workspace_dir, instance_slug, config_path, events.clone(), ps,
        ))));
    }

    // ── Skills ──
    tools.push(wrap(Box::new(ListSkillsTool::new(workspace_dir))));
    tools.push(wrap(Box::new(ActivateSkillTool::new(workspace_dir))));
    tools.push(wrap(Box::new(ReadSkillReferenceTool::new(workspace_dir))));

    // ── Web ──
    tools.push(wrap(Box::new(WebSearchTool::new(brave_api_key, config_path))));
    tools.push(wrap(Box::new(WebFetchTool)));
    tools.push(wrap(Box::new(WatchVideoTool::new(openrouter_key))));
    if browser_enabled {
        tools.push(wrap(Box::new(BrowseTool::new(workspace_dir, instance_slug))));
    }

    // ── Code ──
    tools.push(wrap(Box::new(SearchCodeTool::new(workspace_dir, instance_slug))));
    tools.push(wrap(Box::new(ExploreCodeTool::new(workspace_dir, instance_slug, llm.clone()))));


    // ── Creative ──
    tools.push(wrap(Box::new(CreateDropTool::new(workspace_dir, instance_slug, events.clone()))));
    tools.push(wrap(Box::new(ScheduleMessageTool::new(workspace_dir, instance_slug))));

    // ── Email (unified: Gmail + SMTP/IMAP) ──
    let has_email = google.is_some() || !email_accounts.is_empty();
    if has_email {
        tools.push(wrap(Box::new(SendEmailTool::new(google.clone(), instance_slug, email_accounts.clone()))));
        tools.push(wrap(Box::new(ReadEmailTool::new(google.clone(), instance_slug, email_accounts))));
    }

    // ── Google (calendar, drive) ──
    if let Some(g) = google {
        tools.push(wrap(Box::new(ListEventsTool::new(g.clone(), instance_slug))));
        tools.push(wrap(Box::new(CreateEventTool::new(g.clone(), instance_slug))));
        tools.push(wrap(Box::new(ListDriveFilesTool::new(g.clone(), instance_slug))));
        tools.push(wrap(Box::new(ReadDriveFileTool::new(g.clone(), instance_slug))));
        tools.push(wrap(Box::new(UploadDriveFileTool::new(g, instance_slug))));
    }


    // MCP tools
    for mcp_tool in mcp_tools {
        tools.push(wrap(mcp_tool));
    }

    log::info!("built {} tools", tools.len());
    (tools, sent_files)
}

// ---------------------------------------------------------------------------
// Shared error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ToolExecError(pub(crate) String);

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

pub fn unix_millis() -> u128 {
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

/// Update an MCP App message's content (tool result) after the tool call completes.
fn update_mcp_app_result(
    workspace_dir: &Path,
    instance_slug: &str,
    chat_id: &str,
    message_id: &str,
    tool_output: &str,
) {
    let chat_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("chats")
        .join(chat_id);
    let path = chat_dir.join("messages.json");

    let lock = chat_file_lock(&path);
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

    let mut messages: Vec<crate::domain::chat::ChatMessage> = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
        msg.content = tool_output.to_string();
    }

    if let Ok(json) = serde_json::to_string_pretty(&messages) {
        let _ = fs::write(&path, json);
    }
}

/// Shared collector for file attachments produced by send_file during a turn.
pub type SentFiles = std::sync::Arc<std::sync::Mutex<Vec<String>>>;
