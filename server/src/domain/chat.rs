use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: ChatRole,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub instance_slug: String,
    pub content: String,
    #[serde(default = "default_chat_id")]
    pub chat_id: String,
}

fn default_chat_id() -> String {
    "default".into()
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub instance_slug: String,
    pub chat_id: String,
    pub messages: Vec<ChatMessage>,
}

/// Summary of a chat session for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub message_count: usize,
    pub last_message_at: Option<String>,
    pub created_at: String,
}

/// Metadata stored alongside each chat's messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
}
