use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub enabled: bool,
    /// System prompt fragment injected when this skill is active.
    #[serde(default)]
    pub instructions: String,
}
