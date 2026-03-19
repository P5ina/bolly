use serde::Serialize;

use crate::domain::{chat::ChatMessage, drop::Drop, instance::InstanceSummary, thought::Thought};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    ChatMessageCreated {
        instance_slug: String,
        chat_id: String,
        message: ChatMessage,
    },
    InstanceDiscovered {
        instance: InstanceSummary,
    },
    MoodUpdated {
        instance_slug: String,
        mood: String,
    },
    AgentRunning {
        instance_slug: String,
        chat_id: String,
    },
    AgentStopped {
        instance_slug: String,
        chat_id: String,
    },
    DropCreated {
        instance_slug: String,
        drop: Drop,
    },
    HeartbeatThought {
        instance_slug: String,
        thought: Thought,
    },
    ContextCompacting {
        instance_slug: String,
        chat_id: String,
        messages_compacted: usize,
    },
    ChatStreamDelta {
        instance_slug: String,
        chat_id: String,
        message_id: String,
        delta: String,
    },
    SecretRequest {
        instance_slug: String,
        id: String,
        prompt: String,
        target: String,
    },
    ToolOutputChunk {
        instance_slug: String,
        chat_id: String,
        chunk: String,
    },
    /// Tool result arrived for an MCP App — viewer should send it to the iframe.
    McpAppResult {
        instance_slug: String,
        chat_id: String,
        /// The message id of the McpApp chat message to update.
        message_id: String,
        tool_output: String,
    },
    /// Streaming: an MCP App tool call is starting — show the iframe immediately.
    McpAppStart {
        instance_slug: String,
        chat_id: String,
        tool_name: String,
        html: String,
    },
    /// Streaming: partial tool arguments delta for an MCP App.
    McpAppInputDelta {
        instance_slug: String,
        chat_id: String,
        delta: String,
    },
}
