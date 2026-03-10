use serde::{Deserialize, Serialize};

/// Aggregated interaction rhythm patterns for a user.
/// Computed from message history and persisted to rhythm.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InteractionRhythm {
    /// Message count per hour of day (0-23), user messages only.
    #[serde(default)]
    pub hourly_activity: [u32; 24],

    /// Message count per day of week (0=Mon, 6=Sun), user messages only.
    #[serde(default)]
    pub daily_activity: [u32; 7],

    /// Average user message length in characters.
    #[serde(default)]
    pub avg_message_length: f64,

    /// Average seconds between consecutive user messages within a session.
    /// A session gap is defined as >2 hours of silence.
    #[serde(default)]
    pub avg_response_interval_secs: f64,

    /// Total user messages analyzed.
    #[serde(default)]
    pub total_messages: u32,

    /// Unix timestamp of last rhythm update.
    #[serde(default)]
    pub updated_at: i64,
}
