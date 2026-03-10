use std::path::Path;

use rig::client::CompletionClient;
use rig::completion::{Chat, Message};
use rig::providers::{anthropic, openai};
use rig::tool::ToolDyn;

use crate::config::{Config, LlmProvider};
use crate::domain::chat::{ChatMessage, ChatRole};

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
        match self {
            LlmBackend::Anthropic { client, model } => {
                let agent = client.agent(model).preamble(system_prompt).build();
                Ok(agent.chat(prompt, history).await?)
            }
            LlmBackend::OpenAI { client, model } => {
                let agent = client.agent(model).preamble(system_prompt).build();
                Ok(agent.chat(prompt, history).await?)
            }
        }
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
            Err(e) => {
                log::error!("Tool agent failed: {e:?}");
                log::warn!("Retrying without tools");
                self.chat(system_prompt, prompt, history).await
            }
        }
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
