use serde::Serialize;

use crate::domain::{chat::ChatMessage, drop::Drop, instance::InstanceSummary};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    ChatMessageCreated {
        instance_slug: String,
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
    },
    AgentStopped {
        instance_slug: String,
    },
    ToolActivity {
        instance_slug: String,
        tool_name: String,
        summary: String,
    },
    DropCreated {
        instance_slug: String,
        drop: Drop,
    },
}
