use serde::{Deserialize, Serialize};

/// A creative artifact the companion generates autonomously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drop {
    pub id: String,
    pub kind: DropKind,
    pub title: String,
    pub content: String,
    pub mood: String,
    pub created_at: String,
    /// Optional image URL (e.g. generated via fal.ai).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropKind {
    Thought,
    Idea,
    Poem,
    Observation,
    Reflection,
    Recommendation,
    Story,
    Question,
    Note,
}

impl DropKind {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "thought" => Self::Thought,
            "idea" => Self::Idea,
            "poem" => Self::Poem,
            "observation" => Self::Observation,
            "reflection" => Self::Reflection,
            "recommendation" => Self::Recommendation,
            "story" => Self::Story,
            "question" => Self::Question,
            _ => Self::Note,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Thought => "thought",
            Self::Idea => "idea",
            Self::Poem => "poem",
            Self::Observation => "observation",
            Self::Reflection => "reflection",
            Self::Recommendation => "recommendation",
            Self::Story => "story",
            Self::Question => "question",
            Self::Note => "note",
        }
    }
}
