use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use futures::StreamExt;
use tokio::sync::broadcast;

use crate::config::{Config, CHEAP_MODEL, DEFAULT_FAST_MODEL};
use crate::domain::chat::{ChatMessage, ChatRole, MessageKind};
use crate::domain::events::ServerEvent;
use crate::services::tool::{ToolDefinition, ToolDyn};

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

    pub fn image_base64(data: String, media_type: &str) -> Self {
        ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: media_type.to_string(),
                data,
            },
        }
    }

    pub fn document_base64(data: String, media_type: &str) -> Self {
        ContentBlock::Document {
            source: DocumentSource::Base64 {
                media_type: media_type.to_string(),
                data,
            },
        }
    }

    pub fn document_url(url: String) -> Self {
        ContentBlock::Document {
            source: DocumentSource::Url { url },
        }
    }

    pub fn tool_result(tool_use_id: String, content: String) -> Self {
        // ToolDyn blanket impl wraps String output via serde_json::to_string,
        // which adds JSON quotes: `[...]` becomes `"[...]"`. Unwrap that layer.
        let inner = if content.starts_with('"') && content.ends_with('"') {
            serde_json::from_str::<String>(&content).unwrap_or(content)
        } else {
            content
        };

        // If content is a JSON array of content blocks, use directly
        if inner.starts_with('[') {
            if let Ok(blocks) = serde_json::from_str::<serde_json::Value>(&inner) {
                if blocks.is_array() {
                    return ContentBlock::ToolResult {
                        tool_use_id,
                        content: blocks,
                    };
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
                // Image, Document, Unknown — skip for UI
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
pub fn merge_with_timestamps(
    old: &[HistoryEntry],
    new_messages: &[Message],
    ts_fn: impl Fn() -> String,
    id_fn: impl Fn() -> String,
) -> Vec<HistoryEntry> {
    // If the LLM returned fewer messages (compaction happened), don't match old entries —
    // the structure changed and positions no longer correspond.
    let can_match = new_messages.len() >= old.len();

    new_messages.iter().enumerate().map(|(i, msg)| {
        if can_match && i < old.len() {
            // Keep the original entry as-is (preserves clean content without
            // [timestamp] prefixes or [context] blocks that the LLM saw).
            old[i].clone()
        } else {
            HistoryEntry {
                message: msg.clone(),
                ts: Some(ts_fn()),
                id: Some(id_fn()),
                mcp_app_html: None,
                mcp_app_input: None,
                model: None,
            }
        }
    }).collect()
}

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

async fn retry_on_rate_limit<F, Fut, T>(f: F) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
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
pub struct ToolChatResult {
    pub text: String,
    /// Full message history including tool call/result entries.
    pub rig_history: Option<Vec<Message>>,
    /// The message ID used during streaming (so the saved message can reuse it).
    pub message_id: Option<String>,
    /// Total tokens used (input + output) across all turns, from API usage.
    pub tokens_used: u64,
    /// File IDs generated by Agent Skills (code execution).
    pub file_ids: Vec<String>,
}

#[derive(Clone)]
pub struct LlmBackend {
    pub http: reqwest::Client,
    pub api_key: String,
    pub model: String,
}

impl LlmBackend {
    pub fn from_config(config: &Config) -> Option<Self> {
        let api_key = config.llm.api_key()?.to_string();
        let model = config.llm.model_name().to_string();
        let http = reqwest::Client::new();
        Some(Self { http, api_key, model })
    }

    /// Create a variant using the fast/cheap model.
    pub fn fast_variant_with(&self, override_model: Option<&str>) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: override_model.filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_FAST_MODEL).to_string(),
        }
    }

    /// Create a variant using the cheapest model for background tasks (Haiku).
    pub fn cheap_variant(&self) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: CHEAP_MODEL.to_string(),
        }
    }

    pub fn pdf_strategy(&self, public_url: Option<&str>, auth_token: &str) -> PdfStrategy {
        match public_url {
            Some(url) if !url.is_empty() => PdfStrategy::Url {
                base_url: url.to_string(),
                auth_token: auth_token.to_string(),
            },
            _ => PdfStrategy::NativeDocument,
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }


    /// Classify whether a user message needs the heavy model.
    /// Uses the fast model with a minimal prompt. Falls back to heavy on error.
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
    ) -> Result<(String, u64), Box<dyn std::error::Error + Send + Sync>> {
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

    /// Chat with structured JSON output (Anthropic only, falls back to text parsing for other providers).
    pub async fn chat_json(
        &self,
        system_prompt: &str,
        prompt: &str,
        schema: serde_json::Value,
    ) -> Result<(String, u64), Box<dyn std::error::Error + Send + Sync>> {
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
                    return Err(format!("Anthropic API error {status}: {resp_text}").into());
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
    ) -> Result<ToolChatResult, Box<dyn std::error::Error + Send + Sync>> {
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
        )
        .await;

        match result {
            Ok((text, message_id, tokens_used, file_ids)) => Ok(ToolChatResult {
                text,
                rig_history: Some(messages),
                message_id,
                tokens_used,
                file_ids,
            }),
            Err(e) => Err(e),
        }
    }

    /// Simplified tool call (no streaming). Used by heartbeat.
    pub async fn chat_with_tools_only(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> Result<(String, u64), Box<dyn std::error::Error + Send + Sync>> {
        if tools.is_empty() {
            return self.chat(system_prompt, prompt, history).await;
        }
        let system_blocks: &[&str] = &[system_prompt];

        let tool_defs = collect_tool_defs(&tools).await;
        let mut messages = history;
        messages.push(Message::user(prompt));

        agent_loop(self, system_blocks, &tool_defs, &tools, &mut messages).await
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
) -> Result<(String, u64), Box<dyn std::error::Error + Send + Sync>> {
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

        // Execute tools
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        messages.push(Message::User { content: results });
    }
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
) -> Result<(String, Option<String>, u64, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
    let mut all_text = String::new();
    let mut total_tokens: u64 = 0;
    let mut current_message_id = super::chat::next_id();
    let mut active_container_id: Option<String> = None;
    let mut all_file_ids: Vec<String> = Vec::new();

    loop {
        let turn = stream_once(
            backend, system, tool_defs, messages, events,
            instance_slug, chat_id, &current_message_id, mcp_snapshot,
            active_container_id.as_deref(),
        ).await?;

        total_tokens += turn.tokens_used;
        let turn_text = turn.text;
        let tool_uses = turn.tool_uses;
        let stop_reason = turn.stop_reason;

        // Track container_id for skills/code-execution continuations
        if let Some(cid) = turn.container_id {
            active_container_id = Some(cid);
        }
        all_file_ids.extend(turn.file_ids);

        // Build assistant message
        let mut assistant_content = Vec::new();
        if let Some(ref summary) = turn.compaction {
            // Compaction: the API will automatically drop all messages before
            // the compaction block on the NEXT request. We just append the
            // compaction block to the response — don't clear messages ourselves.
            log::info!(
                "[llm] compaction block received — summary: {} chars, messages: {}",
                summary.len(), messages.len()
            );

            if !summary.is_empty() {
                assistant_content.push(ContentBlock::Compaction {
                    content: summary.clone(),
                });
            }
            let _ = events.send(ServerEvent::ContextCompacting {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                messages_compacted: messages.len(),
            });
        }
        if !turn_text.is_empty() {
            assistant_content.push(ContentBlock::text(&turn_text));
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

        // Execute tools
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        messages.push(Message::User { content: results });

        // Persist rig_history after each tool cycle so restarts don't lose context.
        // Load existing entries to preserve timestamps from earlier saves.
        let rig_path = super::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
        let old_entries = super::chat::load_rig_history(&rig_path).unwrap_or_default();
        let ts_fn = || super::tools::unix_millis().to_string();
        let id_fn = || format!("tool_{}", super::tools::unix_millis());
        let entries = merge_with_timestamps(&old_entries, messages, ts_fn, id_fn);
        super::chat::save_rig_history(&rig_path, &entries);

        // Snapshot after each tool cycle — all clients converge to ground truth
        if let Ok(resp) = super::chat::load_messages(workspace_dir, instance_slug, chat_id) {
            let _ = events.send(ServerEvent::ChatSnapshot {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                messages: resp.messages,
                agent_running: true,
            });
        }
    }

    Ok((all_text, Some(current_message_id), total_tokens, all_file_ids))
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
) -> Result<(String, Vec<ToolUseBlock>, String, u64), Box<dyn std::error::Error + Send + Sync>> {
    anthropic_complete(&backend.http, &backend.api_key, &backend.model, system, tool_defs, messages, 16384).await
}

// ═══════════════════════════════════════════════════════════════════════════
// Anthropic API
// ═══════════════════════════════════════════════════════════════════════════

fn anthropic_headers(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-api-key", api_key.parse().unwrap());
    headers.insert("anthropic-beta", "compact-2026-01-12".parse().unwrap());
    headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
    headers.insert("content-type", "application/json".parse().unwrap());
    headers
}

/// Headers with skills beta — only for requests with tools
fn anthropic_headers_with_skills(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-api-key", api_key.parse().unwrap());
    headers.insert("anthropic-beta", "compact-2026-01-12,code-execution-2025-08-25,skills-2025-10-02".parse().unwrap());
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
    container_id: Option<&str>,
) -> serde_json::Value {
    // System blocks — cache_control on stable blocks only (not the volatile last block)
    let block_count = system.iter().filter(|s| !s.is_empty()).count();
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
            let mut block = serde_json::json!({
                "type": "text",
                "text": *s,
            });
            // Only cache non-last blocks (last is volatile time block)
            if i < block_count - 1 {
                block["cache_control"] = serde_json::json!({"type": "ephemeral"});
            }
            block
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
                    // Strip compaction blocks with empty content
                    if block_type == Some("compaction") {
                        let content = block.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        if content.is_empty() {
                            log::info!("stripping empty compaction block");
                            return false;
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

    let mut req = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system_blocks,
        "messages": msgs,
    });

    if !tools.is_empty() {
        req["tools"] = serde_json::Value::Array(tools);
    }

    // Agent Skills (code execution + document generation) — only for streaming chat
    if stream && !tool_defs.is_empty() {
        let tools_arr = req["tools"].as_array_mut().unwrap();
        tools_arr.push(serde_json::json!({
            "type": "code_execution_20250825",
            "name": "code_execution"
        }));

        let skills = serde_json::json!([
            {"type": "anthropic", "skill_id": "pptx", "version": "latest"},
            {"type": "anthropic", "skill_id": "xlsx", "version": "latest"},
            {"type": "anthropic", "skill_id": "docx", "version": "latest"},
            {"type": "anthropic", "skill_id": "pdf", "version": "latest"},
        ]);
        if let Some(cid) = container_id {
            req["container"] = serde_json::json!({"id": cid, "skills": skills});
        } else {
            req["container"] = serde_json::json!({"skills": skills});
        }
    }
    if stream {
        req["stream"] = serde_json::json!(true);
    }
    // Server-side context compaction
    if model.contains("opus-4") || model.contains("sonnet-4") {
        req["context_management"] = serde_json::json!({
            "edits": [{"type": "compact_20260112"}]
        });
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
) -> Result<(String, Vec<ToolUseBlock>, String, u64), Box<dyn std::error::Error + Send + Sync>> {
    let body = build_anthropic_request(model, system, tool_defs, messages, max_tokens, false, api_key, None);

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
        return Err(format!("Anthropic API error {status}: {resp_text}").into());
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
    container_id: Option<&str>,
) -> Result<(String, Vec<ToolUseBlock>, String, Option<String>, u64, Option<String>, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
    let body = build_anthropic_request(model, system, tool_defs, messages, max_tokens, true, api_key, container_id);

    // Use skills headers for streaming requests (they have tools + code_execution)
    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .headers(anthropic_headers_with_skills(api_key))
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
        return Err(format!("Anthropic API error {status}: {err_text}").into());
    }

    let mut text = String::new();
    let mut tool_uses: Vec<ToolUseBlock> = Vec::new();
    let mut stop_reason = String::new();
    let mut compaction_summary: Option<String> = None;
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_write_tokens: u64 = 0;
    let mut container_id: Option<String> = None;
    let mut file_ids: Vec<String> = Vec::new();

    // Current block being built
    let mut current_block_type = String::new();
    let mut current_tool_id = String::new();
    let mut current_tool_name = String::new();
    let mut current_tool_input_json = String::new();
    let mut current_compaction_text = String::new();
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
                            log::info!(
                                "anthropic cache: read={} write={} input={}",
                                cache_read_tokens, cache_write_tokens, input_tokens,
                            );
                        }
                        // Capture container.id for skills/code-execution
                        if let Some(cid) = msg.pointer("/container/id").and_then(|v| v.as_str()) {
                            container_id = Some(cid.to_string());
                            log::info!("[llm] container_id: {cid}");
                        }
                    }
                }
                "content_block_start" => {
                    if let Some(block) = ev.get("content_block") {
                        current_block_type =
                            block["type"].as_str().unwrap_or("").to_string();

                        // Extract file_ids from code execution results
                        if let Some(fid) = block.get("file_id").and_then(|v| v.as_str()) {
                            file_ids.push(fid.to_string());
                            log::info!("[llm] file generated: {fid}");
                        }
                        // Also check nested content for file refs
                        if let Some(content) = block.get("content") {
                            if let Some(arr) = content.as_array() {
                                for item in arr {
                                    if let Some(fid) = item.get("file_id").and_then(|v| v.as_str()) {
                                        file_ids.push(fid.to_string());
                                        log::info!("[llm] file generated (nested): {fid}");
                                    }
                                }
                            }
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
                                    if current_block_type == "compaction" {
                                        current_compaction_text.push_str(t);
                                    } else {
                                        text.push_str(t);
                                        let _ = events.send(ServerEvent::ChatStreamDelta {
                                            instance_slug: instance_slug.to_string(),
                                            chat_id: chat_id.to_string(),
                                            message_id: message_id.to_string(),
                                            delta: t.to_string(),
                                        });
                                    }
                                }
                            }
                            Some("summary_delta") => {
                                if let Some(s) = delta["summary"].as_str() {
                                    current_compaction_text.push_str(s);
                                }
                            }
                            Some("compaction_delta") => {
                                if let Some(c) = delta["content"].as_str() {
                                    current_compaction_text.push_str(c);
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
                    if current_block_type == "compaction" {
                        log::info!(
                            "[llm] context compaction triggered — summary length: {} chars",
                            current_compaction_text.len()
                        );
                        compaction_summary = Some(current_compaction_text.clone());
                        current_compaction_text.clear();
                        current_block_type.clear();
                    } else if current_block_type == "tool_use" {
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
                    return Err(format!("Anthropic stream error: {error_msg}").into());
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
    Ok((text, tool_uses, stop_reason, compaction_summary, tokens_used, container_id, file_ids))
}

/// Result of a single streaming turn.
struct StreamOnceResult {
    text: String,
    tool_uses: Vec<ToolUseBlock>,
    stop_reason: String,
    compaction: Option<String>,
    tokens_used: u64,
    container_id: Option<String>,
    file_ids: Vec<String>,
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
    container_id: Option<&str>,
) -> Result<StreamOnceResult, Box<dyn std::error::Error + Send + Sync>> {
    let (text, tool_uses, stop_reason, compaction, tokens_used, container_id, file_ids) =
        anthropic_stream(
            &backend.http, &backend.api_key, &backend.model, system, tool_defs, messages,
            16384, events, instance_slug, chat_id, message_id, mcp_snapshot, container_id,
        ).await?;
    Ok(StreamOnceResult { text, tool_uses, stop_reason, compaction, tokens_used, container_id, file_ids })
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// How to send PDFs to the LLM.
pub enum PdfStrategy {
    /// Send base64 document block.
    NativeDocument,
    /// Send a public URL.
    Url {
        base_url: String,
        auth_token: String,
    },
}


/// Build a multimodal Message from text + file attachments.
pub fn build_multimodal_prompt(
    text: &str,
    workspace_dir: &Path,
    instance_slug: &str,
    pdf_strategy: &PdfStrategy,
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
            // Label each image for better multi-image attention (Anthropic best practice)
            if num_images > 1 {
                contents.push(ContentBlock::text(&format!("Image {image_idx} ({name}):")));
            }
            if let PdfStrategy::Url { base_url, auth_token } = pdf_strategy {
                let url = format!(
                    "{base_url}/public/files/{instance_slug}/{upload_id}?token={auth_token}"
                );
                contents.push(ContentBlock::Image {
                    source: ImageSource::Url { url },
                });
                log::info!("attached image (URL): {name} ({}, {} bytes)", meta.mime_type, bytes.len());
            } else {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                contents.push(ContentBlock::image_base64(b64, &meta.mime_type));
                log::info!("attached image (base64): {name} ({}, {} bytes)", meta.mime_type, bytes.len());
            }
        } else if meta.mime_type == "application/pdf" {
            match pdf_strategy {
                PdfStrategy::NativeDocument => {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    contents.push(ContentBlock::document_base64(b64, "application/pdf"));
                    log::info!("attached PDF (native document): {name} ({} bytes)", bytes.len());
                }
                PdfStrategy::Url {
                    base_url,
                    auth_token,
                } => {
                    let url = format!(
                        "{base_url}/public/files/{instance_slug}/{upload_id}?token={auth_token}"
                    );
                    contents.push(ContentBlock::document_url(url));
                    log::info!("attached PDF (URL): {name} ({} bytes)", bytes.len());
                }
            }
        } else if meta.mime_type.starts_with("text/") || meta.mime_type == "application/json" {
            let text_content = String::from_utf8_lossy(&bytes);
            let truncated: String = text_content.chars().take(10_000).collect();
            contents.push(ContentBlock::text(format!(
                "\n--- {name} ---\n{truncated}\n---"
            )));
            log::info!("attached text file: {name} ({} bytes)", bytes.len());
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
