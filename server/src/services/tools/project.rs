use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Task board
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub notes: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Todo => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Blocked => write!(f, "blocked"),
        }
    }
}
