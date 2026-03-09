use serde::Serialize;

use crate::domain::{chat::ChatMessage, instance::InstanceSummary};

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
}
