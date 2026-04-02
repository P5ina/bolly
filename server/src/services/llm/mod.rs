mod agent_loop;
mod anthropic;
mod helpers;
mod openai;
mod types;

use std::path::Path;

use tokio::sync::broadcast;

use crate::config::Config;
use crate::domain::events::ServerEvent;
use crate::services::tool::ToolDyn;

// Re-export all public types and functions that were accessible from crate::services::llm::*
pub use helpers::{
    build_multimodal_prompt, get_real_input_tokens, history_to_chat_messages, load_system_prompt,
};
#[allow(unused_imports)]
pub use helpers::DEFAULT_ONBOARDING_PROMPT;
pub use types::{ContentBlock, HistoryEntry, LlmBackend, Message, ToolChatResult};
#[allow(unused_imports)]
pub use types::{DocumentSource, ImageSource};

use agent_loop::{agent_loop, collect_tool_defs, streaming_agent_loop};
use anthropic::{anthropic_complete, anthropic_headers};
use helpers::retry_on_rate_limit;
use openai::openai_complete;
use types::{ANTHROPIC_BASE_URL, BYOKEY_BASE_URL, OPENAI_BASE_URL};

impl LlmBackend {
    pub fn from_config(config: &Config) -> Option<Self> {
        let http = reqwest::Client::new();
        let model = config.llm.model_name().to_string();

        match config.llm.provider {
            crate::config::LlmProvider::Api => {
                let api_key = config.llm.api_key()?.to_string();
                Some(Self {
                    http,
                    api_key,
                    model,
                    base_url: ANTHROPIC_BASE_URL.to_string(),
                    provider: crate::config::LlmProvider::Api,
                })
            }
            crate::config::LlmProvider::ClaudeCli => {
                // BYOKEY proxy: Anthropic API format on localhost
                Some(Self {
                    http,
                    api_key: "byokey".to_string(),
                    model,
                    base_url: BYOKEY_BASE_URL.to_string(),
                    provider: crate::config::LlmProvider::ClaudeCli,
                })
            }
            crate::config::LlmProvider::Openai => {
                let api_key = if config.llm.tokens.open_ai.is_empty() {
                    return None;
                } else {
                    config.llm.tokens.open_ai.clone()
                };
                Some(Self {
                    http,
                    api_key,
                    model,
                    base_url: OPENAI_BASE_URL.to_string(),
                    provider: crate::config::LlmProvider::Openai,
                })
            }
            crate::config::LlmProvider::Codex => {
                // BYOKEY proxy: OpenAI format on localhost
                Some(Self {
                    http,
                    api_key: "byokey".to_string(),
                    model,
                    base_url: BYOKEY_BASE_URL.to_string(),
                    provider: crate::config::LlmProvider::Codex,
                })
            }
        }
    }

    /// Whether this backend uses the Meridian proxy (Claude subscription).
    #[allow(dead_code)]
    pub fn is_cli(&self) -> bool {
        matches!(self.provider, crate::config::LlmProvider::ClaudeCli)
    }

    /// Create a variant using the fast model.
    pub fn fast_variant_with(&self, override_model: Option<&str>) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: override_model
                .filter(|s| !s.is_empty())
                .unwrap_or(self.provider.fast_model())
                .to_string(),
            base_url: self.base_url.clone(),
            provider: self.provider,
        }
    }

    /// Create a variant using the cheapest model for background tasks.
    pub fn cheap_variant(&self) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: self.provider.cheap_model().to_string(),
            base_url: self.base_url.clone(),
            provider: self.provider,
        }
    }

    /// Create a variant using the heavy model for deep reflection.
    pub fn heavy_variant(&self) -> Self {
        Self {
            http: self.http.clone(),
            api_key: self.api_key.clone(),
            model: self.provider.heavy_model().to_string(),
            base_url: self.base_url.clone(),
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
                log::info!(
                    "model router: classified as {} for: {}",
                    if heavy { "heavy" } else { "fast" },
                    &user_message.chars().take(80).collect::<String>()
                );
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
                if backend.provider.is_openai_format() {
                    openai_complete(
                        &backend.http,
                        &backend.api_key,
                        &backend.model,
                        &[&system],
                        &[],
                        &messages,
                        16384,
                        &backend.base_url,
                    )
                    .await
                    .map(|(text, _, _, tokens)| (text, tokens))
                } else {
                    anthropic_complete(
                        &backend.http,
                        &backend.api_key,
                        &backend.model,
                        &[&system],
                        &[],
                        &messages,
                        16384,
                        &backend.base_url,
                    )
                    .await
                    .map(|(text, _, _, tokens)| (text, tokens))
                }
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
        if self.provider.is_openai_format() {
            let backend = self.clone();
            let system = system_prompt.to_string();
            let prompt = prompt.to_string();
            return retry_on_rate_limit(|| {
                let backend = backend.clone();
                let system = system.clone();
                let prompt = prompt.clone();
                let schema = schema.clone();
                async move {
                    // Include schema in prompt since Codex subscription may not
                    // support response_format.json_schema. Use json_object mode
                    // which is widely supported.
                    let schema_str = serde_json::to_string(&schema).unwrap_or_default();
                    let messages = vec![serde_json::json!(
                        {"role": "system", "content": format!("{system}\n\nRespond with ONLY valid JSON matching this schema:\n{schema_str}")}
                    ), serde_json::json!(
                        {"role": "user", "content": &prompt}
                    )];
                    let req = serde_json::json!({
                        "model": &backend.model,
                        "max_completion_tokens": 16384,
                        "stream": false,
                        "messages": messages,
                        "response_format": { "type": "json_object" },
                    });
                    let resp = backend.http
                        .post(&format!("{}/v1/chat/completions", backend.base_url))
                        .header("Authorization", format!("Bearer {}", backend.api_key))
                        .header("Content-Type", "application/json")
                        .json(&req)
                        .send()
                        .await?;
                    let status = resp.status();
                    let resp_text = resp.text().await?;
                    if !status.is_success() {
                        return Err(anyhow::anyhow!("OpenAI API error {status}: {resp_text}"));
                    }
                    let resp_json: serde_json::Value = serde_json::from_str(&resp_text)?;
                    let tokens = resp_json["usage"]["prompt_tokens"].as_u64().unwrap_or(0)
                        + resp_json["usage"]["completion_tokens"].as_u64().unwrap_or(0);
                    let text = resp_json["choices"][0]["message"]["content"]
                        .as_str().unwrap_or("").to_string();
                    Ok((text, tokens))
                }
            }).await;
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
                let system_blocks =
                    vec![serde_json::json!({"type": "text", "text": &system})];
                let msgs =
                    serde_json::to_value(&messages).unwrap_or(serde_json::json!([]));

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

                let resp = backend
                    .http
                    .post(&format!("{}/v1/messages", backend.base_url))
                    .headers(anthropic_headers(&backend.api_key))
                    .json(&req)
                    .send()
                    .await?;

                let status = resp.status();
                let resp_text = resp.text().await?;
                if !status.is_success() {
                    return Err(anyhow::anyhow!(
                        "Anthropic API error {status}: {resp_text}"
                    ));
                }

                let resp_json: serde_json::Value = serde_json::from_str(&resp_text)?;
                let tokens = resp_json
                    .pointer("/usage/input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    + resp_json
                        .pointer("/usage/output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                // Find first text block (may be preceded by thinking blocks)
                let text = resp_json
                    .pointer("/content")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| {
                        arr.iter()
                            .find(|b| b["type"].as_str() == Some("text"))
                    })
                    .and_then(|b| b["text"].as_str())
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

    /// Simplified tool call (no streaming). Used by heartbeat.
    #[allow(dead_code)]
    pub async fn chat_with_tools_only(
        &self,
        system_prompt: &str,
        prompt: &str,
        history: Vec<Message>,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> anyhow::Result<(String, u64)> {
        if tools.is_empty() {
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
        let (text, tokens) =
            agent_loop(self, system_blocks, &tool_defs, &tools, &mut messages).await?;
        Ok((text, tokens, messages))
    }
}
