use serde::{Deserialize, Serialize};

/// Stats for a single day. Stored as `instances/{slug}/stats/{date}.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyStats {
    /// Date string (YYYY-MM-DD) in the instance's local timezone.
    pub date: String,
    /// Number of user messages this day.
    #[serde(default)]
    pub messages: u32,
    /// Total chars across all user messages this day.
    #[serde(default)]
    pub chars: u64,
    /// Messages per hour of day (0-23).
    #[serde(default)]
    pub hours: [u32; 24],
    /// Day of week (0=Mon, 6=Sun).
    #[serde(default)]
    pub weekday: u8,
}
