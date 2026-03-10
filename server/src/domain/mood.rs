use serde::{Deserialize, Serialize};

/// Emotional state tracked per instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MoodState {
    /// Companion's current mood (e.g. "curious", "warm", "reflective", "excited").
    #[serde(default)]
    pub companion_mood: String,
    /// Last observed user sentiment (e.g. "frustrated", "excited", "tired", "neutral").
    #[serde(default)]
    pub user_sentiment: String,
    /// Short note about the emotional context of the last conversation.
    #[serde(default)]
    pub emotional_context: String,
    /// Unix timestamp of last update.
    #[serde(default)]
    pub updated_at: i64,
    /// Unix timestamp of last user message.
    #[serde(default)]
    pub last_interaction: i64,
    /// Unix timestamp of last autonomous reach-out (to prevent spam).
    #[serde(default)]
    pub last_reach_out: i64,
}
