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

// ═══════════════════════════════════════════════════════════════════════════
// LlmBackend — direct API calls to Anthropic / OpenAI / OpenRouter
// ═══════════════════════════════════════════════════════════════════════════

pub(crate) const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
pub(crate) const BYOKEY_BASE_URL: &str = "http://127.0.0.1:8018";
pub(crate) const OPENAI_BASE_URL: &str = "https://api.openai.com";

#[derive(Clone)]
pub struct LlmBackend {
    pub http: reqwest::Client,
    pub api_key: String,
    pub model: String,
    /// Base URL for Anthropic API calls.
    pub base_url: String,
    /// Provider type — Api (direct Anthropic) or ClaudeCli (via Meridian proxy).
    pub provider: crate::config::LlmProvider,
}

pub(crate) struct ToolUseBlock {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) input: serde_json::Value,
}

/// Result of a single streaming turn.
pub(crate) struct StreamOnceResult {
    pub(crate) text: String,
    pub(crate) tool_uses: Vec<ToolUseBlock>,
    pub(crate) stop_reason: String,
    pub(crate) tokens_used: u64,
    /// Content blocks in the order they arrived from the API.
    /// Preserves interleaving of text, server_tool_use, and server_tool_result.
    pub(crate) ordered_content: Vec<ContentBlock>,
}
