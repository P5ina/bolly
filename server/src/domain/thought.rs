use serde::{Deserialize, Serialize};

/// A captured heartbeat thought — the companion's inner monologue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thought {
    pub id: String,
    pub raw: String,
    pub actions: Vec<String>,
    pub mood: String,
    pub created_at: String,
}
