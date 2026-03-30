use serde::{Deserialize, Serialize};

use crate::services::llm::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Scheduled,
    OnDemand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Completed,
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    pub agent_name: String,
    pub agent_kind: AgentKind,
    pub trigger: String,
    pub started_at: u64,
    pub finished_at: u64,
    pub duration_ms: u64,
    pub tokens_used: u64,
    pub model: String,
    pub summary: String,
    pub trace: Vec<Message>,
    pub status: RunStatus,
}

/// Lightweight version without trace for list responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunSummary {
    pub id: String,
    pub agent_name: String,
    pub agent_kind: AgentKind,
    pub trigger: String,
    pub started_at: u64,
    pub finished_at: u64,
    pub duration_ms: u64,
    pub tokens_used: u64,
    pub model: String,
    pub summary: String,
    pub status: RunStatus,
}

impl From<AgentRun> for AgentRunSummary {
    fn from(r: AgentRun) -> Self {
        Self {
            id: r.id,
            agent_name: r.agent_name,
            agent_kind: r.agent_kind,
            trigger: r.trigger,
            started_at: r.started_at,
            finished_at: r.finished_at,
            duration_ms: r.duration_ms,
            tokens_used: r.tokens_used,
            model: r.model,
            summary: r.summary,
            status: r.status,
        }
    }
}
