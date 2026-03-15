use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use futures::StreamExt;
use tokio::sync::broadcast;

use crate::config::{Config, LlmProvider};
use crate::domain::chat::{ChatMessage, ChatRole};
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

    pub fn document_url(url: String, media_type: &str) -> Self {
        ContentBlock::Document {
            source: DocumentSource::Url {
                url,
                media_type: media_type.to_string(),
            },
        }
    }

    pub fn tool_result(tool_use_id: String, content: String) -> Self {
        ContentBlock::ToolResult {
            tool_use_id,
            content: serde_json::Value::String(content),
        }
    }

    pub fn tool_result_error(tool_use_id: String, error: String) -> Self {
        ContentBlock::ToolResult {
            tool_use_id,
            content: serde_json::Value::String(error),
        }
    }
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
    Url { url: String, media_type: String },
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
}

#[derive(Clone)]
pub enum LlmBackend {
    Anthropic {
        http: reqwest::Client,
        api_key: String,
        model: String,
    },
    OpenAI {
        http: reqwest::Client,
        api_key: String,
        model: String,
        base_url: String,
    },
}

impl LlmBackend {
    pub fn from_config(config: &Config) -> Option<Self> {
        let provider = config.llm.provider?;
        match provider {
            LlmProvider::Anthropic => {
                let api_key = config.llm.tokens.anthropic.clone();
                if api_key.is_empty() {
                    return None;
                }
                let model = config
                    .llm
                    .model
                    .clone()
                    .unwrap_or_else(|| "claude-sonnet-4-6".to_string());
                Some(LlmBackend::Anthropic {
                    http: reqwest::Client::new(),
                    api_key,
                    model,
                })
            }
            LlmProvider::OpenAI => {
                let api_key = config.llm.tokens.open_ai.clone();
                if api_key.is_empty() {
                    return None;
                }
                let model = config
                    .llm
                    .model
                    .clone()
                    .unwrap_or_else(|| "gpt-5.2".to_string());
                Some(LlmBackend::OpenAI {
                    http: reqwest::Client::new(),
                    api_key,
                    model,
                    base_url: "https://api.openai.com/v1".to_string(),
                })
            }
            LlmProvider::OpenRouter => {
                let api_key = config.llm.tokens.open_router.clone();
                if api_key.is_empty() {
                    return None;
                }
                let model = config
                    .llm
                    .model
                    .clone()
                    .unwrap_or_else(|| "anthropic/claude-sonnet-4-6".to_string());
                Some(LlmBackend::OpenAI {
                    http: reqwest::Client::new(),
                    api_key,
                    model,
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                })
            }
        }
    }

    pub fn fast_variant(&self) -> Self {
        match self {
            LlmBackend::Anthropic { http, api_key, .. } => LlmBackend::Anthropic {
                http: http.clone(),
                api_key: api_key.clone(),
                model: "claude-haiku-4-5-20251001".to_string(),
            },
            LlmBackend::OpenAI {
                http,
                api_key,
                base_url,
                ..
            } => LlmBackend::OpenAI {
                http: http.clone(),
                api_key: api_key.clone(),
                model: if base_url.contains("openrouter") {
                    "anthropic/claude-sonnet-4-6".to_string()
                } else {
                    "gpt-5-mini-2025-08-07".to_string()
                },
                base_url: base_url.clone(),
            },
        }
    }

    pub fn pdf_strategy(&self, public_url: Option<&str>, auth_token: &str) -> PdfStrategy {
        match public_url {
            Some(url) if !url.is_empty() => PdfStrategy::Url {
                base_url: url.to_string(),
                auth_token: auth_token.to_string(),
            },
            _ => match self {
                LlmBackend::Anthropic { .. } => PdfStrategy::NativeDocument,
                _ => PdfStrategy::ExtractText,
            },
        }
    }

    pub fn model_name(&self) -> &str {
        match self {
            LlmBackend::Anthropic { model, .. } => model,
            LlmBackend::OpenAI { model, .. } => model,
        }
    }

    /// Simple chat without tools.
    pub async fn chat(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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
                match &backend {
                    LlmBackend::Anthropic {
                        http,
                        api_key,
                        model,
                    } => anthropic_complete(http, api_key, model, &[&system], &[], &messages, 16384)
                        .await
                        .map(|(text, _)| text),
                    LlmBackend::OpenAI {
                        http,
                        api_key,
                        model,
                        base_url,
                    } => {
                        openai_complete(http, api_key, base_url, model, &system, &[], &messages, 16384)
                            .await
                            .map(|(text, _)| text)
                    }
                }
            }
        })
        .await
    }

    /// Chat with tools (non-streaming agent loop).
    pub async fn chat_with_tools(
        &self,
        system_prompt: &[&str],
        prompt: Message,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> Result<ToolChatResult, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("chat_with_tools: {} tools registered", tools.len());

        let prompt_text = extract_text_from_message(&prompt);
        if tools.is_empty() {
            let joined = system_prompt.join("\n\n");
            let text = self.chat(&joined, &prompt_text, history).await?;
            return Ok(ToolChatResult {
                text,
                rig_history: None,
            });
        }

        const MAX_TURNS: usize = 4;
        let tool_defs = collect_tool_defs(&tools).await;
        let mut messages = history;
        // Add the user prompt as the last message
        if let Message::User { content } = prompt {
            messages.push(Message::User { content });
        }

        let result = agent_loop(self, system_prompt, &tool_defs, &tools, &mut messages, MAX_TURNS).await;

        match result {
            Ok(text) => Ok(ToolChatResult {
                text,
                rig_history: Some(messages),
            }),
            Err(e) if is_rate_limit_error(&e.to_string()) => {
                // Retry with backoff (simple chat, no tools)
                log::warn!("Rate limited during tool agent, retrying with backoff");
                for attempt in 1..=MAX_RETRIES {
                    let delay = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1);
                    log::info!("Rate limit retry {attempt}/{MAX_RETRIES}, waiting {delay}ms");
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    let fallback_sys = system_prompt.join("\n\n");
                    match self.chat(&fallback_sys, &prompt_text, vec![]).await {
                        Ok(text) => {
                            return Ok(ToolChatResult {
                                text,
                                rig_history: None,
                            })
                        }
                        Err(e) if is_rate_limit_error(&e.to_string()) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Err("rate limited — try again in a moment".into())
            }
            Err(e) => Err(e),
        }
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

        const MAX_TURNS: usize = 16;
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
            MAX_TURNS,
            &events,
            instance_slug,
            chat_id,
            workspace_dir,
            mcp_snapshot.as_ref(),
        )
        .await;

        match result {
            Ok(text) => Ok(ToolChatResult {
                text,
                rig_history: Some(messages),
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
        max_turns: usize,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if tools.is_empty() {
            return self.chat(system_prompt, prompt, history).await;
        }
        let system_blocks: &[&str] = &[system_prompt];

        let tool_defs = collect_tool_defs(&tools).await;
        let mut messages = history;
        messages.push(Message::user(prompt));

        agent_loop(self, system_blocks, &tool_defs, &tools, &mut messages, max_turns).await
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

/// Non-streaming agent loop. Returns final text.
async fn agent_loop(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    tools: &[Box<dyn ToolDyn>],
    messages: &mut Vec<Message>,
    max_turns: usize,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    for _turn in 0..max_turns {
        let (text, tool_uses, stop_reason) = complete_once(backend, system, tool_defs, messages).await?;

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

        if stop_reason != "tool_use" || tool_uses.is_empty() {
            return Ok(text);
        }

        // Execute tools
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        messages.push(Message::User { content: results });
    }

    // Turn limit reached — extract last text
    Ok(extract_last_assistant_text(messages))
}

/// Streaming agent loop. Returns final accumulated text.
async fn streaming_agent_loop(
    backend: &LlmBackend,
    system: &[&str],
    tool_defs: &[ToolDefinition],
    tools: &[Box<dyn ToolDyn>],
    messages: &mut Vec<Message>,
    max_turns: usize,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
    workspace_dir: &Path,
    mcp_snapshot: Option<&super::mcp::McpAppSnapshot>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut all_text = String::new();

    for _turn in 0..max_turns {
        let (turn_text, tool_uses, stop_reason, compaction) = stream_once(
            backend,
            system,
            tool_defs,
            messages,
            events,
            instance_slug,
            chat_id,
            mcp_snapshot,
        )
        .await?;

        // Build assistant message
        let mut assistant_content = Vec::new();
        if let Some(ref summary) = compaction {
            assistant_content.push(ContentBlock::Compaction {
                content: summary.clone(),
            });
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
            // Accumulate text across continuation turns
            all_text.push_str(&turn_text);
            // Don't execute truncated tool calls — ask LLM to continue instead
            messages.push(Message::User {
                content: vec![ContentBlock::text(
                    "[system: your previous response was cut off due to length. please continue exactly where you left off.]",
                )],
            });
            continue;
        }

        // For the final turn (no more tool use), only keep this turn's text.
        // Intermediate texts from tool-use turns are saved individually below.
        all_text = turn_text.clone();

        if stop_reason != "tool_use" || tool_uses.is_empty() {
            break;
        }

        // Save intermediate text before tool execution
        if !turn_text.trim().is_empty() {
            let ts = super::tools::unix_millis();
            let msg = ChatMessage {
                id: format!("im_{ts}"),
                role: ChatRole::Assistant,
                content: turn_text.trim().to_string(),
                created_at: ts.to_string(),
                kind: Default::default(),
                tool_name: None,
                mcp_app_html: None,
                mcp_app_input: None,
            };
            super::tools::append_message_to_chat(workspace_dir, instance_slug, chat_id, &msg);
            let _ = events.send(ServerEvent::ChatMessageCreated {
                instance_slug: instance_slug.to_string(),
                chat_id: chat_id.to_string(),
                message: msg,
            });
        }

        // Execute tools
        let mut results = Vec::new();
        for tu in &tool_uses {
            let content = execute_tool(tools, &tu.name, &tu.input).await;
            results.push(ContentBlock::tool_result(tu.id.clone(), content));
        }
        messages.push(Message::User { content: results });

        // Persist rig_history after each tool cycle so restarts don't lose context
        let rig_path = super::chat::rig_history_path(workspace_dir, instance_slug, chat_id);
        super::chat::save_rig_history(&rig_path, messages);
    }

    Ok(all_text)
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
) -> Result<(String, Vec<ToolUseBlock>, String), Box<dyn std::error::Error + Send + Sync>> {
    match backend {
        LlmBackend::Anthropic {
            http,
            api_key,
            model,
        } => {
            let (text, stop_reason) =
                anthropic_complete(http, api_key, model, system, tool_defs, messages, 16384)
                    .await?;
            // For non-streaming, parse tool_use from response
            // The anthropic_complete returns only text; for tool use we need the full response
            Ok((text, vec![], stop_reason))
        }
        LlmBackend::OpenAI {
            http,
            api_key,
            model,
            base_url,
        } => {
            let joined = system.join("\n\n");
            let (text, stop_reason) =
                openai_complete(http, api_key, base_url, model, &joined, tool_defs, messages, 16384).await?;
            Ok((text, vec![], stop_reason))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Anthropic API
// ═══════════════════════════════════════════════════════════════════════════

fn anthropic_headers(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-api-key", api_key.parse().unwrap());
    headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
    headers.insert("anthropic-beta", "compact-2026-01-12".parse().unwrap());
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
) -> serde_json::Value {
    // System blocks — no manual cache_control needed, Anthropic auto-caches
    let system_blocks: Vec<serde_json::Value> = system
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::json!({"type": "text", "text": *s}))
        .collect();

    // Tool definitions
    let tools: Vec<serde_json::Value> = tool_defs
        .iter()
        .map(|td| {
            serde_json::json!({
                "name": td.name,
                "description": td.description,
                "input_schema": td.parameters,
            })
        })
        .collect();

    // Messages — strip any legacy oversized base64 images
    let mut msgs = serde_json::to_value(messages).unwrap_or(serde_json::json!([]));
    if let Some(arr) = msgs.as_array_mut() {
        for msg in arr.iter_mut() {
            if let Some(content_arr) = msg.get_mut("content").and_then(|c| c.as_array_mut()) {
                content_arr.retain(|block| {
                    if block.get("type").and_then(|t| t.as_str()) == Some("image") {
                        if let Some(data) = block.pointer("/source/data").and_then(|d| d.as_str()) {
                            if data.len() > 5 * 1024 * 1024 {
                                log::info!("stripping oversized base64 image ({} bytes)", data.len());
                                return false;
                            }
                        }
                    }
                    true
                });
            }
        }
    }

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
    if stream {
        req["stream"] = serde_json::json!(true);
    }
    // Server-side context compaction — only supported on opus/sonnet 4.6+
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
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let body = build_anthropic_request(model, system, tool_defs, messages, max_tokens, false);

    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .headers(anthropic_headers(api_key))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_text = resp.text().await?;
    if !status.is_success() {
        return Err(format!("Anthropic API error {status}: {resp_text}").into());
    }

    let resp_json: serde_json::Value = serde_json::from_str(&resp_text)?;
    let stop_reason = resp_json["stop_reason"]
        .as_str()
        .unwrap_or("end_turn")
        .to_string();

    if let Some(usage) = resp_json.get("usage") {
        log::info!(
            "anthropic usage: input={} cache_read={} cache_write={} output={}",
            usage["input_tokens"].as_u64().unwrap_or(0),
            usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
            usage["cache_creation_input_tokens"].as_u64().unwrap_or(0),
            usage["output_tokens"].as_u64().unwrap_or(0),
        );
    }

    // Extract text from content blocks
    let text = resp_json["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| {
                    if b["type"].as_str() == Some("text") {
                        b["text"].as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    Ok((text, stop_reason))
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
    mcp_snapshot: Option<&super::mcp::McpAppSnapshot>,
) -> Result<(String, Vec<ToolUseBlock>, String, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
    let body = build_anthropic_request(model, system, tool_defs, messages, max_tokens, true);

    let resp = http
        .post("https://api.anthropic.com/v1/messages")
        .headers(anthropic_headers(api_key))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error {status}: {err_text}").into());
    }

    let mut text = String::new();
    let mut tool_uses: Vec<ToolUseBlock> = Vec::new();
    let mut stop_reason = String::new();
    let mut compaction_summary: Option<String> = None;

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
                    if let Some(usage) = ev.get("message").and_then(|m| m.get("usage")) {
                        log::info!(
                            "anthropic cache: read={} write={} input={}",
                            usage["cache_read_input_tokens"].as_u64().unwrap_or(0),
                            usage["cache_creation_input_tokens"].as_u64().unwrap_or(0),
                            usage["input_tokens"].as_u64().unwrap_or(0),
                        );
                    }
                }
                "content_block_start" => {
                    if let Some(block) = ev.get("content_block") {
                        current_block_type =
                            block["type"].as_str().unwrap_or("").to_string();
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
                                        snap.get_app_html(&current_tool_name)
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
                        log::info!(
                            "anthropic output tokens: {}",
                            usage["output_tokens"].as_u64().unwrap_or(0)
                        );
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

    Ok((text, tool_uses, stop_reason, compaction_summary))
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
    mcp_snapshot: Option<&super::mcp::McpAppSnapshot>,
) -> Result<(String, Vec<ToolUseBlock>, String, Option<String>), Box<dyn std::error::Error + Send + Sync>> {
    match backend {
        LlmBackend::Anthropic {
            http,
            api_key,
            model,
        } => {
            anthropic_stream(
                http,
                api_key,
                model,
                system,
                tool_defs,
                messages,
                16384,
                events,
                instance_slug,
                chat_id,
                mcp_snapshot,
            )
            .await
        }
        LlmBackend::OpenAI {
            http,
            api_key,
            model,
            base_url,
        } => {
            let joined = system.join("\n\n");
            let (text, tool_uses, stop_reason) = openai_stream(
                http,
                api_key,
                base_url,
                model,
                &joined,
                tool_defs,
                messages,
                16384,
                events,
                instance_slug,
                chat_id,
            )
            .await?;
            Ok((text, tool_uses, stop_reason, None))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OpenAI / OpenRouter API
// ═══════════════════════════════════════════════════════════════════════════

fn openai_headers(api_key: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {api_key}").parse().unwrap(),
    );
    headers.insert("content-type", "application/json".parse().unwrap());
    headers
}

fn build_openai_request(
    model: &str,
    system: &str,
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
    stream: bool,
) -> serde_json::Value {
    // Convert messages to OpenAI format
    let mut oai_msgs = vec![serde_json::json!({"role": "system", "content": system})];

    for msg in messages {
        match msg {
            Message::User { content } => {
                // Check for tool results
                let has_tool_results = content
                    .iter()
                    .any(|c| matches!(c, ContentBlock::ToolResult { .. }));
                if has_tool_results {
                    for c in content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content: result_content,
                        } = c
                        {
                            let text = match result_content {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            oai_msgs.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": text,
                            }));
                        }
                    }
                } else {
                    // Build content array
                    let oai_content: Vec<serde_json::Value> = content
                        .iter()
                        .filter_map(|c| match c {
                            ContentBlock::Text { text } => {
                                Some(serde_json::json!({"type": "text", "text": text}))
                            }
                            ContentBlock::Image {
                                source: ImageSource::Base64 { media_type, data },
                            } => Some(serde_json::json!({
                                "type": "image_url",
                                "image_url": {"url": format!("data:{media_type};base64,{data}")}
                            })),
                            _ => None,
                        })
                        .collect();
                    if oai_content.len() == 1 {
                        if let Some(text) = oai_content[0]["text"].as_str() {
                            oai_msgs.push(serde_json::json!({"role": "user", "content": text}));
                        } else {
                            oai_msgs.push(
                                serde_json::json!({"role": "user", "content": oai_content}),
                            );
                        }
                    } else {
                        oai_msgs
                            .push(serde_json::json!({"role": "user", "content": oai_content}));
                    }
                }
            }
            Message::Assistant { content } => {
                let text_parts: Vec<&str> = content
                    .iter()
                    .filter_map(|c| {
                        if let ContentBlock::Text { text } = c {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                let tool_calls: Vec<serde_json::Value> = content
                    .iter()
                    .filter_map(|c| {
                        if let ContentBlock::ToolUse { id, name, input } = c {
                            Some(serde_json::json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": serde_json::to_string(input).unwrap_or_default(),
                                }
                            }))
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut msg = serde_json::json!({
                    "role": "assistant",
                    "content": if text_parts.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(text_parts.join("")) },
                });
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = serde_json::Value::Array(tool_calls);
                }
                oai_msgs.push(msg);
            }
        }
    }

    // Tool definitions
    let tools: Vec<serde_json::Value> = tool_defs
        .iter()
        .map(|td| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": td.name,
                    "description": td.description,
                    "parameters": td.parameters,
                }
            })
        })
        .collect();

    let mut req = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": oai_msgs,
    });
    if !tools.is_empty() {
        req["tools"] = serde_json::Value::Array(tools);
    }
    if stream {
        req["stream"] = serde_json::json!(true);
    }
    req
}

/// Non-streaming OpenAI call. Returns (text, stop_reason).
async fn openai_complete(
    http: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    system: &str,
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let body = build_openai_request(model, system, tool_defs, messages, max_tokens, false);

    let resp = http
        .post(format!("{base_url}/chat/completions"))
        .headers(openai_headers(api_key))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let resp_text = resp.text().await?;
    if !status.is_success() {
        return Err(format!("OpenAI API error {status}: {resp_text}").into());
    }

    let resp_json: serde_json::Value = serde_json::from_str(&resp_text)?;
    let choice = &resp_json["choices"][0];
    let text = choice["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let stop_reason = choice["finish_reason"]
        .as_str()
        .unwrap_or("stop")
        .to_string();

    // Map OpenAI stop reasons to our standard
    let stop_reason = match stop_reason.as_str() {
        "tool_calls" => "tool_use".to_string(),
        "length" => "max_tokens".to_string(),
        _ => "end_turn".to_string(),
    };

    Ok((text, stop_reason))
}

/// Streaming OpenAI call. Returns (text, tool_uses, stop_reason).
async fn openai_stream(
    http: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    model: &str,
    system: &str,
    tool_defs: &[ToolDefinition],
    messages: &[Message],
    max_tokens: u64,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
) -> Result<(String, Vec<ToolUseBlock>, String), Box<dyn std::error::Error + Send + Sync>> {
    let body = build_openai_request(model, system, tool_defs, messages, max_tokens, true);

    let resp = http
        .post(format!("{base_url}/chat/completions"))
        .headers(openai_headers(api_key))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error {status}: {err_text}").into());
    }

    let mut text = String::new();
    let mut tool_uses: Vec<ToolUseBlock> = Vec::new();
    let mut stop_reason = String::new();

    // Track tool calls being built
    let mut tool_call_map: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new(); // index -> (id, name, args_json)

    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();

    const STREAM_TIMEOUT: Duration = Duration::from_secs(480);

    loop {
        let chunk = tokio::time::timeout(STREAM_TIMEOUT, stream.next()).await;
        let chunk = match chunk {
            Ok(Some(Ok(c))) => c,
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(None) => break,
            Err(_) => {
                log::warn!("OpenAI stream timed out");
                break;
            }
        };

        buf.extend_from_slice(&chunk);

        while let Some(newline_pos) = buf.iter().position(|&b| b == b'\n') {
            let line = String::from_utf8_lossy(&buf[..newline_pos]).to_string();
            buf = buf[newline_pos + 1..].to_vec();

            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }

            let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };

            if let Some(choice) = ev["choices"].as_array().and_then(|a| a.first()) {
                // Text delta
                if let Some(content) = choice["delta"]["content"].as_str() {
                    text.push_str(content);
                    let _ = events.send(ServerEvent::ChatStreamDelta {
                        instance_slug: instance_slug.to_string(),
                        chat_id: chat_id.to_string(),
                        delta: content.to_string(),
                    });
                }

                // Tool call deltas
                if let Some(tool_calls) = choice["delta"]["tool_calls"].as_array() {
                    for tc in tool_calls {
                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                        let entry = tool_call_map
                            .entry(idx)
                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                        if let Some(id) = tc["id"].as_str() {
                            entry.0 = id.to_string();
                        }
                        if let Some(name) = tc["function"]["name"].as_str() {
                            entry.1 = name.to_string();
                        }
                        if let Some(args) = tc["function"]["arguments"].as_str() {
                            entry.2.push_str(args);
                        }
                    }
                }

                // Finish reason
                if let Some(fr) = choice["finish_reason"].as_str() {
                    stop_reason = match fr {
                        "tool_calls" => "tool_use".to_string(),
                        "length" => "max_tokens".to_string(),
                        _ => "end_turn".to_string(),
                    };
                }
            }
        }
    }

    // Finalize tool calls
    for (_, (id, name, args_json)) in tool_call_map {
        let input: serde_json::Value = match serde_json::from_str(&args_json) {
            Ok(v) => v,
            Err(e) => {
                log::warn!(
                    "[llm] truncated tool call JSON for '{}': {e} (input len={})",
                    name,
                    args_json.len()
                );
                serde_json::json!({})
            }
        };
        tool_uses.push(ToolUseBlock { id, name, input });
    }

    Ok((text, tool_uses, stop_reason))
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn extract_text_from_message(msg: &Message) -> String {
    match msg {
        Message::User { content } | Message::Assistant { content } => content
            .iter()
            .find_map(|c| {
                if let ContentBlock::Text { text } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default(),
    }
}

fn extract_last_assistant_text(messages: &[Message]) -> String {
    for msg in messages.iter().rev() {
        if let Message::Assistant { content } = msg {
            for block in content {
                if let ContentBlock::Text { text } = block {
                    if !text.trim().is_empty() {
                        return text.clone();
                    }
                }
            }
        }
    }
    String::new()
}

/// How to send PDFs to the LLM provider.
pub enum PdfStrategy {
    /// Anthropic: send base64 document block.
    NativeDocument,
    /// OpenRouter/others: send a public URL.
    Url {
        base_url: String,
        auth_token: String,
    },
    /// Fallback: extract text from PDF.
    ExtractText,
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

    let clean_text = re.replace_all(text, "").trim().to_string();
    if !clean_text.is_empty() {
        contents.push(ContentBlock::text(&clean_text));
    }

    for cap in re.captures_iter(text) {
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
                    contents.push(ContentBlock::document_url(url, "application/pdf"));
                    log::info!("attached PDF (URL): {name} ({} bytes)", bytes.len());
                }
                PdfStrategy::ExtractText => {
                    let extracted = pdf_extract::extract_text_from_mem(&bytes).unwrap_or_default();
                    if extracted.trim().is_empty() {
                        contents.push(ContentBlock::text(format!(
                            "[PDF: {name} — could not extract text, {} bytes]",
                            bytes.len()
                        )));
                    } else {
                        let truncated: String = extracted.chars().take(15_000).collect();
                        let suffix = if extracted.chars().count() > 15_000 {
                            "\n...(truncated)"
                        } else {
                            ""
                        };
                        contents.push(ContentBlock::text(format!(
                            "\n--- PDF: {name} ---\n{truncated}{suffix}\n---"
                        )));
                    }
                    log::info!(
                        "attached PDF (text extracted): {name} ({} bytes)",
                        bytes.len()
                    );
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
        } else {
            contents.push(ContentBlock::text(format!(
                "[file: {name} — {}, {} bytes, binary format]",
                meta.mime_type,
                bytes.len()
            )));
        }
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

pub fn to_rig_messages(messages: &[ChatMessage]) -> Vec<Message> {
    messages
        .iter()
        .map(|m| match m.role {
            ChatRole::User => Message::user(&m.content),
            ChatRole::Assistant => Message::assistant(&m.content),
        })
        .collect()
}
