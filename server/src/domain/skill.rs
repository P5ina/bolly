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
    /// Where this skill was installed from (None = user-created).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SkillSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSource {
    /// GitHub repo in "owner/repo" format.
    pub repo: String,
    /// Git ref that was installed (branch, tag, or commit SHA).
    pub version: String,
}

/// An entry in the remote skills registry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub icon: String,
    /// GitHub repo "owner/repo".
    pub repo: String,
    /// Branch or tag to fetch from.
    #[serde(default = "default_git_ref")]
    pub git_ref: String,
    /// Author display name.
    #[serde(default)]
    pub author: String,
}

fn default_git_ref() -> String {
    "main".into()
}
