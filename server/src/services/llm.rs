use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use rig::client::CompletionClient;
use rig::completion::{Chat, Message, Prompt};
use rig::agent::AgentBuilder;
use rig::completion::message::{AssistantContent, UserContent, ImageMediaType, DocumentMediaType, MimeType};
use rig::one_or_many::OneOrMany;
use rig::providers::{anthropic, openai};
use rig::tool::ToolDyn;
use rig::vector_store::VectorStoreIndexDyn;

use crate::config::{Config, LlmProvider};
use crate::domain::chat::{ChatMessage, ChatRole};

/// A single tool interaction: the tool name, arguments, and (truncated) result.
#[derive(Clone, Debug)]
pub struct ToolLogEntry {
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
}

/// Result of `chat_with_tools`: the final text + a log of tool calls that happened.
#[derive(Clone, Debug)]
pub struct ToolChatResult {
    pub text: String,
    pub tool_log: Vec<ToolLogEntry>,
}

impl ToolChatResult {
    /// Format tool interactions as a human-readable summary for persistence.
    /// Returns None if no tools were called.
    pub fn tool_log_summary(&self) -> Option<String> {
        if self.tool_log.is_empty() {
            return None;
        }
        let mut out = String::from("[tool activity]\n");
        for entry in &self.tool_log {
            out.push_str(&format!("• {} {}", entry.tool_name, entry.arguments));
            if !entry.result.is_empty() {
                // Truncate long results
                let truncated: String = entry.result.chars().take(200).collect();
                out.push_str(&format!(" → {truncated}"));
                if entry.result.len() > 200 {
                    out.push_str("...");
                }
            }
            out.push('\n');
        }
        Some(out)
    }
}

/// Extract tool call/result pairs from new messages Rig added to chat_history.
fn extract_tool_log(new_messages: &[Message]) -> Vec<ToolLogEntry> {
    let mut pending_calls: Vec<ToolLogEntry> = Vec::new();
    let mut log: Vec<ToolLogEntry> = Vec::new();

    for msg in new_messages {
        match msg {
            Message::Assistant { content, .. } => {
                for c in content.iter() {
                    if let AssistantContent::ToolCall(tc) = c {
                        let args = serde_json::to_string(&tc.function.arguments)
                            .unwrap_or_default();
                        pending_calls.push(ToolLogEntry {
                            tool_name: tc.function.name.clone(),
                            arguments: args,
                            result: String::new(),
                        });
                    }
                }
            }
            Message::User { content } => {
                for c in content.iter() {
                    if let UserContent::ToolResult(tr) = c {
                        // Match result to the earliest pending call
                        if let Some(entry) = pending_calls.first_mut() {
                            let result_text: String = tr.content.iter().filter_map(|rc| {
                                if let rig::completion::message::ToolResultContent::Text(t) = rc {
                                    Some(t.text.clone())
                                } else {
                                    None
                                }
                            }).collect::<Vec<_>>().join(" ");
                            entry.result = result_text;
                            log.push(pending_calls.remove(0));
                        }
                    }
                }
            }
        }
    }
    // Any unmatched calls (shouldn't happen normally)
    log.extend(pending_calls);
    log
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
            return Ok(ToolChatResult { text, tool_log: Vec::new() });
        }

        let history_len = history.len();

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
                    .max_turns(64)
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
                    .max_turns(64)
                    .await;
                (res, chat_history)
            }
        };

        // Extract tool interactions from the new messages Rig added to chat_history
        let tool_log = extract_tool_log(&chat_history[history_len..]);

        match result {
            Ok(response) => Ok(ToolChatResult { text: response, tool_log }),
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
                        Ok(response) => return Ok(ToolChatResult { text: response, tool_log }),
                        Err(e) if is_rate_limit_error(&e.to_string()) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Err("rate limited — try again in a moment".into())
            }
            Err(e) => {
                // If max turns exceeded, extract the last assistant text from chat history
                if let Some(text) = extract_last_assistant_text_from_error(&e) {
                    log::warn!("Max turns reached, returning last assistant text");
                    return Ok(ToolChatResult { text, tool_log });
                }
                log::error!("Tool agent failed: {e:?}");
                Err(e.into())
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

        let result = match self {
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
                res
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
                res
            }
        };

        match result {
            Ok(response) => Ok(response),
            Err(e) => {
                if let Some(text) = extract_last_assistant_text_from_error(&e) {
                    return Ok(text);
                }
                Err(e.into())
            }
        }
    }
}

/// Try to extract the last assistant text from a MaxTurnsError's chat history.
fn extract_last_assistant_text_from_error(
    error: &(dyn std::error::Error + Send + Sync),
) -> Option<String> {
    let msg = error.to_string();
    if !msg.contains("MaxTurnError") && !msg.contains("max turn limit") {
        return None;
    }

    // The error is a PromptError::MaxTurnsError which contains chat_history.
    // We can't downcast easily due to generics, but we can try to get the
    // PromptError source chain. Since we can't access the typed fields,
    // we'll return a graceful fallback message.
    log::warn!("Agent hit max turn limit — tool call loop was too long");
    Some("i ran out of steps trying to do that — try breaking it into smaller tasks".to_string())
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
