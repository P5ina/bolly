use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use futures::StreamExt;
use rig::client::CompletionClient;
use rig::completion::{Chat, Message, Prompt};
use rig::agent::{AgentBuilder, MultiTurnStreamItem, StreamingError};
use rig::completion::message::{AssistantContent, UserContent, ImageMediaType, DocumentMediaType, MimeType};
use rig::one_or_many::OneOrMany;
use rig::providers::{anthropic, openai};
use rig::streaming::{StreamingPrompt, StreamedAssistantContent};
use rig::tool::ToolDyn;
use rig::vector_store::VectorStoreIndexDyn;
use tokio::sync::broadcast;

use crate::domain::events::ServerEvent;

use crate::config::{Config, LlmProvider};
use crate::domain::chat::{ChatMessage, ChatRole};

/// Result of `chat_with_tools`: the final text response.
/// Tool activity is now persisted incrementally by ObservableTool.
#[derive(Clone, Debug)]
pub struct ToolChatResult {
    pub text: String,
    /// True when the agent was cut short by the turn limit (still has work to do).
    pub hit_turn_limit: bool,
}

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 2000;

fn is_rate_limit_error(msg: &str) -> bool {
    msg.contains("429") || msg.contains("rate_limit") || msg.contains("Too Many Requests")
        || msg.contains("529") || msg.contains("overloaded")
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
                log::warn!("Rate limited, retrying in {delay}ms (attempt {attempt}/{MAX_RETRIES})");
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

#[derive(Clone)]
pub enum LlmBackend {
    Anthropic {
        client: anthropic::Client,
        model: String,
    },
    OpenAI {
        client: openai::Client,
        model: String,
    },
}

impl LlmBackend {
    pub fn from_config(config: &Config) -> Option<Self> {
        let provider = config.llm.provider?;
        match provider {
            LlmProvider::Anthropic => {
                let token = &config.llm.tokens.anthropic;
                if token.is_empty() {
                    return None;
                }
                let client = anthropic::Client::builder()
                    .api_key(token)
                    .anthropic_beta("prompt-caching-2024-07-31")
                    .build()
                    .ok()?;
                let model = config
                    .llm
                    .model
                    .clone()
                    .unwrap_or_else(|| "claude-sonnet-4-6".to_string());
                Some(LlmBackend::Anthropic { client, model })
            }
            LlmProvider::OpenAI => {
                let token = &config.llm.tokens.open_ai;
                if token.is_empty() {
                    return None;
                }
                let client = openai::Client::new(token).ok()?;
                let model = config
                    .llm
                    .model
                    .clone()
                    .unwrap_or_else(|| "gpt-5.2".to_string());
                Some(LlmBackend::OpenAI { client, model })
            }
        }
    }

    /// Create a fast/cheap variant for sub-agent tasks (exploration, summarization).
    /// Uses Haiku for Anthropic, gpt-5.2 for OpenAI.
    pub fn fast_variant(&self) -> Self {
        match self {
            LlmBackend::Anthropic { client, .. } => LlmBackend::Anthropic {
                client: client.clone(),
                model: "claude-haiku-4-5-20251001".to_string(),
            },
            LlmBackend::OpenAI { client, .. } => LlmBackend::OpenAI {
                client: client.clone(),
                model: "gpt-5-mini-2025-08-07".to_string(),
            },
        }
    }

    pub fn model_name(&self) -> &str {
        match self {
            LlmBackend::Anthropic { model, .. } => model,
            LlmBackend::OpenAI { model, .. } => model,
        }
    }

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
                match &backend {
                    LlmBackend::Anthropic { client, model } => {
                        let cm = client.completion_model(model).with_prompt_caching();
                        let agent = AgentBuilder::new(cm).preamble(&system).build();
                        Ok(agent.chat(&prompt, history).await?)
                    }
                    LlmBackend::OpenAI { client, model } => {
                        let agent = client.agent(model).preamble(&system).build();
                        Ok(agent.chat(&prompt, history).await?)
                    }
                }
            }
        })
        .await
    }

    /// Result of a tool-using LLM call: the final text response plus any tool
    /// interactions that happened during Rig's internal loop.
    pub async fn chat_with_tools<I>(
        &self,
        system_prompt: &str,
        prompt: Message,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
        memory_index: Option<I>,
    ) -> Result<ToolChatResult, Box<dyn std::error::Error + Send + Sync>>
    where
        I: VectorStoreIndexDyn + Send + Sync + 'static,
    {
        log::info!("chat_with_tools: {} tools registered, memory_rag={}", tools.len(), memory_index.is_some());
        for t in &tools {
            log::debug!("  tool: {}", t.name());
        }

        let prompt_text = match &prompt {
            Message::User { content } => {
                content.iter().find_map(|c| {
                    if let UserContent::Text(t) = c { Some(t.text.clone()) } else { None }
                }).unwrap_or_default()
            }
            _ => String::new(),
        };

        if tools.is_empty() {
            let text = self.chat(system_prompt, &prompt_text, history).await?;
            return Ok(ToolChatResult { text, hit_turn_limit: false });
        }

        // Keep max_turns low so the agent returns text frequently.
        // The outer loop in routes/chat.rs handles continuation.
        const AGENT_MAX_TURNS: usize = 4;

        let (result, chat_history) = match self {
            LlmBackend::Anthropic { client, model } => {
                let cm = client.completion_model(model).with_prompt_caching();
                let mut builder = AgentBuilder::new(cm)
                    .preamble(system_prompt)
                    .tools(tools);
                if let Some(index) = memory_index {
                    builder = builder.dynamic_context(8, index);
                }
                let agent = builder.build();
                let mut chat_history = history.clone();
                let res = agent.prompt(prompt)
                    .with_history(&mut chat_history)
                    .max_turns(AGENT_MAX_TURNS)
                    .await;
                (res, chat_history)
            }
            LlmBackend::OpenAI { client, model } => {
                let mut builder = client
                    .agent(model)
                    .preamble(system_prompt)
                    .tools(tools);
                if let Some(index) = memory_index {
                    builder = builder.dynamic_context(8, index);
                }
                let agent = builder.build();
                let mut chat_history = history.clone();
                let res = agent.prompt(prompt)
                    .with_history(&mut chat_history)
                    .max_turns(AGENT_MAX_TURNS)
                    .await;
                (res, chat_history)
            }
        };

        match result {
            Ok(response) => Ok(ToolChatResult { text: response, hit_turn_limit: false }),
            Err(e) if is_rate_limit_error(&e.to_string()) => {
                // Retry with exponential backoff, keeping tools
                log::warn!("Rate limited during tool agent, retrying with backoff");
                for attempt in 1..=MAX_RETRIES {
                    let delay = INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1);
                    log::info!("Rate limit retry {attempt}/{MAX_RETRIES}, waiting {delay}ms");
                    tokio::time::sleep(Duration::from_millis(delay)).await;

                    let retry_result = match self {
                        LlmBackend::Anthropic { client, model } => {
                            let cm = client.completion_model(model).with_prompt_caching();
                            let agent = AgentBuilder::new(cm)
                                .preamble(system_prompt)
                                .build();
                            agent.chat(&prompt_text, history.clone()).await
                                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })
                        }
                        LlmBackend::OpenAI { client, model } => {
                            let agent = client
                                .agent(model)
                                .preamble(system_prompt)
                                .build();
                            agent.chat(&prompt_text, history.clone()).await
                                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })
                        }
                    };

                    match retry_result {
                        Ok(response) => return Ok(ToolChatResult { text: response, hit_turn_limit: false }),
                        Err(e) if is_rate_limit_error(&e.to_string()) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Err("rate limited — try again in a moment".into())
            }
            Err(e) => {
                // If max turns exceeded, extract last assistant text from chat history
                if is_max_turns_error(&e) {
                    let text = extract_last_assistant_text(&chat_history);
                    log::info!("Turn limit reached, extracted text: {:?}", text.chars().take(80).collect::<String>());
                    return Ok(ToolChatResult { text, hit_turn_limit: true });
                }
                log::error!("Tool agent failed: {e:?}");
                Err(e.into())
            }
        }
    }

    /// Streaming variant of chat_with_tools: sends text deltas via events channel.
    pub async fn chat_with_tools_streaming<I>(
        &self,
        system_prompt: &str,
        prompt: Message,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
        memory_index: Option<I>,
        events: broadcast::Sender<ServerEvent>,
        instance_slug: &str,
        chat_id: &str,
    ) -> Result<ToolChatResult, Box<dyn std::error::Error + Send + Sync>>
    where
        I: VectorStoreIndexDyn + Send + Sync + 'static,
    {
        log::info!("chat_with_tools_streaming: {} tools registered, memory_rag={}", tools.len(), memory_index.is_some());

        const AGENT_MAX_TURNS: usize = 4;

        let slug = instance_slug.to_string();
        let cid = chat_id.to_string();

        match self {
            LlmBackend::Anthropic { client, model } => {
                let cm = client.completion_model(model).with_prompt_caching();
                let mut builder = AgentBuilder::new(cm)
                    .preamble(system_prompt)
                    .tools(tools);
                if let Some(index) = memory_index {
                    builder = builder.dynamic_context(8, index);
                }
                let agent = builder.build();

                let stream = agent
                    .stream_prompt(prompt)
                    .with_history(history)
                    .multi_turn(AGENT_MAX_TURNS)
                    .await;

                consume_stream(stream, &events, &slug, &cid).await
            }
            LlmBackend::OpenAI { client, model } => {
                let mut builder = client
                    .agent(model)
                    .preamble(system_prompt)
                    .tools(tools);
                if let Some(index) = memory_index {
                    builder = builder.dynamic_context(8, index);
                }
                let agent = builder.build();

                let stream = agent
                    .stream_prompt(prompt)
                    .with_history(history)
                    .multi_turn(AGENT_MAX_TURNS)
                    .await;

                consume_stream(stream, &events, &slug, &cid).await
            }
        }
    }

    /// Simplified tool call without memory RAG index.
    /// Used by heartbeat and other contexts that don't need vector search.
    pub async fn chat_with_tools_only(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if tools.is_empty() {
            return self.chat(system_prompt, prompt, history).await;
        }

        let (result, chat_history) = match self {
            LlmBackend::Anthropic { client, model } => {
                let cm = client.completion_model(model).with_prompt_caching();
                let agent = AgentBuilder::new(cm)
                    .preamble(system_prompt)
                    .tools(tools)
                    .build();
                let mut chat_history = history.clone();
                let res = agent.prompt(prompt)
                    .with_history(&mut chat_history)
                    .max_turns(16)
                    .await;
                (res, chat_history)
            }
            LlmBackend::OpenAI { client, model } => {
                let agent = client
                    .agent(model)
                    .preamble(system_prompt)
                    .tools(tools)
                    .build();
                let mut chat_history = history.clone();
                let res = agent.prompt(prompt)
                    .with_history(&mut chat_history)
                    .max_turns(16)
                    .await;
                (res, chat_history)
            }
        };

        match result {
            Ok(response) => Ok(response),
            Err(e) => {
                if is_max_turns_error(&e) {
                    return Ok(extract_last_assistant_text(&chat_history));
                }
                Err(e.into())
            }
        }
    }
}

/// Consume a streaming response, broadcasting text deltas and accumulating the full text.
async fn consume_stream<R>(
    stream: std::pin::Pin<Box<dyn futures::Stream<Item = Result<MultiTurnStreamItem<R>, StreamingError>> + Send>>,
    events: &broadcast::Sender<ServerEvent>,
    instance_slug: &str,
    chat_id: &str,
) -> Result<ToolChatResult, Box<dyn std::error::Error + Send + Sync>>
where
    R: Clone + Unpin,
{
    let mut accumulated = String::new();
    let mut hit_turn_limit = false;

    // Per-item timeout: if the stream stalls for >3 minutes (e.g. tool hang),
    // break out instead of waiting forever.
    const STREAM_ITEM_TIMEOUT: Duration = Duration::from_secs(180);

    tokio::pin!(stream);

    while let Some(item) = tokio::time::timeout(STREAM_ITEM_TIMEOUT, stream.next())
        .await
        .unwrap_or_else(|_| {
            log::warn!("stream item timed out after {}s", STREAM_ITEM_TIMEOUT.as_secs());
            None
        })
    {
        match item {
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(t))) => {
                let delta = t.text;
                if !delta.is_empty() {
                    accumulated.push_str(&delta);
                    let _ = events.send(ServerEvent::ChatStreamDelta {
                        instance_slug: instance_slug.to_string(),
                        chat_id: chat_id.to_string(),
                        delta,
                    });
                }
            }
            // When the agent calls a tool, discard any intermediate text so
            // only the final response (after all tool rounds) is kept.
            Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall { .. })) |
            Ok(MultiTurnStreamItem::StreamUserItem(_)) => {
                if !accumulated.is_empty() {
                    log::debug!("tool round detected, resetting {} chars of intermediate text", accumulated.len());
                    accumulated.clear();
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("MaxTurn") || msg.contains("max turn") {
                    log::info!("streaming hit turn limit, accumulated {} chars", accumulated.len());
                    hit_turn_limit = true;
                    break;
                }
                if is_rate_limit_error(&msg) && accumulated.is_empty() {
                    return Err(Box::new(e));
                }
                // If we already have some text, return it rather than losing it
                if !accumulated.is_empty() {
                    log::warn!("streaming error after partial text: {msg}");
                    break;
                }
                return Err(Box::new(e));
            }
            _ => {} // Ignore tool call deltas, user items, final response
        }
    }

    Ok(ToolChatResult { text: accumulated, hit_turn_limit })
}

fn is_max_turns_error(error: &(dyn std::error::Error + Send + Sync)) -> bool {
    let msg = error.to_string();
    msg.contains("MaxTurnError") || msg.contains("max turn limit")
}

/// Extract the last assistant text from chat history (used when turn limit is hit).
fn extract_last_assistant_text(history: &[Message]) -> String {
    for msg in history.iter().rev() {
        if let Message::Assistant { content, .. } = msg {
            for item in content.iter() {
                if let AssistantContent::Text(t) = item {
                    if !t.text.trim().is_empty() {
                        return t.text.clone();
                    }
                }
            }
        }
    }
    String::new()
}

/// Build a multimodal Message from text + file attachments.
/// Parses [attached: name (upload_id)] references and loads the actual files.
pub fn build_multimodal_prompt(
    text: &str,
    workspace_dir: &Path,
    instance_slug: &str,
) -> Message {
    let re = regex::Regex::new(r"\[attached:\s*(.+?)\s*\((\w+)\)\]").unwrap();

    let mut contents: Vec<UserContent> = Vec::new();

    // Strip attachment markers from text and add as text content
    let clean_text = re.replace_all(text, "").trim().to_string();
    if !clean_text.is_empty() {
        contents.push(UserContent::text(&clean_text));
    }

    // Load each attachment
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

        let file_path = match super::uploads::get_upload_file_path(workspace_dir, instance_slug, upload_id) {
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
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let media = ImageMediaType::from_mime_type(&meta.mime_type);
            contents.push(UserContent::image_base64(b64, media, None));
            log::info!("attached image: {name} ({}, {} bytes)", meta.mime_type, bytes.len());
        } else if meta.mime_type == "application/pdf" {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            contents.push(UserContent::document(b64, Some(DocumentMediaType::PDF)));
            log::info!("attached PDF: {name} ({} bytes)", bytes.len());
        } else if meta.mime_type.starts_with("text/") || meta.mime_type == "application/json" {
            // Inline text files directly
            let text_content = String::from_utf8_lossy(&bytes);
            let truncated: String = text_content.chars().take(10_000).collect();
            contents.push(UserContent::text(format!("\n--- {name} ---\n{truncated}\n---")));
            log::info!("attached text file: {name} ({} bytes)", bytes.len());
        } else if meta.mime_type == "application/zip" {
            // Extract ZIP and tell the LLM where files are
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
                            summary.push_str(&format!("... and {} more files\n", files.len() - 50));
                            break;
                        }
                        summary.push_str(&format!("  {f}\n"));
                    }
                    summary.push_str("---\nUse read_file, write_file, list_files, and run_command with the path above to work with this project.");
                    contents.push(UserContent::text(summary));
                    log::info!("extracted zip: {name} → {} ({} files)", extract_dir.display(), files.len());
                }
                Err(e) => {
                    contents.push(UserContent::text(format!("[zip: {name} — extraction failed: {e}]")));
                    log::warn!("failed to extract zip {name}: {e}");
                }
            }
        } else {
            contents.push(UserContent::text(format!("[file: {name} — {}, {} bytes, binary format]", meta.mime_type, bytes.len())));
        }
    }

    if contents.is_empty() {
        contents.push(UserContent::text(text));
    }

    let content = OneOrMany::many(contents).unwrap_or_else(|_| OneOrMany::one(UserContent::text(text)));
    Message::User { content }
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
