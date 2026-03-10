use std::path::Path;
use std::time::Duration;

use rig::client::CompletionClient;
use rig::completion::{Chat, Message, Prompt};
use rig::completion::message::{UserContent, ImageMediaType, DocumentMediaType, MimeType};
use rig::one_or_many::OneOrMany;
use rig::providers::{anthropic, openai};
use rig::tool::ToolDyn;

use crate::config::{Config, LlmProvider};
use crate::domain::chat::{ChatMessage, ChatRole};

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 2000;

fn is_rate_limit_error(msg: &str) -> bool {
    msg.contains("429") || msg.contains("rate_limit") || msg.contains("Too Many Requests")
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
                let client = anthropic::Client::new(token).ok()?;
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
                        let agent = client.agent(model).preamble(&system).build();
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

    pub async fn chat_with_tools(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        log::info!("chat_with_tools: {} tools registered", tools.len());
        for t in &tools {
            log::debug!("  tool: {}", t.name());
        }

        if tools.is_empty() {
            return self.chat(system_prompt, prompt, history).await;
        }

        let result = match self {
            LlmBackend::Anthropic { client, model } => {
                let agent = client
                    .agent(model)
                    .preamble(system_prompt)
                    .default_max_turns(8)
                    .tools(tools)
                    .build();
                agent.chat(prompt, history.clone()).await
            }
            LlmBackend::OpenAI { client, model } => {
                let agent = client
                    .agent(model)
                    .preamble(system_prompt)
                    .default_max_turns(8)
                    .tools(tools)
                    .build();
                agent.chat(prompt, history.clone()).await
            }
        };

        match result {
            Ok(response) => Ok(response),
            Err(e) if is_rate_limit_error(&e.to_string()) => {
                log::warn!("Rate limited during tool agent, retrying without tools after backoff");
                tokio::time::sleep(Duration::from_millis(INITIAL_BACKOFF_MS)).await;
                self.chat(system_prompt, prompt, history).await
            }
            Err(e) => {
                log::error!("Tool agent failed: {e:?}");
                log::warn!("Retrying without tools");
                self.chat(system_prompt, prompt, history).await
            }
        }
    }

    pub async fn chat_with_vision(
        &self,
        system_prompt: &str,
        prompt: &str,
        file_bytes: &[u8],
        mime_type: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Build multimodal user message with text + image/document
        let text_content = UserContent::text(prompt);

        let file_content = if mime_type.starts_with("image/") {
            let media = ImageMediaType::from_mime_type(mime_type);
            UserContent::image_raw(file_bytes.to_vec(), media, None)
        } else if mime_type == "application/pdf" {
            UserContent::document_raw(file_bytes.to_vec(), Some(DocumentMediaType::PDF))
        } else {
            // Fallback: treat as text if possible
            let text = String::from_utf8_lossy(file_bytes);
            return self.chat(system_prompt, &format!("{prompt}\n\nFile content:\n{text}"), vec![]).await;
        };

        let content = OneOrMany::many(vec![text_content, file_content])?;
        let message = Message::User { content };

        let backend = self.clone();
        let system = system_prompt.to_string();
        retry_on_rate_limit(|| {
            let backend = backend.clone();
            let system = system.clone();
            let message = message.clone();
            async move {
                match &backend {
                    LlmBackend::Anthropic { client, model } => {
                        let agent = client.agent(model).preamble(&system).build();
                        Ok(agent.prompt(message).await?)
                    }
                    LlmBackend::OpenAI { client, model } => {
                        let agent = client.agent(model).preamble(&system).build();
                        Ok(agent.prompt(message).await?)
                    }
                }
            }
        })
        .await
    }
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
