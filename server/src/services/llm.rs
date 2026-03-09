use std::path::Path;

use rig::client::CompletionClient;
use rig::completion::{Chat, Message};
use rig::providers::{anthropic, openai};

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
                    .unwrap_or_else(|| "gpt-4o".to_string());
                Some(LlmBackend::OpenAI { client, model })
            }
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
}

pub fn load_system_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let soul_path = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("soul.md");
    std::fs::read_to_string(soul_path).unwrap_or_else(|_| DEFAULT_ONBOARDING_PROMPT.to_string())
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
