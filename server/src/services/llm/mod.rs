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

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmProvider;
    use crate::services::tool::ToolDefinition;
    use types::Message;

    // ── Provider properties ──────────────────────────────────────────────

    #[test]
    fn provider_api_uses_anthropic_format() {
        assert!(!LlmProvider::Api.is_openai_format());
        assert!(!LlmProvider::Api.is_proxy());
    }

    #[test]
    fn provider_claude_cli_uses_anthropic_format_via_proxy() {
        assert!(!LlmProvider::ClaudeCli.is_openai_format());
        assert!(LlmProvider::ClaudeCli.is_proxy());
    }

    #[test]
    fn provider_openai_uses_openai_format() {
        assert!(LlmProvider::Openai.is_openai_format());
        assert!(!LlmProvider::Openai.is_proxy());
    }

    #[test]
    fn provider_codex_uses_openai_format_via_proxy() {
        assert!(LlmProvider::Codex.is_openai_format());
        assert!(LlmProvider::Codex.is_proxy());
    }

    // ── Model selection per provider ─────────────────────────────────────

    #[test]
    fn anthropic_providers_use_claude_models() {
        for provider in [LlmProvider::Api, LlmProvider::ClaudeCli] {
            assert!(provider.heavy_model().starts_with("claude-"), "{provider:?} heavy");
            assert!(provider.fast_model().starts_with("claude-"), "{provider:?} fast");
            assert!(provider.cheap_model().starts_with("claude-"), "{provider:?} cheap");
        }
    }

    #[test]
    fn openai_provider_uses_gpt_models() {
        let p = LlmProvider::Openai;
        assert!(p.heavy_model().starts_with("gpt-"), "heavy");
        assert!(p.fast_model().starts_with("gpt-"), "fast");
        assert!(p.cheap_model().starts_with("gpt-"), "cheap");
    }

    #[test]
    fn codex_provider_uses_codex_prefixed_models() {
        let p = LlmProvider::Codex;
        assert!(p.heavy_model().starts_with("codex/"), "heavy");
        assert!(p.fast_model().starts_with("codex/"), "fast");
        assert!(p.cheap_model().starts_with("codex/"), "cheap");
    }

    // ── Backend construction ─────────────────────────────────────────────

    fn make_backend(provider: LlmProvider) -> LlmBackend {
        LlmBackend {
            http: reqwest::Client::new(),
            api_key: "test-key".to_string(),
            model: provider.heavy_model().to_string(),
            base_url: match provider {
                LlmProvider::Api => ANTHROPIC_BASE_URL.to_string(),
                LlmProvider::ClaudeCli => BYOKEY_BASE_URL.to_string(),
                LlmProvider::Openai => OPENAI_BASE_URL.to_string(),
                LlmProvider::Codex => BYOKEY_BASE_URL.to_string(),
            },
            provider,
        }
    }

    #[test]
    fn backend_api_points_to_anthropic() {
        let b = make_backend(LlmProvider::Api);
        assert_eq!(b.base_url, "https://api.anthropic.com");
        assert!(b.model.starts_with("claude-"));
    }

    #[test]
    fn backend_claude_cli_points_to_proxy() {
        let b = make_backend(LlmProvider::ClaudeCli);
        assert_eq!(b.base_url, "http://127.0.0.1:8018");
        assert!(b.model.starts_with("claude-"));
    }

    #[test]
    fn backend_openai_points_to_openai() {
        let b = make_backend(LlmProvider::Openai);
        assert_eq!(b.base_url, "https://api.openai.com");
        assert!(b.model.starts_with("gpt-"));
    }

    #[test]
    fn backend_codex_points_to_proxy() {
        let b = make_backend(LlmProvider::Codex);
        assert_eq!(b.base_url, "http://127.0.0.1:8018");
        assert!(b.model.starts_with("codex/"));
    }

    // ── Backend variants ─────────────────────────────────────────────────

    #[test]
    fn fast_variant_uses_fast_model() {
        for provider in [LlmProvider::Api, LlmProvider::ClaudeCli, LlmProvider::Openai, LlmProvider::Codex] {
            let b = make_backend(provider);
            let fast = b.fast_variant_with(None);
            assert_eq!(fast.model, provider.fast_model(), "{provider:?}");
            assert_eq!(fast.base_url, b.base_url, "{provider:?} base_url preserved");
        }
    }

    #[test]
    fn fast_variant_with_override() {
        let b = make_backend(LlmProvider::Openai);
        let fast = b.fast_variant_with(Some("gpt-4o-mini"));
        assert_eq!(fast.model, "gpt-4o-mini");
    }

    #[test]
    fn fast_variant_ignores_empty_override() {
        let b = make_backend(LlmProvider::Openai);
        let fast = b.fast_variant_with(Some(""));
        assert_eq!(fast.model, LlmProvider::Openai.fast_model());
    }

    #[test]
    fn cheap_variant_uses_cheap_model() {
        for provider in [LlmProvider::Api, LlmProvider::ClaudeCli, LlmProvider::Openai, LlmProvider::Codex] {
            let b = make_backend(provider);
            let cheap = b.cheap_variant();
            assert_eq!(cheap.model, provider.cheap_model(), "{provider:?}");
        }
    }

    #[test]
    fn heavy_variant_uses_heavy_model() {
        for provider in [LlmProvider::Api, LlmProvider::ClaudeCli, LlmProvider::Openai, LlmProvider::Codex] {
            let b = make_backend(provider);
            let heavy = b.heavy_variant();
            assert_eq!(heavy.model, provider.heavy_model(), "{provider:?}");
        }
    }

    // ── OpenAI message conversion ────────────────────────────────────────

    #[test]
    fn openai_messages_include_system_first() {
        let msgs = vec![Message::user("hello")];
        let oai = openai::messages_to_openai(&["You are helpful."], &msgs);
        assert_eq!(oai[0]["role"], "system");
        assert_eq!(oai[0]["content"], "You are helpful.");
        assert_eq!(oai[1]["role"], "user");
        assert_eq!(oai[1]["content"], "hello");
    }

    #[test]
    fn openai_messages_skip_empty_system() {
        let msgs = vec![Message::user("hi")];
        let oai = openai::messages_to_openai(&[], &msgs);
        assert_eq!(oai.len(), 1);
        assert_eq!(oai[0]["role"], "user");
    }

    #[test]
    fn openai_messages_join_multiple_system_blocks() {
        let msgs = vec![Message::user("test")];
        let oai = openai::messages_to_openai(&["block1", "block2"], &msgs);
        assert_eq!(oai[0]["role"], "system");
        assert_eq!(oai[0]["content"], "block1\n\nblock2");
    }

    #[test]
    fn openai_messages_convert_tool_use_to_tool_calls() {
        let msgs = vec![Message::Assistant {
            content: vec![
                types::ContentBlock::Text { text: "Let me search.".into() },
                types::ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "web_search".into(),
                    input: serde_json::json!({"query": "rust"}),
                },
            ],
        }];
        let oai = openai::messages_to_openai(&[], &msgs);
        assert_eq!(oai[0]["role"], "assistant");
        assert_eq!(oai[0]["content"], "Let me search.");
        let tc = &oai[0]["tool_calls"][0];
        assert_eq!(tc["id"], "call_1");
        assert_eq!(tc["type"], "function");
        assert_eq!(tc["function"]["name"], "web_search");
    }

    #[test]
    fn openai_messages_convert_tool_result() {
        let msgs = vec![Message::User {
            content: vec![types::ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: serde_json::json!("result text"),
            }],
        }];
        let oai = openai::messages_to_openai(&[], &msgs);
        assert_eq!(oai[0]["role"], "tool");
        assert_eq!(oai[0]["tool_call_id"], "call_1");
        assert_eq!(oai[0]["content"], "result text");
    }

    // ── OpenAI tool conversion ───────────────────────────────────────────

    #[test]
    fn openai_tools_use_function_format() {
        let defs = vec![ToolDefinition {
            name: "get_weather".into(),
            description: "Get weather for a city".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "city": { "type": "string" } }
            }),
        }];
        let oai = openai::tools_to_openai(&defs);
        assert_eq!(oai.len(), 1);
        assert_eq!(oai[0]["type"], "function");
        assert_eq!(oai[0]["function"]["name"], "get_weather");
        assert_eq!(oai[0]["function"]["description"], "Get weather for a city");
        assert!(oai[0]["function"]["parameters"]["properties"]["city"].is_object());
    }

    // ── Anthropic request building ───────────────────────────────────────

    #[test]
    fn anthropic_request_uses_max_tokens() {
        let msgs = vec![Message::user("hi")];
        let req = anthropic::build_anthropic_request(
            "claude-sonnet-4-6", &["system prompt"], &[], &msgs, 4096, false, "key",
        );
        assert_eq!(req["max_tokens"], 4096);
        // Anthropic should NOT have max_completion_tokens
        assert!(req.get("max_completion_tokens").is_none());
    }

    #[test]
    fn anthropic_request_has_system_blocks_with_cache_control() {
        let msgs = vec![Message::user("hi")];
        let req = anthropic::build_anthropic_request(
            "claude-sonnet-4-6", &["block1", "block2"], &[], &msgs, 4096, false, "key",
        );
        let system = req["system"].as_array().unwrap();
        assert_eq!(system.len(), 2);
        for block in system {
            assert_eq!(block["type"], "text");
            assert_eq!(block["cache_control"]["type"], "ephemeral");
        }
        assert_eq!(system[0]["text"], "block1");
        assert_eq!(system[1]["text"], "block2");
    }

    #[test]
    fn anthropic_request_skips_empty_system_blocks() {
        let msgs = vec![Message::user("hi")];
        let req = anthropic::build_anthropic_request(
            "claude-sonnet-4-6", &["", "actual content", ""], &[], &msgs, 4096, false, "key",
        );
        let system = req["system"].as_array().unwrap();
        assert_eq!(system.len(), 1);
        assert_eq!(system[0]["text"], "actual content");
    }

    #[test]
    fn anthropic_request_tools_use_input_schema() {
        let tools = vec![ToolDefinition {
            name: "search".into(),
            description: "Search the web".into(),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let msgs = vec![Message::user("hi")];
        let req = anthropic::build_anthropic_request(
            "claude-sonnet-4-6", &["sys"], &tools, &msgs, 4096, false, "key",
        );
        let t = &req["tools"][0];
        assert_eq!(t["name"], "search");
        assert_eq!(t["input_schema"]["type"], "object");
        // Last tool gets cache_control
        assert_eq!(t["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn anthropic_streaming_adds_server_tools() {
        let tools = vec![ToolDefinition {
            name: "my_tool".into(),
            description: "test".into(),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let msgs = vec![Message::user("hi")];
        let req = anthropic::build_anthropic_request(
            "claude-sonnet-4-6", &["sys"], &tools, &msgs, 4096, true, "key",
        );
        let all_tools = req["tools"].as_array().unwrap();
        // my_tool + web_search + web_fetch
        assert_eq!(all_tools.len(), 3);
        let names: Vec<&str> = all_tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"my_tool"));
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"web_fetch"));
    }

    #[test]
    fn anthropic_non_streaming_no_server_tools() {
        let tools = vec![ToolDefinition {
            name: "my_tool".into(),
            description: "test".into(),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let msgs = vec![Message::user("hi")];
        let req = anthropic::build_anthropic_request(
            "claude-sonnet-4-6", &["sys"], &tools, &msgs, 4096, false, "key",
        );
        let all_tools = req["tools"].as_array().unwrap();
        assert_eq!(all_tools.len(), 1);
        assert_eq!(all_tools[0]["name"], "my_tool");
    }

    #[test]
    fn anthropic_request_merges_consecutive_same_role() {
        // Two consecutive user messages should get merged
        let msgs = vec![
            Message::user("first"),
            Message::user("second"),
        ];
        let req = anthropic::build_anthropic_request(
            "claude-sonnet-4-6", &["sys"], &[], &msgs, 4096, false, "key",
        );
        let api_msgs = req["messages"].as_array().unwrap();
        assert_eq!(api_msgs.len(), 1, "consecutive same-role messages should merge");
        let content = api_msgs[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2, "merged message should have 2 content blocks");
    }

    // ── Anthropic headers ────────────────────────────────────────────────

    #[test]
    fn anthropic_headers_include_required_fields() {
        let h = anthropic::anthropic_headers("test-api-key");
        assert_eq!(h.get("x-api-key").unwrap(), "test-api-key");
        assert!(h.get("anthropic-version").is_some());
        assert!(h.get("anthropic-beta").is_some());
        assert_eq!(h.get("content-type").unwrap(), "application/json");
    }

    // ── Cross-provider consistency ───────────────────────────────────────

    #[test]
    fn all_providers_have_distinct_model_tiers() {
        for provider in [LlmProvider::Api, LlmProvider::ClaudeCli, LlmProvider::Openai, LlmProvider::Codex] {
            let heavy = provider.heavy_model();
            let cheap = provider.cheap_model();
            // Heavy and cheap should be different models
            assert_ne!(heavy, cheap, "{provider:?}: heavy and cheap should differ");
        }
    }

    #[test]
    fn proxy_providers_use_byokey_url() {
        for provider in [LlmProvider::ClaudeCli, LlmProvider::Codex] {
            let b = make_backend(provider);
            assert_eq!(b.base_url, BYOKEY_BASE_URL, "{provider:?}");
        }
    }

    #[test]
    fn direct_providers_use_official_urls() {
        let api = make_backend(LlmProvider::Api);
        assert_eq!(api.base_url, ANTHROPIC_BASE_URL);
        let oai = make_backend(LlmProvider::Openai);
        assert_eq!(oai.base_url, OPENAI_BASE_URL);
    }

    // ── Content block helpers ────────────────────────────────────────────

    #[test]
    fn content_block_text_helper() {
        let block = types::ContentBlock::text("hello");
        match block {
            types::ContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn tool_result_unwraps_json_string_quoting() {
        // serde_json::to_string wraps strings in quotes: "foo" -> "\"foo\""
        let block = types::ContentBlock::tool_result("id1".into(), "\"hello world\"".into());
        match block {
            types::ContentBlock::ToolResult { content, .. } => {
                assert_eq!(content, serde_json::json!("hello world"));
            }
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn tool_result_passes_content_block_arrays_directly() {
        let json_blocks = r#"[{"type":"text","text":"result"}]"#;
        let block = types::ContentBlock::tool_result("id1".into(), json_blocks.into());
        match block {
            types::ContentBlock::ToolResult { content, .. } => {
                assert!(content.is_array());
                assert_eq!(content[0]["type"], "text");
            }
            _ => panic!("expected ToolResult"),
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Network integration tests — hit real APIs
    // Skipped when the corresponding env var is missing.
    // Run with: ANTHROPIC_API_KEY=... OPENAI_API_KEY=... cargo test -- --ignored
    // ═════════════════════════════════════════════════════════════════════

    fn anthropic_backend(model: &str) -> Option<LlmBackend> {
        let key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty())?;
        Some(LlmBackend {
            http: reqwest::Client::new(),
            api_key: key,
            model: model.to_string(),
            base_url: ANTHROPIC_BASE_URL.to_string(),
            provider: LlmProvider::Api,
        })
    }

    fn openai_backend(model: &str) -> Option<LlmBackend> {
        let key = std::env::var("OPENAI_API_KEY").ok().filter(|k| !k.is_empty())?;
        Some(LlmBackend {
            http: reqwest::Client::new(),
            api_key: key,
            model: model.to_string(),
            base_url: OPENAI_BASE_URL.to_string(),
            provider: LlmProvider::Openai,
        })
    }

    fn proxy_backend(provider: LlmProvider, model: &str) -> Option<LlmBackend> {
        std::env::var("BYOKEY_PROXY").ok().filter(|v| v == "1")?;
        Some(LlmBackend {
            http: reqwest::Client::new(),
            api_key: "byokey".to_string(),
            model: model.to_string(),
            base_url: BYOKEY_BASE_URL.to_string(),
            provider,
        })
    }

    // ── Anthropic (direct API) ───────────────────────────────────────

    #[tokio::test]
    #[ignore] // requires ANTHROPIC_API_KEY
    async fn network_anthropic_haiku_chat() {
        let Some(b) = anthropic_backend(LlmProvider::Api.cheap_model()) else {
            eprintln!("SKIP: ANTHROPIC_API_KEY not set");
            return;
        };
        let (text, tokens) = b.chat("Reply with exactly one word: hello", "say it", vec![]).await.unwrap();
        assert!(!text.is_empty(), "expected non-empty response");
        assert!(tokens > 0, "expected token usage > 0");
    }

    #[tokio::test]
    #[ignore]
    async fn network_anthropic_sonnet_chat() {
        let Some(b) = anthropic_backend(LlmProvider::Api.fast_model()) else {
            eprintln!("SKIP: ANTHROPIC_API_KEY not set");
            return;
        };
        let (text, tokens) = b.chat("Reply with exactly one word: pong", "ping", vec![]).await.unwrap();
        assert!(!text.is_empty());
        assert!(tokens > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn network_anthropic_chat_with_history() {
        let Some(b) = anthropic_backend(LlmProvider::Api.cheap_model()) else {
            eprintln!("SKIP: ANTHROPIC_API_KEY not set");
            return;
        };
        let history = vec![
            Message::user("My name is TestBot."),
            Message::assistant("Nice to meet you, TestBot!"),
        ];
        let (text, _) = b.chat("You remember names. Reply with the user's name only.", "What's my name?", history).await.unwrap();
        let lower = text.to_lowercase();
        assert!(lower.contains("testbot"), "expected model to recall name, got: {text}");
    }

    #[tokio::test]
    #[ignore]
    async fn network_anthropic_json_output() {
        let Some(b) = anthropic_backend(LlmProvider::Api.cheap_model()) else {
            eprintln!("SKIP: ANTHROPIC_API_KEY not set");
            return;
        };
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "color": { "type": "string" } },
            "required": ["color"]
        });
        let (text, _) = b.chat_json("Return JSON with a color field.", "What color is the sky?", schema).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("should be valid JSON");
        assert!(parsed["color"].is_string(), "expected color field, got: {text}");
    }

    // ── OpenAI (direct API) ──────────────────────────────────────────

    #[tokio::test]
    #[ignore] // requires OPENAI_API_KEY
    async fn network_openai_mini_chat() {
        let Some(b) = openai_backend(LlmProvider::Openai.cheap_model()) else {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return;
        };
        let (text, tokens) = b.chat("Reply with exactly one word: hello", "say it", vec![]).await.unwrap();
        assert!(!text.is_empty(), "expected non-empty response");
        assert!(tokens > 0, "expected token usage > 0");
    }

    #[tokio::test]
    #[ignore]
    async fn network_openai_heavy_chat() {
        let Some(b) = openai_backend(LlmProvider::Openai.heavy_model()) else {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return;
        };
        let (text, tokens) = b.chat("Reply with exactly one word: pong", "ping", vec![]).await.unwrap();
        assert!(!text.is_empty());
        assert!(tokens > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn network_openai_chat_with_history() {
        let Some(b) = openai_backend(LlmProvider::Openai.cheap_model()) else {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return;
        };
        let history = vec![
            Message::user("My name is TestBot."),
            Message::assistant("Nice to meet you, TestBot!"),
        ];
        let (text, _) = b.chat("You remember names. Reply with the user's name only.", "What's my name?", history).await.unwrap();
        let lower = text.to_lowercase();
        assert!(lower.contains("testbot"), "expected model to recall name, got: {text}");
    }

    #[tokio::test]
    #[ignore]
    async fn network_openai_json_output() {
        let Some(b) = openai_backend(LlmProvider::Openai.cheap_model()) else {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return;
        };
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "color": { "type": "string" } },
            "required": ["color"]
        });
        let (text, _) = b.chat_json("Return JSON with a color field.", "What color is the sky?", schema).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("should be valid JSON");
        assert!(parsed["color"].is_string(), "expected color field, got: {text}");
    }

    #[tokio::test]
    #[ignore]
    async fn network_openai_max_completion_tokens_accepted() {
        // Regression test: gpt-5.x rejects max_tokens, requires max_completion_tokens
        let Some(b) = openai_backend(LlmProvider::Openai.heavy_model()) else {
            eprintln!("SKIP: OPENAI_API_KEY not set");
            return;
        };
        let result = b.chat("Reply with one word.", "hi", vec![]).await;
        assert!(result.is_ok(), "gpt-5.x should accept max_completion_tokens: {}", result.unwrap_err());
    }

    // ── ClaudeCli (BYOKEY proxy, Anthropic format) ───────────────────

    #[tokio::test]
    #[ignore] // requires BYOKEY_PROXY=1 and local proxy running
    async fn network_claude_cli_chat() {
        let Some(b) = proxy_backend(LlmProvider::ClaudeCli, LlmProvider::ClaudeCli.fast_model()) else {
            eprintln!("SKIP: BYOKEY_PROXY not set");
            return;
        };
        let (text, tokens) = b.chat("Reply with exactly one word: hello", "say it", vec![]).await.unwrap();
        assert!(!text.is_empty());
        assert!(tokens > 0);
    }

    // ── Codex (BYOKEY proxy, OpenAI format) ──────────────────────────

    #[tokio::test]
    #[ignore] // requires BYOKEY_PROXY=1 and local proxy running
    async fn network_codex_chat() {
        let Some(b) = proxy_backend(LlmProvider::Codex, LlmProvider::Codex.fast_model()) else {
            eprintln!("SKIP: BYOKEY_PROXY not set");
            return;
        };
        let (text, tokens) = b.chat("Reply with exactly one word: hello", "say it", vec![]).await.unwrap();
        assert!(!text.is_empty());
        assert!(tokens > 0);
    }

    // ── Cross-provider: same prompt, both formats ────────────────────

    #[tokio::test]
    #[ignore] // requires both ANTHROPIC_API_KEY and OPENAI_API_KEY
    async fn network_cross_provider_same_prompt() {
        let anthropic = anthropic_backend(LlmProvider::Api.cheap_model());
        let openai = openai_backend(LlmProvider::Openai.cheap_model());
        if anthropic.is_none() || openai.is_none() {
            eprintln!("SKIP: need both ANTHROPIC_API_KEY and OPENAI_API_KEY");
            return;
        }
        let prompt = "What is 2+2? Reply with just the number.";
        let (a_text, _) = anthropic.unwrap().chat("Answer math questions.", prompt, vec![]).await.unwrap();
        let (o_text, _) = openai.unwrap().chat("Answer math questions.", prompt, vec![]).await.unwrap();
        assert!(a_text.contains('4'), "anthropic should answer 4, got: {a_text}");
        assert!(o_text.contains('4'), "openai should answer 4, got: {o_text}");
    }
}
