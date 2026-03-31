use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::broadcast;

use crate::config::{Config, CHEAP_MODEL, DEFAULT_FAST_MODEL, DEFAULT_MODEL};
use crate::domain::chat::{ChatMessage, ChatRole, MessageKind};
use crate::domain::events::ServerEvent;
use crate::services::tool::{ToolDefinition, ToolDyn};

// ═══════════════════════════════════════════════════════════════════════════
// Real input token cache — populated from Anthropic API responses
// ═══════════════════════════════════════════════════════════════════════════

static REAL_INPUT_TOKENS: Mutex<Option<std::collections::HashMap<String, u64>>> = Mutex::new(None);

/// Cache the real input token count from an Anthropic API response.
fn cache_real_input_tokens(instance_slug: &str, chat_id: &str, tokens: u64) {
    let key = format!("{instance_slug}/{chat_id}");
    let mut guard = REAL_INPUT_TOKENS.lock().unwrap();
    guard.get_or_insert_with(std::collections::HashMap::new).insert(key, tokens);
}

/// Retrieve the last real input token count for a given instance/chat.
pub fn get_real_input_tokens(instance_slug: &str, chat_id: &str) -> Option<u64> {
    let key = format!("{instance_slug}/{chat_id}");
    REAL_INPUT_TOKENS.lock().unwrap().as_ref()?.get(&key).copied()
}

// ═══════════════════════════════════════════════════════════════════════════
// Message types — serialize directly to Anthropic API format
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User { content: Vec<ContentBlock> },
    #[serde(rename = "assistant")]
    Assistant { content: Vec<ContentBlock> },
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Message::User {
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Message::Assistant {
            content: vec![ContentBlock::text(text)],
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "document")]
    Document { source: DocumentSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
    #[serde(rename = "compaction")]
    Compaction {
        #[serde(alias = "summary")]
        content: String,
    },
    /// Catch-all for unknown content block types (e.g. from newer API versions).
    /// Preserves the raw JSON so it can be serialized back without data loss.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text { text: text.into() }
    }

    pub fn tool_result(tool_use_id: String, content: String) -> Self {
        // ToolDyn blanket impl wraps String output via serde_json::to_string,
        // which adds JSON quotes: `[...]` becomes `"[...]"`. Unwrap that layer.
        let inner = if content.starts_with('"') && content.ends_with('"') {
            serde_json::from_str::<String>(&content).unwrap_or(content)
        } else {
            content
        };

        // If content is a JSON array of Anthropic content blocks (each with a "type" field), use directly.
        // This allows tools to return image+text results (e.g. screenshots).
        if inner.starts_with('[') {
            if let Ok(blocks) = serde_json::from_str::<serde_json::Value>(&inner) {
                if let Some(arr) = blocks.as_array() {
                    let is_content_blocks = !arr.is_empty()
                        && arr.iter().all(|b| b.get("type").and_then(|t| t.as_str()).is_some());
                    if is_content_blocks {
                        return ContentBlock::ToolResult {
                            tool_use_id,
                            content: blocks,
                        };
                    }
                }
            }
        }

        ContentBlock::ToolResult {
            tool_use_id,
            content: serde_json::Value::String(inner),
        }
    }

}

// ═══════════════════════════════════════════════════════════════════════════
// HistoryEntry — wraps Message with timestamps/IDs for unified history
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    #[serde(flatten)]
    pub message: Message,
    /// Timestamp in millis since epoch (as string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
    /// Stable ID for client dedup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// HTML content for MCP App rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_app_html: Option<String>,
    /// Tool input JSON for MCP App rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_app_input: Option<String>,
    /// Model name used to generate this message (assistant only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl HistoryEntry {
    /// Wrap a Message with timestamp and ID.
    pub fn new(message: Message, ts: String, id: String) -> Self {
        Self { message, ts: Some(ts), id: Some(id), mcp_app_html: None, mcp_app_input: None, model: None }
    }

    /// Extract just the Messages from a slice of entries.
    pub fn to_messages(entries: &[HistoryEntry]) -> Vec<Message> {
        entries.iter().map(|e| e.message.clone()).collect()
    }
}

/// Convert HistoryEntry slice to ChatMessage vec for UI display.
pub fn history_to_chat_messages(entries: &[HistoryEntry]) -> Vec<ChatMessage> {

    let mut out = Vec::new();
    let mut counter = 0u64;
    let mut seen_ids = std::collections::HashSet::new();

    for entry in entries {
        let ts = entry.ts.clone().unwrap_or_else(|| "0".to_string());
        let base_id = entry.id.clone().unwrap_or_else(|| {
            counter += 1;
            format!("h_{counter}")
        });

        let (role, blocks) = match &entry.message {
            Message::User { content } => (ChatRole::User, content),
            Message::Assistant { content } => (ChatRole::Assistant, content),
        };

        let mut block_idx = 0u32;
        for block in blocks {
            let mut block_id = if block_idx == 0 {
                base_id.clone()
            } else {
                format!("{base_id}_{block_idx}")
            };
            block_idx += 1;

            // Ensure uniqueness — append suffix if ID was already emitted
            if !seen_ids.insert(block_id.clone()) {
                let mut dedup = 2u32;
                loop {
                    let candidate = format!("{block_id}_d{dedup}");
                    if seen_ids.insert(candidate.clone()) {
                        block_id = candidate;
                        break;
                    }
                    dedup += 1;
                }
            }

            match block {
                ContentBlock::Text { text } => {
                    if text.is_empty() { continue; }
                    out.push(ChatMessage {
                        id: block_id,
                        role: role.clone(),
                        content: text.clone(),
                        created_at: ts.clone(),
                        kind: MessageKind::Message,
                        tool_name: None,
                        mcp_app_html: None,
                        mcp_app_input: None,
                        model: if role == ChatRole::Assistant { entry.model.clone() } else { None },
                    });
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    let summary = tool_use_summary(name, input);
                    out.push(ChatMessage {
                        id: block_id,
                        role: ChatRole::Assistant,
                        content: summary,
                        created_at: ts.clone(),
                        kind: MessageKind::ToolCall,
                        tool_name: Some(name.clone()),
                        mcp_app_html: entry.mcp_app_html.clone(),
                        mcp_app_input: entry.mcp_app_input.clone(),
                        model: None,
                    });
                }
                ContentBlock::ToolResult { content, .. } => {
                    let text = match content {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    out.push(ChatMessage {
                        id: block_id,
                        role: ChatRole::Assistant,
                        content: text,
                        created_at: ts.clone(),
                        kind: MessageKind::ToolOutput,
                        tool_name: None,
                        mcp_app_html: None,
                        mcp_app_input: None, model: None,
                    });
                }
                ContentBlock::Compaction { content } => {
                    out.push(ChatMessage {
                        id: block_id,
                        role: ChatRole::Assistant,
                        content: content.clone(),
                        created_at: ts.clone(),
                        kind: MessageKind::Compaction,
                        tool_name: None,
                        mcp_app_html: None,
                        mcp_app_input: None, model: None,
                    });
                }
                ContentBlock::Unknown(val) => {
                    // Server tool blocks (web_search, code_execution) — render like regular tools
                    let block_type = val["type"].as_str().unwrap_or("");
                    if block_type == "server_tool_use" {
                        let tool_name = val["name"].as_str().unwrap_or("server_tool");
                        let summary = match tool_name {
                            "web_search" => {
                                let q = val["input"]["query"].as_str().unwrap_or("");
                                if q.is_empty() { "searching the web".into() }
                                else { format!("web search: {q}") }
                            }
                            "web_fetch" => {
                                let u = val["input"]["url"].as_str().unwrap_or("");
                                if u.is_empty() { "fetching web page".into() }
                                else { format!("fetching {u}") }
                            }
                            "bash_code_execution" | "code_execution" => "executing code".to_string(),
                            "text_editor_code_execution" => "editing file".to_string(),
                            other => format!("{other}"),
                        };
                        out.push(ChatMessage {
                            id: block_id,
                            role: ChatRole::Assistant,
                            content: summary,
                            created_at: ts.clone(),
                            kind: MessageKind::ToolCall,
                            tool_name: Some(tool_name.to_string()),
                            mcp_app_html: None,
                            mcp_app_input: None, model: None,
                        });
                    } else if block_type.ends_with("_tool_result") {
                        let mut output = String::new();

                        // Web search results — show titles and URLs
                        if block_type == "web_search_tool_result" {
                            if let Some(results) = val["content"].as_array() {
                                for r in results {
                                    let title = r["title"].as_str().unwrap_or("");
                                    let url = r["url"].as_str().unwrap_or("");
                                    if !title.is_empty() {
                                        output.push_str(&format!("- {title}"));
                                        if !url.is_empty() { output.push_str(&format!(" ({url})")); }
                                        output.push('\n');
                                    }
                                }
                            }
                        }

                        // Code execution results — show stdout/stderr
                        if output.is_empty() {
                            let stdout = val["content"]["stdout"].as_str().unwrap_or("");
                            let stderr = val["content"]["stderr"].as_str().unwrap_or("");
                            if !stdout.is_empty() { output.push_str(stdout); }
                            if !stderr.is_empty() {
                                if !output.is_empty() { output.push('\n'); }
                                output.push_str(stderr);
                            }
                        }

                        if output.is_empty() {
                            // Skip empty results entirely (encrypted results, etc.)
                            continue;
                        }

                        let truncated: String = output.chars().take(2000).collect();
                        out.push(ChatMessage {
                            id: block_id,
                            role: ChatRole::Assistant,
                            content: truncated,
                            created_at: ts.clone(),
                            kind: MessageKind::ToolOutput,
                            tool_name: None,
                            mcp_app_html: None,
                            mcp_app_input: None, model: None,
                        });
                    }
                    // Other unknown blocks (container_upload, etc.) — skip
                }
                // Image, Document — skip for UI
                _ => {}
            }
        }
    }
    out
}

/// Short summary of a tool use for display.
fn tool_use_summary(name: &str, input: &serde_json::Value) -> String {
    // Extract first meaningful field value for a one-line summary
    if let Some(obj) = input.as_object() {
        for key in &["query", "command", "path", "content", "url", "name", "message"] {
            if let Some(val) = obj.get(*key) {
                let owned = val.to_string();
                let s = val.as_str().unwrap_or(&owned);
                let truncated = if s.len() > 80 {
                    let end = s.floor_char_boundary(80);
                    format!("{}…", &s[..end])
                } else {
                    s.to_string()
                };
                return format!("{name}: {truncated}");
            }
        }
    }
    name.to_string()
}

/// Merge timestamps from old entries into a new message list from the LLM.
/// Old entries that match by position keep their ts/id; new entries get fresh values.
/// Strip injected context blocks from user messages before saving to history.
/// Removes [current time: ...] and [system: auto-recalled memories ...] blocks.
fn strip_context_blocks(msg: &Message) -> Message {
    match msg {
        Message::User { content } => {
            let cleaned: Vec<ContentBlock> = content.iter().filter(|b| {
                if let ContentBlock::Text { text } = b {
                    !text.starts_with("[current time:")
                        && !text.starts_with("[system: auto-recalled")
                } else {
                    true
                }
            }).cloned().collect();
            Message::User { content: if cleaned.is_empty() { content.clone() } else { cleaned } }
        }
        other => other.clone(),
    }
}

// merge_with_timestamps removed — history is now append-only.
// Each new message is appended to rig_history.json with a fresh ts/id.
// No index-based matching, no overwriting of existing entries.

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ImageSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
    #[serde(rename = "url")]
    Url { url: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum DocumentSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
    #[serde(rename = "url")]
    Url { url: String },
}

// ═══════════════════════════════════════════════════════════════════════════
// LlmBackend — direct API calls to Anthropic / OpenAI / OpenRouter
// ═══════════════════════════════════════════════════════════════════════════

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 2000;

fn is_rate_limit_error(msg: &str) -> bool {
    msg.contains("429")
        || msg.contains("rate_limit")
        || msg.contains("Too Many Requests")
        || msg.contains("529")
        || msg.contains("overloaded")
}

async fn retry_on_rate_limit<F, Fut, T>(f: F) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if attempt < MAX_RETRIES && is_rate_limit_error(&e.to_string()) => {
                attempt += 1;
                let delay = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1);
                log::warn!(
                    "Rate limited, retrying in {delay}ms (attempt {attempt}/{MAX_RETRIES})"
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

pub const DEFAULT_ONBOARDING_PROMPT: &str = "\
you are a quiet, thoughtful companion. you speak in lowercase, keep your \
responses short and gentle — one or two sentences at most. you listen more \
than you speak. you're warm but not overbearing. this is a safe, intimate space.";

/// Result of a tool-using LLM call.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ToolChatResult {
    pub text: String,
    /// Full message history including tool call/result entries.
    pub rig_history: Option<Vec<Message>>,
    /// The message ID used during streaming (so the saved message can reuse it).
    pub message_id: Option<String>,
    /// Total tokens used (input + output) across all turns, from API usage.
    pub tokens_used: u64,
}

#[derive(Clone)]
pub struct LlmBackend {
    pub http: reqwest::Client,
    pub api_key: String,
    pub model: String,
    /// Provider type — Api (direct Anthropic) or ClaudeCli (subprocess).
    pub provider: crate::config::LlmProvider,
}

impl LlmBackend {
    pub fn from_config(config: &Config) -> Option<Self> {
        let http = reqwest::Client::new();
        let model = config.llm.model_name().to_string();

        match config.llm.provider {
            crate::config::LlmProvider::Api => {
                let api_key = config.llm.api_key()?.to_string();
                Some(Self { http, api_key, model, provider: crate::config::LlmProvider::Api })
            }
            crate::config::LlmProvider::ClaudeCli => {
                Some(Self {
                    http,
                    api_key: String::new(), // not used for CLI
                    model,
                    provider: crate::config::LlmProvider::ClaudeCli,
                })
            }
        }
    }

    /// Whether this backend uses the Claude CLI subprocess.
    pub fn is_cli(&self) -> bool {
        matches!(self.provider, crate::config::LlmProvider::ClaudeCli)
    }

    /// Find an OAuth token from any instance for background CLI calls.
    fn resolve_cli_token(&self) -> String {
        let workspace = crate::config::workspace_root();
        let instances_dir = workspace.join("instances");
        if let Ok(entries) = std::fs::read_dir(&instances_dir) {
            for entry in entries.flatten() {
                if let Some(token) = super::claude_cli::load_token(&workspace, &entry.file_name().to_string_lossy()) {
                    return token.access_token;
                }
            }
        }
        String::new()
    }

    /// Create a variant using the fast/cheap model.
    pub fn fast_variant_with(&self, override_model: Option<&str>) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: override_model.filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_FAST_MODEL).to_string(),
            provider: self.provider,
        }
    }

    /// Create a variant using the cheapest model for background tasks (Haiku).
    pub fn cheap_variant(&self) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: CHEAP_MODEL.to_string(),
            provider: self.provider,
        }
    }

    /// Create a variant using the heavy model (Opus) for deep reflection.
    pub fn heavy_variant(&self) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: DEFAULT_MODEL.to_string(),
            provider: self.provider,
        }
    }


    pub fn model_name(&self) -> &str {
        &self.model
    }


    /// Classify whether a user message needs the heavy model.
    pub async fn classify_needs_heavy(&self, user_message: &str) -> bool {
        let classifier = self.cheap_variant();
        let system = "Classify this message. Respond with exactly one word.\n\
            Say \"heavy\" if it needs: complex reasoning, code, analysis, creative writing, research, multi-step tasks, tool use.\n\
            Say \"fast\" if it's: casual chat, greeting, short reply, simple question, emotional support, acknowledgment.";

        match classifier.chat(system, user_message, vec![]).await {
            Ok((response, _)) => {
                let word = response.trim().to_lowercase();
                let heavy = word.contains("heavy");
                log::info!("model router: classified as {} for: {}",
                    if heavy { "heavy" } else { "fast" },
                    &user_message.chars().take(80).collect::<String>());
                heavy
            }
            Err(e) => {
                log::warn!("model router: classifier failed, defaulting to heavy: {e}");
                true
            }
        }
    }

    /// Simple chat without tools. Returns (text, tokens_used).
    pub async fn chat(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
    ) -> anyhow::Result<(String, u64)> {
        if self.is_cli() {
            // Build context from history
            let mut context = String::new();
            for msg in &history {
                match msg {
                    Message::User { content } => {
                        let t: String = content.iter().filter_map(|b| if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join(" ");
                        if !t.is_empty() { context.push_str(&format!("User: {t}\n")); }
                    }
                    Message::Assistant { content } => {
                        let t: String = content.iter().filter_map(|b| if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join(" ");
                        if !t.is_empty() { context.push_str(&format!("Assistant: {t}\n")); }
                    }
                }
            }
            let full = if context.is_empty() { prompt.to_string() } else { format!("{context}\n{prompt}") };
            // Load any available OAuth token (try all instances)
            let token = self.resolve_cli_token();
            return super::claude_cli::run_prompt(&self.model, system_prompt, &full, &token, None).await;
        }
        let backend = self.clone();
        let system = system_prompt.to_string();
        let prompt = prompt.to_string();
        retry_on_rate_limit(|| {
            let backend = backend.clone();
            let system = system.clone();
            let prompt = prompt.clone();
            let history = history.clone();
            async move {
                let mut messages = history;
                messages.push(Message::user(&prompt));
                anthropic_complete(&backend.http, &backend.api_key, &backend.model, &[&system], &[], &messages, 16384)
                    .await
                    .map(|(text, _, _, tokens)| (text, tokens))
            }
        })
        .await
    }

    /// Chat with structured JSON output.
    pub async fn chat_json(
        &self,
        system_prompt: &str,
        prompt: &str,
        schema: serde_json::Value,
    ) -> anyhow::Result<(String, u64)> {
        if self.is_cli() {
            // CLI mode: ask for JSON in the prompt, parse from response
            let json_prompt = format!(
                "{prompt}\n\nIMPORTANT: Respond with ONLY valid JSON matching this schema, no other text:\n{schema}",
                schema = serde_json::to_string_pretty(&schema).unwrap_or_default()
            );
            let token = self.resolve_cli_token();
            let (text, tokens) = super::claude_cli::run_prompt(&self.model, system_prompt, &json_prompt, &token, None).await?;
            // Try to extract JSON from response (may have markdown fences)
            let cleaned = text.trim()
                .trim_start_matches("```json").trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            return Ok((cleaned.to_string(), tokens));
        }
        let backend = self.clone();
        let system = system_prompt.to_string();
        let prompt = prompt.to_string();
        retry_on_rate_limit(|| {
            let backend = backend.clone();
            let system = system.clone();
            let prompt = prompt.clone();
            let schema = schema.clone();
            async move {
                let messages = vec![Message::user(&prompt)];
                let system_blocks = vec![serde_json::json!({"type": "text", "text": &system})];
                let msgs = serde_json::to_value(&messages).unwrap_or(serde_json::json!([]));

                let req = serde_json::json!({
                    "model": &backend.model,
                    "max_tokens": 16384,
                    "system": system_blocks,
                    "messages": msgs,
                    "output_config": {
                        "format": {
                            "type": "json_schema",
                            "schema": schema,
                        }
                    }
                });

                let resp = backend.http
                    .post("https://api.anthropic.com/v1/messages")
                    .headers(anthropic_headers(&backend.api_key))
                    .json(&req)
                    .send()
                    .await?;

                let status = resp.status();
                let resp_text = resp.text().await?;
                if !status.is_success() {
                    return Err(anyhow::anyhow!("Anthropic API error {status}: {resp_text}"));
                }

                let resp_json: serde_json::Value = serde_json::from_str(&resp_text)?;
                let tokens = resp_json.pointer("/usage/input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    + resp_json.pointer("/usage/output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

                let text = resp_json.pointer("/content/0/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                Ok((text, tokens))
            }
        })
        .await
    }

    /// Streaming chat with tools.
    pub async fn chat_with_tools_streaming(
        &self,
        system_prompt: &[&str],
        prompt: Message,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
        events: broadcast::Sender<ServerEvent>,
        instance_slug: &str,
        chat_id: &str,
        workspace_dir: &Path,
        mcp_snapshot: Option<super::mcp::McpAppSnapshot>,
        sent_files: super::tools::SentFiles,
    ) -> anyhow::Result<ToolChatResult> {
        if self.is_cli() {
            return self.cli_chat_streaming(
                system_prompt, prompt, history, events,
                instance_slug, chat_id, workspace_dir,
            ).await;
        }

        log::info!("chat_with_tools_streaming: {} tools", tools.len());

        let tool_defs = collect_tool_defs(&tools).await;
        let mut messages = history;
        if let Message::User { content } = prompt {
            messages.push(Message::User { content });
        }

        let result = streaming_agent_loop(
            self,
            system_prompt,
            &tool_defs,
            &tools,
            &mut messages,
            &events,
            instance_slug,
            chat_id,
            workspace_dir,
            mcp_snapshot.as_ref(),
            &sent_files,
        )
        .await;

        match result {
            Ok((text, message_id, tokens_used)) => Ok(ToolChatResult {
                text,
                rig_history: Some(messages),
                message_id,
                tokens_used,
            }),
            Err(e) => Err(e),
        }
    }

    /// Claude CLI chat path: spawn subprocess, fake-stream result.
    async fn cli_chat_streaming(
        &self,
        system_prompt: &[&str],
        prompt: Message,
        history: Vec<Message>,
        events: broadcast::Sender<ServerEvent>,
        instance_slug: &str,
        chat_id: &str,
        workspace_dir: &Path,
    ) -> anyhow::Result<ToolChatResult> {
        // Load per-instance OAuth token
        let token = super::claude_cli::load_token(workspace_dir, instance_slug)
            .ok_or_else(|| anyhow::anyhow!(
                "No Claude CLI OAuth token found for this instance. Please connect your Claude account in Settings."
            ))?;

        // Build system prompt string
        let system = system_prompt.join("\n\n");

        // Extract the last user message text
        let user_text = if let Message::User { content } = &prompt {
            content.iter().filter_map(|b| {
                if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
            }).collect::<Vec<_>>().join("\n")
        } else {
            String::new()
        };

        // Build history context to embed in the prompt
        let mut context = String::new();
        let recent = if history.len() > 20 { &history[history.len() - 20..] } else { &history };
        if !recent.is_empty() {
            context.push_str("[Recent conversation]\n");
            for msg in recent {
                match msg {
                    Message::User { content } => {
                        let text: String = content.iter().filter_map(|b| {
                            if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
                        }).collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            context.push_str(&format!("User: {text}\n"));
                        }
                    }
                    Message::Assistant { content } => {
                        let text: String = content.iter().filter_map(|b| {
                            if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
                        }).collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            context.push_str(&format!("Assistant: {text}\n"));
                        }
                    }
                }
            }
            context.push_str("\n[Current message]\n");
        }
        let full_prompt = format!("{context}{user_text}");

        log::info!("claude CLI: sending prompt ({} chars) with system ({} chars)",
            full_prompt.len(), system.len());

        // Build MCP config so Claude CLI can call our tools
        // Build MCP config so Claude CLI can call our tools
        let mcp = {
            let config = crate::config::load_config().ok();
            config.map(|c| super::claude_cli::McpConfig {
                server_url: format!("http://localhost:{}", c.port),
                auth_token: c.auth_token.clone(),
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
            })
        };

        // Call Claude CLI
        let (text, tokens_used) = super::claude_cli::run_prompt(
            &self.model,
            &system,
            &full_prompt,
            &token.access_token,
            mcp.as_ref(),
        ).await?;

        log::info!("claude CLI: response received, {} chars. Streaming to client...", text.len());

        // Fake-stream the result word by word
        let message_id = super::chat::next_id();
        for word in text.split_inclusive(char::is_whitespace) {
            let _ = events.send(crate::domain::events::ServerEvent::ChatStreamDelta {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                message_id: message_id.clone(),
                delta: word.to_string(),
            });
            tokio::time::sleep(std::time::Duration::from_millis(8)).await;
        }

        // Build messages list for rig_history
        let mut messages = history;
        if let Message::User { content } = prompt {
            messages.push(Message::User { content });
        }
        let assistant_msg = Message::Assistant {
            content: vec![ContentBlock::Text { text: text.clone() }],
        };

        // Save to rig_history (the API path does this inside streaming_agent_loop)
        let rig_path = super::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
        let ts = crate::services::tools::unix_millis().to_string();
        let mut entry = HistoryEntry::new(
            assistant_msg.clone(),
            ts,
            message_id.clone(),
        );
        entry.model = Some(self.model.clone());
        super::chat::append_to_rig_history(&rig_path, &entry);

        messages.push(assistant_msg);

        // Send snapshot so client refreshes the chat
        if let Ok(resp) = super::chat::load_messages(workspace_dir, instance_slug, chat_id) {
            let _ = events.send(crate::domain::events::ServerEvent::ChatSnapshot {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                messages: resp.messages,
                agent_running: false,
            });
        }

        log::info!("claude CLI: turn complete, saved {} chars to rig_history", text.len());

        Ok(ToolChatResult {
            text,
            rig_history: Some(messages),
            message_id: Some(message_id),
            tokens_used,
        })
    }

    /// Simplified tool call (no streaming). Used by heartbeat.
    #[allow(dead_code)]
    pub async fn chat_with_tools_only(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> anyhow::Result<(String, u64)> {
        if self.is_cli() || tools.is_empty() {
            return self.chat(system_prompt, prompt, history).await;
        }
        let system_blocks: &[&str] = &[system_prompt];

        let tool_defs = collect_tool_defs(&tools).await;
        let mut messages = history;
        messages.push(Message::user(prompt));

        agent_loop(self, system_blocks, &tool_defs, &tools, &mut messages).await
    }

    /// Like `chat_with_tools_only` but returns the full message trace.
    pub async fn chat_with_tools_traced(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> anyhow::Result<(String, u64, Vec<Message>)> {
        if tools.is_empty() {
            let (text, tokens) = self.chat(system_prompt, prompt, history).await?;
            return Ok((text, tokens, vec![]));
        }
        let system_blocks: &[&str] = &[system_prompt];
        let tool_defs = collect_tool_defs(&tools).await;
        let mut messages = history;
        messages.push(Message::user(prompt));
        let (text, tokens) = agent_loop(self, system_blocks, &tool_defs, &tools, &mut messages).await?;
        Ok((text, tokens, messages))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Agent loops (tool call → execute → send back)
// ═══════════════════════════════════════════════════════════════════════════

async fn collect_tool_defs(tools: &[Box<dyn ToolDyn>]) -> Vec<ToolDefinition> {
    let mut defs = Vec::with_capacity(tools.len());
    for t in tools {
        defs.push(t.definition(String::new()).await);
    }
    defs
}

/// Non-streaming agent loop. Returns (final text, total tokens used).
async fn agent_loop(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    tools: &[Box<dyn ToolDyn>],
    messages: &mut Vec<Message>,
) -> anyhow::Result<(String, u64)> {
    let mut total_tokens: u64 = 0;
    loop {
        let (text, tool_uses, stop_reason, tokens) = complete_once(backend, system, tool_defs, messages).await?;
        total_tokens += tokens;

        // Build assistant message
        let mut assistant_content = Vec::new();
        if !text.is_empty() {
            assistant_content.push(ContentBlock::text(&text));
        }
        for tu in &tool_uses {
            assistant_content.push(ContentBlock::ToolUse {
                id: tu.id.clone(),
                name: tu.name.clone(),
                input: tu.input.clone(),
            });
        }
        messages.push(Message::Assistant {
            content: assistant_content,
        });

        if stop_reason == "max_tokens" {
            log::warn!("[llm] response truncated (max_tokens reached) — requesting continuation");
            messages.push(Message::User {
                content: vec![ContentBlock::text(
                    "[system: your previous response was cut off due to length. please continue exactly where you left off.]",
                )],
            });
            continue;
        }

        if stop_reason == "pause_turn" {
            log::info!("[llm] pause_turn — code execution in progress, continuing...");
            continue;
        }

        if stop_reason != "tool_use" || tool_uses.is_empty() {
            return Ok((text, total_tokens));
        }

        // Execute tools — images stay inside tool_result content per Anthropic API spec
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        messages.push(Message::User { content: results });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Context compaction — provider-agnostic
// ═══════════════════════════════════════════════════════════════════════════

const COMPACT_TOKEN_THRESHOLD: usize = 100_000;
const COMPACT_KEEP_MESSAGES: usize = 10;
const COMPACT_KEEP_TOKENS: usize = 20_000;

/// Estimate total tokens in a message history (~3 chars per token).
fn estimate_history_tokens(messages: &[Message]) -> usize {
    let chars: usize = messages.iter().map(|m| {
        let content = match m {
            Message::User { content } | Message::Assistant { content } => content,
        };
        content.iter().map(|block| match block {
            ContentBlock::Text { text } => text.len(),
            ContentBlock::Compaction { content } => content.len(),
            ContentBlock::ToolResult { content, .. } => {
                content.as_str().map(|s| s.len()).unwrap_or(0)
            }
            ContentBlock::ToolUse { name, input, .. } => {
                name.len() + input.to_string().len()
            }
            _ => 0,
        }).sum::<usize>()
    }).sum();
    chars / 3
}

/// Flatten messages to a plain-text transcript for the summarizer.
fn messages_to_transcript(messages: &[Message]) -> String {
    let mut out = String::new();
    for msg in messages {
        let (role, content) = match msg {
            Message::User { content } => ("User", content),
            Message::Assistant { content } => ("Assistant", content),
        };
        out.push_str(&format!("\n{role}:\n"));
        for block in content {
            match block {
                ContentBlock::Text { text } => {
                    if text.starts_with("[current time:") || text.starts_with("[system: auto-recalled") {
                        continue;
                    }
                    let truncated: String = text.chars().take(2000).collect();
                    out.push_str(&truncated);
                    out.push('\n');
                }
                ContentBlock::Compaction { content } => {
                    out.push_str(&format!("[Previous summary: {content}]\n"));
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    let input_str: String = input.to_string().chars().take(300).collect();
                    out.push_str(&format!("[Called tool: {name}({input_str})]\n"));
                }
                ContentBlock::ToolResult { content, .. } => {
                    let s = content.as_str().unwrap_or("(non-text)");
                    let truncated: String = s.chars().take(500).collect();
                    out.push_str(&format!("[Tool result: {truncated}]\n"));
                }
                _ => {}
            }
        }
    }
    out
}

/// Compact history if it exceeds the token threshold.
/// Returns true if compaction was performed.
async fn maybe_compact_history(
    backend: &LlmBackend,
    messages: &mut Vec<Message>,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    workspace_dir: &Path,
) -> bool {
    let total_tokens = estimate_history_tokens(messages);
    if total_tokens < COMPACT_TOKEN_THRESHOLD {
        return false;
    }

    log::info!(
        "[compaction] history ~{total_tokens} tokens (threshold {COMPACT_TOKEN_THRESHOLD}) — compacting"
    );

    let msg_count = messages.len();

    // Determine split point: keep recent messages from the end
    let mut keep_from = msg_count.saturating_sub(COMPACT_KEEP_MESSAGES);

    // Ensure we keep at least COMPACT_KEEP_TOKENS worth of recent context
    let mut recent_tokens = 0usize;
    for i in (0..msg_count).rev() {
        recent_tokens += estimate_history_tokens(&messages[i..=i]);
        if recent_tokens >= COMPACT_KEEP_TOKENS {
            keep_from = keep_from.min(i);
            break;
        }
    }

    // Never split a tool_use/tool_result pair
    if keep_from > 0 {
        if let Message::User { content } = &messages[keep_from] {
            if content.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. })) {
                keep_from -= 1;
            }
        }
    }

    if keep_from <= 1 {
        log::info!("[compaction] too few messages to compact — skipping");
        return false;
    }

    let messages_to_compact = &messages[..keep_from];
    let compacted_count = messages_to_compact.len();

    // Broadcast UI event
    let _ = events.send(ServerEvent::ContextCompacting {
        instance_slug: instance_slug.to_string(),
        chat_id: chat_id.to_string(),
        messages_compacted: compacted_count,
    });

    // Build transcript and summarize with the cheap model
    let transcript = messages_to_transcript(messages_to_compact);
    let system_prompt = "\
        You are a conversation summarizer. Produce a concise summary of the conversation transcript below.\n\
        Capture:\n\
        1. Key facts, decisions, and user preferences\n\
        2. Current task state and goals\n\
        3. Important tool calls and their outcomes\n\
        4. Pending work or unresolved questions\n\
        Be factual and specific. Do not editorialize. Write in third person.\n\
        Target 1000-2000 words. If there is a previous summary, incorporate it.";

    let summary = match backend.chat(system_prompt, &transcript, vec![]).await {
        Ok((text, _tokens)) => {
            log::info!("[compaction] summary generated: {} chars", text.len());
            text
        }
        Err(e) => {
            log::error!("[compaction] summarization failed: {e} — skipping");
            return false;
        }
    };

    // Replace old messages with compaction block + recent messages
    let recent: Vec<Message> = messages[keep_from..].to_vec();
    messages.clear();
    messages.push(Message::Assistant {
        content: vec![ContentBlock::Compaction { content: summary }],
    });
    messages.extend(recent);

    // Persist compacted history to disk
    let rig_path = super::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
    let ts = super::tools::unix_millis().to_string();
    let entries: Vec<HistoryEntry> = messages.iter().enumerate().map(|(i, msg)| {
        HistoryEntry::new(msg.clone(), ts.clone(), format!("compact_{i}_{ts}"))
    }).collect();
    super::chat::save_rig_history(&rig_path, &entries);

    // Broadcast snapshot so the client immediately reflects the compacted state
    if let Ok(resp) = super::chat::load_messages(workspace_dir, instance_slug, chat_id) {
        let _ = events.send(ServerEvent::ChatSnapshot {
            instance_slug: instance_slug.to_string(),
            chat_id: chat_id.to_string(),
            messages: resp.messages,
            agent_running: true,
        });
    }

    log::info!(
        "[compaction] compacted {compacted_count} messages → {} remaining",
        messages.len(),
    );
    true
}

/// Streaming agent loop. Returns (final text, message_id, total tokens).
async fn streaming_agent_loop(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    tools: &[Box<dyn ToolDyn>],
    messages: &mut Vec<Message>,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    workspace_dir: &Path,
    mcp_snapshot: Option<&super::mcp::McpAppSnapshot>,
    sent_files: &super::tools::SentFiles,
) -> anyhow::Result<(String, Option<String>, u64)> {
    let mut all_text = String::new();
    let mut total_tokens: u64 = 0;
    let mut current_message_id = super::chat::next_id();

    // Pre-call compaction: summarize old messages if context is too large
    maybe_compact_history(backend, messages, events, instance_slug, chat_id, workspace_dir).await;

    loop {
        let turn = stream_once(
            backend, system, tool_defs, messages, events,
            instance_slug, chat_id, &current_message_id, mcp_snapshot,
        ).await?;

        total_tokens += turn.tokens_used;
        let turn_text = turn.text;
        let tool_uses = turn.tool_uses;
        let stop_reason = turn.stop_reason;


        // Build assistant message — use ordered_content which preserves
        // the interleaving of text, server_tool_use, and server_tool_result.
        let mut assistant_content = Vec::new();
        // Ordered content: text and server tool blocks in their original order
        assistant_content.extend(turn.ordered_content.into_iter());
        for tu in &tool_uses {
            assistant_content.push(ContentBlock::ToolUse {
                id: tu.id.clone(),
                name: tu.name.clone(),
                input: tu.input.clone(),
            });
        }
        messages.push(Message::Assistant {
            content: assistant_content,
        });

        if stop_reason == "max_tokens" {
            log::warn!("[llm] response truncated (max_tokens reached) — requesting continuation");
            all_text.push_str(&turn_text);
            messages.push(Message::User {
                content: vec![ContentBlock::text(
                    "[system: your previous response was cut off due to length. please continue exactly where you left off.]",
                )],
            });
            continue;
        }

        // pause_turn: code execution skill is still running — continue with same messages
        if stop_reason == "pause_turn" {
            log::info!("[llm] pause_turn — code execution in progress, continuing...");
            all_text.push_str(&turn_text);
            continue;
        }

        // For the final turn (no more tool use), only keep this turn's text.
        all_text = turn_text.clone();

        if stop_reason != "tool_use" || tool_uses.is_empty() {
            break;
        }

        // Save intermediate text before tool execution — reuse the streaming message_id
        if !turn_text.trim().is_empty() {
            let ts = super::tools::unix_millis();
            let msg = ChatMessage {
                id: current_message_id.clone(),
                role: ChatRole::Assistant,
                content: turn_text.trim().to_string(),
                created_at: ts.to_string(),
                kind: Default::default(),
                tool_name: None,
                mcp_app_html: None,
                mcp_app_input: None, model: None,
            };
            let _ = events.send(ServerEvent::ChatMessageCreated {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                message: msg,
            });
            // Generate new ID for the next streaming turn
            current_message_id = super::chat::next_id();
        }

        // Execute tools — images stay inside tool_result content per Anthropic API spec
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        let tool_result_msg = Message::User { content: results };
        messages.push(tool_result_msg.clone());

        // Append new messages to rig_history (append-only, no merge).
        let rig_path = super::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
        let ts = super::tools::unix_millis().to_string();
        // The assistant message (with tool_use) was pushed to messages a few lines above
        let assistant_msg = &messages[messages.len() - 2]; // assistant before tool_result
        super::chat::append_to_rig_history(&rig_path, &HistoryEntry::new(
            strip_context_blocks(assistant_msg), ts.clone(), format!("tool_{}", super::tools::unix_millis()),
        ));
        super::chat::append_to_rig_history(&rig_path, &HistoryEntry::new(
            strip_context_blocks(&tool_result_msg), ts, format!("tool_{}", super::tools::unix_millis()),
        ));

        // Snapshot after each tool cycle — all clients converge to ground truth
        if let Ok(resp) = super::chat::load_messages(workspace_dir, instance_slug, chat_id) {
            let _ = events.send(ServerEvent::ChatSnapshot {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                messages: resp.messages,
                agent_running: true,
            });
        }

        // Re-check compaction after tool cycles (long chains grow fast)
        maybe_compact_history(backend, messages, events, instance_slug, chat_id, workspace_dir).await;
    }

    // ── Final assembly: file markers from send_file accumulated during the agent loop ──
    let final_markers: Vec<String> = {
        let mut sf = sent_files.lock().unwrap_or_else(|e| e.into_inner());
        sf.drain(..).collect()
    };

    // Append all markers to the last assistant message in rig_history
    if !final_markers.is_empty() {
        if let Some(Message::Assistant { content }) = messages.last_mut() {
            for m in &final_markers {
                content.push(ContentBlock::text(m));
            }
        }
    }

    // Stamp model name on last assistant entry

    // Final save: append the last assistant message to rig_history.
    // Tool-cycle messages were already appended during the loop.
    // Only the final response (no more tool_use) needs to be saved here.
    let rig_path = super::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
    if let Some(last_msg) = messages.last() {
        if matches!(last_msg, Message::Assistant { .. }) {
            let ts = super::tools::unix_millis().to_string();
            let mut entry = HistoryEntry::new(
                strip_context_blocks(last_msg),
                ts,
                format!("msg_{}", super::tools::unix_millis()),
            );
            entry.model = Some(backend.model.clone());
            super::chat::append_to_rig_history(&rig_path, &entry);
        }
    }

    // Final snapshot so client converges to ground truth
    if let Ok(resp) = super::chat::load_messages(workspace_dir, instance_slug, chat_id) {
        let _ = events.send(ServerEvent::ChatSnapshot {
            instance_slug: instance_slug.to_string(),
            chat_id: chat_id.to_string(),
            messages: resp.messages,
            agent_running: true,
        });
    }

    Ok((all_text, Some(current_message_id), total_tokens))
}

async fn execute_tool(tools: &[Box<dyn ToolDyn>], name: &str, input: &serde_json::Value) -> String {
    if let Some(tool) = tools.iter().find(|t| t.name() == name) {
        let args = serde_json::to_string(input).unwrap_or_default();
        match tool.call(args).await {
            Ok(s) => s,
            Err(e) => format!("error: {e}"),
        }
    } else {
        format!("error: unknown tool '{name}'")
    }
}

struct ToolUseBlock {
    id: String,
    name: String,
    input: serde_json::Value,
}

// ═══════════════════════════════════════════════════════════════════════════
// Provider dispatch — route to Anthropic or OpenAI
// ═══════════════════════════════════════════════════════════════════════════

/// Non-streaming completion. Returns (text, tool_uses, stop_reason).
async fn complete_once(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
) -> anyhow::Result<(String, Vec<ToolUseBlock>, String, u64)> {
    anthropic_complete(&backend.http, &backend.api_key, &backend.model, system, tool_defs, messages, 16384).await
}

// ═══════════════════════════════════════════════════════════════════════════
// Anthropic API
// ═══════════════════════════════════════════════════════════════════════════

fn anthropic_headers(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-api-key", api_key.parse().unwrap());
    headers.insert("anthropic-beta", "interleaved-thinking-2025-05-14".parse().unwrap());
    headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
    headers.insert("content-type", "application/json".parse().unwrap());
    headers
}

fn build_anthropic_request(
    model: &str,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
    stream: bool,
    _api_key: &str,
) -> serde_json::Value {
    // System blocks — all blocks are stable now (time moved to user message).
    // Each block gets cache_control to maximize prefix caching.
    let system_blocks: Vec<serde_json::Value> = system
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_empty())
        .map(|(i, s)| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            s.hash(&mut hasher);
            let hash = hasher.finish();
            log::info!("[llm] system block[{i}]: {} chars, hash={:x}", s.len(), hash);
            serde_json::json!({
                "type": "text",
                "text": *s,
                "cache_control": {"type": "ephemeral"},
            })
        })
        .collect();

    // Tool definitions
    let tool_count = tool_defs.len();
    let tools: Vec<serde_json::Value> = tool_defs
        .iter()
        .enumerate()
        .map(|(i, td)| {
            let mut tool = serde_json::json!({
                "name": td.name,
                "description": td.description,
                "input_schema": td.parameters,
            });
            // Cache breakpoint on last tool — caches all tools as one prefix
            if i == tool_count - 1 {
                tool["cache_control"] = serde_json::json!({"type": "ephemeral"});
            }
            tool
        })
        .collect();

    // Messages — strip any legacy oversized base64 images
    let mut msgs = serde_json::to_value(messages).unwrap_or(serde_json::json!([]));
    if let Some(arr) = msgs.as_array_mut() {
        for msg in arr.iter_mut() {
            if let Some(content_arr) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                // Transform compaction blocks to text (APIs don't recognize "compaction" type)
                for block in content_arr.iter_mut() {
                    if block.get("type").and_then(|t| t.as_str()) == Some("compaction") {
                        if let Some(content) = block.get("content").and_then(|c| c.as_str()).map(|s| s.to_string()) {
                            *block = serde_json::json!({
                                "type": "text",
                                "text": format!("[Context summary from earlier conversation]\n{content}")
                            });
                        }
                    }
                }
                content_arr.retain(|block| {
                    let block_type = block.get("type").and_then(|t| t.as_str());
                    // Strip oversized base64 images
                    if block_type == Some("image") {
                        if let Some(data) = block.pointer("/source/data").and_then(|d| d.as_str()) {
                            if data.len() > 5 * 1024 * 1024 {
                                log::info!("stripping oversized base64 image ({} bytes)", data.len());
                                return false;
                            }
                        }
                    }
                    // Strip blocks with no recognized type (Unknown variant)
                    if block_type.is_none() {
                        log::info!("stripping block with no type");
                        return false;
                    }
                    true
                });
                // Remove empty content arrays (can happen after stripping)
                if content_arr.is_empty() {
                    content_arr.push(serde_json::json!({"type": "text", "text": "(continued)"}));
                }
            }
        }
    }

    // Strip orphaned tool_result blocks — can happen when server-side compaction
    // replaces tool_use with summary text but leaves the tool_result in place.
    if let Some(arr) = msgs.as_array_mut() {
        // Collect all tool_use IDs from assistant messages
        let mut tool_use_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for msg in arr.iter() {
            if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            if let Some(id) = block.get("id").and_then(|i| i.as_str()) {
                                tool_use_ids.insert(id.to_string());
                            }
                        }
                    }
                }
            }
        }
        // Remove tool_result blocks that reference non-existent tool_use IDs
        for msg in arr.iter_mut() {
            if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                if let Some(content) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                    let before = content.len();
                    content.retain(|block| {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                            if let Some(id) = block.get("tool_use_id").and_then(|i| i.as_str()) {
                                if !tool_use_ids.contains(id) {
                                    log::warn!("[llm] stripping orphaned tool_result for {id} (tool_use lost, likely compaction)");
                                    return false;
                                }
                            }
                        }
                        true
                    });
                    if content.is_empty() && before > 0 {
                        content.push(serde_json::json!({"type": "text", "text": "(tool result removed — original tool call was compacted)"}));
                    }
                }
            }
        }
    }

    // Merge consecutive same-role messages (API requires strict alternation)
    if let Some(arr) = msgs.as_array_mut() {
        let mut merged: Vec<serde_json::Value> = Vec::with_capacity(arr.len());
        for msg in arr.drain(..) {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let last_role = merged.last()
                .and_then(|m| m.get("role"))
                .and_then(|r| r.as_str())
                .unwrap_or("");
            if role == last_role && !role.is_empty() {
                // Merge content arrays
                if let Some(last) = merged.last_mut() {
                    if let (Some(existing), Some(new_content)) = (
                        last.get_mut("content").and_then(|c| c.as_array_mut()),
                        msg.get("content").and_then(|c| c.as_array()),
                    ) {
                        existing.extend(new_content.iter().cloned());
                    }
                }
            } else {
                merged.push(msg);
            }
        }
        *arr = merged;
    }

    // Top-level cache_control: Anthropic automatically places a cache breakpoint
    // on the last cacheable block, so the entire conversation history (system +
    // tools + all prior messages) is cached. No manual per-message breakpoints needed.
    let mut req = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "cache_control": {"type": "ephemeral"},
        "system": system_blocks,
        "messages": msgs,
    });

    if !tools.is_empty() {
        req["tools"] = serde_json::Value::Array(tools);
    }

    // Anthropic server tools (always added for streaming chat with tools)
    if stream && !tool_defs.is_empty() {
        let tools_arr = req["tools"].as_array_mut().unwrap();

        // Web search + fetch (native Anthropic)
        // allowed_callers: ["direct"] disables dynamic filtering (code_execution for search)
        tools_arr.push(serde_json::json!({
            "type": "web_search_20260209",
            "name": "web_search",
            "allowed_callers": ["direct"]
        }));
        tools_arr.push(serde_json::json!({
            "type": "web_fetch_20260209",
            "name": "web_fetch",
            "allowed_callers": ["direct"]
        }));

    }
    if stream {
        req["stream"] = serde_json::json!(true);
    }
    req
}

/// Non-streaming Anthropic call. Returns (text, stop_reason).
async fn anthropic_complete(
    http: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
) -> anyhow::Result<(String, Vec<ToolUseBlock>, String, u64)> {
    let body = build_anthropic_request(model, system, tool_defs, messages, max_tokens, false, api_key);

    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .headers(anthropic_headers(api_key))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_text = resp.text().await?;
    if !status.is_success() {
        log::error!(
            "[llm] API {status} — model={model}, msgs={}, body_chars={}",
            messages.len(),
            serde_json::to_string(&body).map(|s| s.len()).unwrap_or(0),
        );
        return Err(anyhow::anyhow!("Anthropic API error {status}: {resp_text}"));
    }

    let resp_json: serde_json::Value = serde_json::from_str(&resp_text)?;
    let stop_reason = resp_json["stop_reason"]
        .as_str()
        .unwrap_or("end_turn")
        .to_string();

    let tokens_used = if let Some(usage) = resp_json.get("usage") {
        let input = usage["input_tokens"].as_u64().unwrap_or(0);
        let output = usage["output_tokens"].as_u64().unwrap_or(0);
        let cache_read = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
        let cache_write = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
        log::info!(
            "anthropic usage: input={} cache_read={} cache_write={} output={}",
            input, cache_read, cache_write, output,
        );
        // Normalize to output-equivalent tokens by cost ratio (Sonnet 4.6 pricing):
        // Output: $15/M (1.0x), Input: $3/M (0.2x), Cache write: $3.75/M (0.25x), Cache read: $0.30/M (0.02x)
        let normalized = (output as f64)
            + (input as f64 * 0.2)
            + (cache_write as f64 * 0.25)
            + (cache_read as f64 * 0.02);
        normalized as u64
    } else {
        0
    };

    let mut text = String::new();
    let mut tool_uses = Vec::new();

    if let Some(blocks) = resp_json["content"].as_array() {
        for b in blocks {
            match b["type"].as_str() {
                Some("text") => {
                    if let Some(s) = b["text"].as_str() {
                        text.push_str(s);
                    }
                }
                Some("tool_use") => {
                    if let (Some(id), Some(name)) = (b["id"].as_str(), b["name"].as_str()) {
                        tool_uses.push(ToolUseBlock {
                            id: id.to_string(),
                            name: name.to_string(),
                            input: b["input"].clone(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Ok((text, tool_uses, stop_reason, tokens_used))
}

/// Streaming Anthropic call. Broadcasts text deltas, returns (text, tool_uses, stop_reason).
async fn anthropic_stream(
    http: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    message_id: &str,
    mcp_snapshot: Option<&super::mcp::McpAppSnapshot>,
) -> anyhow::Result<(String, Vec<ToolUseBlock>, String, u64, Vec<ContentBlock>)> {
    let body = build_anthropic_request(model, system, tool_defs, messages, max_tokens, true, api_key);

    let headers = anthropic_headers(api_key);
    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .headers(headers)
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        log::error!(
            "[llm] streaming API {status} — model={model}, msgs={}",
            messages.len(),
        );
        // Log the message types to help debug which content block is invalid
        for (i, msg) in messages.iter().enumerate() {
            let (role, blocks) = match msg {
                Message::User { content } => ("user", content),
                Message::Assistant { content } => ("assistant", content),
            };
            let types: Vec<&str> = blocks.iter().map(|b| match b {
                ContentBlock::Text { .. } => "text",
                ContentBlock::Image { .. } => "image",
                ContentBlock::Document { .. } => "document",
                ContentBlock::ToolUse { .. } => "tool_use",
                ContentBlock::ToolResult { .. } => "tool_result",
                ContentBlock::Compaction { .. } => "compaction",
                ContentBlock::Unknown(_) => "unknown",
            }).collect();
            log::error!("[llm] msg[{i}] {role}: {:?}", types);
        }
        return Err(anyhow::anyhow!("Anthropic API error {status}: {err_text}"));
    }

    let mut text = String::new();
    let mut tool_uses: Vec<ToolUseBlock> = Vec::new();
    let mut stop_reason = String::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_write_tokens: u64 = 0;
    let mut current_server_block: Option<serde_json::Value> = None;

    // Ordered content blocks — preserves interleaving of text and server tools
    let mut ordered_content: Vec<ContentBlock> = Vec::new();
    let mut current_text_block = String::new();

    // Current block being built
    let mut current_block_type = String::new();
    let mut current_tool_id = String::new();
    let mut current_tool_name = String::new();
    let mut current_tool_input_json = String::new();
    let mut streaming_mcp_app = false;

    // SSE parser
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    let mut event_type = String::new();

    const STREAM_TIMEOUT: Duration = Duration::from_secs(480);

    loop {
        let chunk = tokio::time::timeout(STREAM_TIMEOUT, stream.next()).await;
        let chunk = match chunk {
            Ok(Some(Ok(c))) => c,
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(None) => break,
            Err(_) => {
                log::warn!("stream timed out after {}s", STREAM_TIMEOUT.as_secs());
                break;
            }
        };

        buf.extend_from_slice(&chunk);

        // Process complete lines
        while let Some(newline_pos) = buf.iter().position(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(&buf[..newline_pos]).to_string();
            buf = buf[newline_pos + 1..].to_vec();

            if line.is_empty() {
                // End of event — process it
                // (event_type is set from the "event: " line)
                event_type.clear();
                continue;
            }

            if let Some(e) = line.strip_prefix("event: ") {
                event_type = e.to_string();
                continue;
            }

            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };

            let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };

            match event_type.as_str() {
                "message_start" => {
                    if let Some(msg) = ev.get("message") {
                        if let Some(usage) = msg.get("usage") {
                            input_tokens = usage["input_tokens"].as_u64().unwrap_or(0);
                            cache_read_tokens = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                            cache_write_tokens = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                            let real_total = input_tokens + cache_read_tokens + cache_write_tokens;
                            log::info!(
                                "anthropic cache: read={} write={} input={} real_total={}",
                                cache_read_tokens, cache_write_tokens, input_tokens, real_total,
                            );
                            cache_real_input_tokens(instance_slug, chat_id, real_total);
                        }
                    }
                }
                "content_block_start" => {
                    if let Some(block) = ev.get("content_block") {
                        current_block_type =
                            block["type"].as_str().unwrap_or("").to_string();

                        // Flush accumulated text before server tool blocks
                        if (current_block_type == "server_tool_use" || current_block_type.ends_with("_tool_result"))
                            && !current_text_block.is_empty()
                        {
                            ordered_content.push(ContentBlock::text(&current_text_block));
                            current_text_block.clear();
                        }

                        // Save server tool blocks as raw JSON for rig_history
                        if current_block_type == "server_tool_use"
                            || current_block_type.ends_with("_tool_result")
                        {
                            current_server_block = Some(block.clone());
                        }

                        // Broadcast server tool activity (web_search, code_execution, etc.)
                        if current_block_type == "server_tool_use" {
                            let tool_name = block["name"].as_str().unwrap_or("server_tool");
                            let summary = match tool_name {
                                "web_search" => "searching the web".to_string(),
                                "web_fetch" => "fetching web page".to_string(),
                                "bash_code_execution" | "code_execution" => "executing code".to_string(),
                                "text_editor_code_execution" => "editing file in sandbox".to_string(),
                                other => format!("server: {other}"),
                            };
                            let msg = crate::domain::chat::ChatMessage {
                                id: format!("srvtool_{}",
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap().as_millis()),
                                role: crate::domain::chat::ChatRole::Assistant,
                                content: summary,
                                created_at: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap().as_millis().to_string(),
                                kind: crate::domain::chat::MessageKind::ToolCall,
                                tool_name: Some(tool_name.to_string()),
                                mcp_app_html: None, mcp_app_input: None, model: None,
                            };
                            let _ = events.send(ServerEvent::ChatMessageCreated {
                                instance_slug: instance_slug.to_string(),
                                chat_id: chat_id.to_string(),
                                message: msg,
                            });
                        }

                        if current_block_type == "tool_use" {
                            current_tool_id =
                                block["id"].as_str().unwrap_or("").to_string();
                            current_tool_name =
                                block["name"].as_str().unwrap_or("").to_string();
                            current_tool_input_json.clear();

                            // MCP app streaming
                            if let Some(snap) = mcp_snapshot {
                                if snap.is_app_tool(&current_tool_name) {
                                    streaming_mcp_app = true;
                                    if let Some(html) =
                                        snap.get_html(&current_tool_name).cloned()
                                    {
                                        let _ =
                                            events.send(ServerEvent::McpAppStart {
                                                instance_slug: instance_slug
                                                    .to_string(),
                                                chat_id: chat_id.to_string(),
                                                tool_name: current_tool_name
                                                    .clone(),
                                                html,
                                            });
                                    }
                                } else {
                                    streaming_mcp_app = false;
                                }
                            }
                        }
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = ev.get("delta") {
                        match delta["type"].as_str() {
                            Some("text_delta") => {
                                if let Some(t) = delta["text"].as_str() {
                                    text.push_str(t);
                                    current_text_block.push_str(t);
                                    let _ = events.send(ServerEvent::ChatStreamDelta {
                                        instance_slug: instance_slug.to_string(),
                                        chat_id: chat_id.to_string(),
                                        message_id: message_id.to_string(),
                                        delta: t.to_string(),
                                    });
                                }
                            }
                            Some("input_json_delta") => {
                                if let Some(partial) = delta["partial_json"].as_str() {
                                    current_tool_input_json.push_str(partial);
                                    if streaming_mcp_app {
                                        let _ = events.send(
                                            ServerEvent::McpAppInputDelta {
                                                instance_slug: instance_slug
                                                    .to_string(),
                                                chat_id: chat_id.to_string(),
                                                delta: partial.to_string(),
                                            },
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "content_block_stop" => {
                    // Commit completed server tool block (preserves order)
                    if let Some(block) = current_server_block.take() {
                        ordered_content.push(ContentBlock::Unknown(block));
                    }
                    if current_block_type == "tool_use" {
                        let input: serde_json::Value =
                            match serde_json::from_str(&current_tool_input_json) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::warn!(
                                        "[llm] truncated tool call JSON for '{}': {e} (input len={})",
                                        current_tool_name,
                                        current_tool_input_json.len()
                                    );
                                    serde_json::json!({})
                                }
                            };
                        tool_uses.push(ToolUseBlock {
                            id: current_tool_id.clone(),
                            name: current_tool_name.clone(),
                            input,
                        });
                        current_block_type.clear();
                    }
                }
                "message_delta" => {
                    if let Some(delta) = ev.get("delta") {
                        if let Some(sr) = delta["stop_reason"].as_str() {
                            stop_reason = sr.to_string();
                        }
                    }
                    if let Some(usage) = ev.get("usage") {
                        output_tokens = usage["output_tokens"].as_u64().unwrap_or(0);
                        log::info!("anthropic output tokens: {}", output_tokens);
                    }
                }
                "message_stop" => {
                    // Stream complete
                }
                "error" => {
                    let error_msg = ev["error"]["message"]
                        .as_str()
                        .unwrap_or("unknown error");
                    return Err(anyhow::anyhow!("Anthropic stream error: {error_msg}"));
                }
                _ => {}
            }
        }
    }

    // Normalize to output-equivalent tokens by cost ratio
    let tokens_used = {
        let normalized = (output_tokens as f64)
            + (input_tokens as f64 * 0.2)
            + (cache_write_tokens as f64 * 0.25)
            + (cache_read_tokens as f64 * 0.02);
        normalized as u64
    };
    // Flush remaining text
    if !current_text_block.is_empty() {
        ordered_content.push(ContentBlock::text(&current_text_block));
    }

    Ok((text, tool_uses, stop_reason, tokens_used, ordered_content))
}

/// Result of a single streaming turn.
struct StreamOnceResult {
    text: String,
    tool_uses: Vec<ToolUseBlock>,
    stop_reason: String,
    tokens_used: u64,
    /// Content blocks in the order they arrived from the API.
    /// Preserves interleaving of text, server_tool_use, and server_tool_result.
    ordered_content: Vec<ContentBlock>,
}

/// Streaming dispatch: route to provider-specific streaming.
async fn stream_once(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    message_id: &str,
    mcp_snapshot: Option<&super::mcp::McpAppSnapshot>,
) -> anyhow::Result<StreamOnceResult> {
    let (text, tool_uses, stop_reason, tokens_used, ordered_content) =
        anthropic_stream(
            &backend.http, &backend.api_key, &backend.model, system, tool_defs, messages,
            16384, events, instance_slug, chat_id, message_id, mcp_snapshot,
        ).await?;
    Ok(StreamOnceResult { text, tool_uses, stop_reason, tokens_used, ordered_content })
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Build a multimodal Message from text + file attachments.
/// Files are referenced via public URL so the LLM provider can fetch them directly.
/// Falls back to inline text for text files when no public URL is configured.
pub fn build_multimodal_prompt(
    text: &str,
    workspace_dir: &Path,
    instance_slug: &str,
    public_url: &str,
    auth_token: &str,
) -> Message {
    let re = regex::Regex::new(r"\[attached:\s*(.+?)\s*\((\w+)\)\]").unwrap();

    let mut contents: Vec<ContentBlock> = Vec::new();

    // Images first (with labels) — Claude performs best with images before text
    let caps: Vec<_> = re.captures_iter(text).collect();
    let num_images = caps.iter().filter(|c| {
        let uid = &c[2];
        super::uploads::get_upload(workspace_dir, instance_slug, uid)
            .ok().flatten()
            .map(|m| m.mime_type.starts_with("image/"))
            .unwrap_or(false)
    }).count();
    let mut image_idx = 0;

    for cap in &caps {
        let name = &cap[1];
        let upload_id = &cap[2];

        let meta = match super::uploads::get_upload(workspace_dir, instance_slug, upload_id) {
            Ok(Some(m)) => m,
            _ => {
                log::warn!("attachment {upload_id} not found, skipping");
                continue;
            }
        };

        let file_path =
            match super::uploads::get_upload_file_path(workspace_dir, instance_slug, upload_id) {
                Some(p) => p,
                None => {
                    log::warn!("attachment file for {upload_id} missing, skipping");
                    continue;
                }
            };

        let bytes = match std::fs::read(&file_path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("failed to read attachment {upload_id}: {e}");
                continue;
            }
        };

        if meta.mime_type.starts_with("image/") {
            image_idx += 1;
            if num_images > 1 {
                contents.push(ContentBlock::text(&format!("Image {image_idx} ({name}):")));
            }
            if !public_url.is_empty() {
                let url = format!(
                    "{public_url}/public/files/{instance_slug}/{upload_id}?token={auth_token}"
                );
                contents.push(ContentBlock::Image {
                    source: ImageSource::Url { url: url.clone() },
                });
                log::info!("attached image (url): {name} ({url})");
            } else {
                log::warn!("image {name}: no public URL configured, skipping");
                contents.push(ContentBlock::text(format!("[image: {name} — no public URL configured]")));
            }
        } else if meta.mime_type == "application/pdf" {
            if !public_url.is_empty() {
                let url = format!(
                    "{public_url}/public/files/{instance_slug}/{upload_id}?token={auth_token}"
                );
                contents.push(ContentBlock::Document {
                    source: DocumentSource::Url { url: url.clone() },
                });
                log::info!("attached PDF (url): {name} ({url})");
            } else {
                log::warn!("PDF {name}: no public URL configured, skipping");
                contents.push(ContentBlock::text(format!("[PDF: {name} — no public URL configured]")));
            }
        } else if meta.mime_type.starts_with("text/") || meta.mime_type == "application/json" {
            // Text files are small enough to inline directly — works with any provider
            let text_content = String::from_utf8_lossy(&bytes);
            let truncated: String = text_content.chars().take(10_000).collect();
            contents.push(ContentBlock::text(format!(
                "\n--- {name} ---\n{truncated}\n---"
            )));
            log::info!("attached text file (inline): {name} ({} bytes)", bytes.len());
        } else if meta.mime_type == "application/zip" {
            match super::uploads::extract_zip(workspace_dir, instance_slug, upload_id) {
                Ok((extract_dir, files)) => {
                    let mut summary = format!(
                        "\n--- ZIP extracted: {name} ---\n\
                         path: {}\n\
                         {} files:\n",
                        extract_dir.display(),
                        files.len()
                    );
                    for (i, f) in files.iter().enumerate() {
                        if i >= 50 {
                            summary.push_str(&format!(
                                "... and {} more files\n",
                                files.len() - 50
                            ));
                            break;
                        }
                        summary.push_str(&format!("  {f}\n"));
                    }
                    summary.push_str("---\nUse read_file, write_file, list_files, and run_command with the path above to work with this project.");
                    contents.push(ContentBlock::text(summary));
                    log::info!(
                        "extracted zip: {name} → {} ({} files)",
                        extract_dir.display(),
                        files.len()
                    );
                }
                Err(e) => {
                    contents.push(ContentBlock::text(format!(
                        "[zip: {name} — extraction failed: {e}]"
                    )));
                    log::warn!("failed to extract zip {name}: {e}");
                }
            }
        } else if meta.mime_type.starts_with("video/") || meta.mime_type.starts_with("audio/") {
            // Video/audio: tell the LLM about the file and how to analyze it
            let kind = if meta.mime_type.starts_with("video/") { "video" } else { "audio" };
            let size_mb = bytes.len() as f64 / (1024.0 * 1024.0);
            let file_path = super::uploads::get_upload_file_path(workspace_dir, instance_slug, upload_id)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let mime = &meta.mime_type;
            let tool_name = if kind == "audio" { "listen_music" } else { "watch_video" };
            contents.push(ContentBlock::text(format!(
                "[{kind}: {name} — {mime}, {size_mb:.1} MB]\n\
                 local path: {file_path}\n\
                 to analyze this {kind}, call {tool_name} with the local path above.\n\
                 IMPORTANT: in the prompt field, include ALL context you know about this file — \
                 filename, what the user said about it, where it's from, etc. \
                 this context helps the model give a much better analysis."
            )));
            log::info!("attached {kind}: {name} ({}, {size_mb:.1} MB)", meta.mime_type);
        } else {
            contents.push(ContentBlock::text(format!(
                "[file: {name} — {}, {} bytes, binary format]",
                meta.mime_type,
                bytes.len()
            )));
        }
    }

    // Text goes after images (Anthropic best practice: images before text)
    let clean_text = re.replace_all(text, "").trim().to_string();
    if !clean_text.is_empty() {
        contents.push(ContentBlock::text(&clean_text));
    }

    if contents.is_empty() {
        contents.push(ContentBlock::text(text));
    }

    Message::User { content: contents }
}

pub fn load_system_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let soul = super::soul::read_soul(workspace_dir, instance_slug);
    if soul.exists && !soul.content.trim().is_empty() {
        soul.content
    } else {
        DEFAULT_ONBOARDING_PROMPT.to_string()
    }
}

// to_rig_messages removed — history_to_chat_messages replaces it
