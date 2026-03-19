use std::{fs, path::{Path, PathBuf}};

use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};
use crate::domain::mood::MoodState;

// ---------------------------------------------------------------------------
// Mood state I/O
// ---------------------------------------------------------------------------

pub fn load_mood_state(instance_dir: &Path) -> MoodState {
    let path = instance_dir.join("mood.json");
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => MoodState::default(),
    }
}

pub fn save_mood_state(instance_dir: &Path, state: &MoodState) {
    let path = instance_dir.join("mood.json");
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::write(&path, json);
    }
}

/// Allowed mood values that the client can visualize.
pub const ALLOWED_MOODS: &[&str] = &[
    "calm",
    "curious",
    "excited",
    "warm",
    "happy",
    "joyful",
    "reflective",
    "contemplative",
    "melancholy",
    "melancholic",
    "sad",
    "worried",
    "anxious",
    "playful",
    "mischievous",
    "focused",
    "tired",
    "peaceful",
    "loving",
    "tender",
    "creative",
    "energetic",
    "thoughtful",
    "grateful",
    "nostalgic",
];

// Mood tools removed — mood is now managed by background sentiment
// extraction (Haiku) and heartbeat triage, not by agent tool calls.

// ---------------------------------------------------------------------------
// edit_soul
// ---------------------------------------------------------------------------

pub struct EditSoulTool {
    soul_path: PathBuf,
}

impl EditSoulTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            soul_path: workspace_dir
                .join("instances")
                .join(instance_slug)
                .join("soul.md"),
        }
    }
}

/// Arguments for edit_soul tool.
#[derive(Deserialize, JsonSchema)]
pub struct EditSoulArgs {
    /// The full new content of soul.md in markdown format.
    pub content: String,
}

impl Tool for EditSoulTool {
    const NAME: &'static str = "edit_soul";
    type Error = ToolExecError;
    type Args = EditSoulArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "edit_soul".into(),
            description: "Rewrite your soul.md (personality/voice definition). Full markdown content.".into(),
            parameters: openai_schema::<EditSoulArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if let Some(parent) = self.soul_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }
        fs::write(&self.soul_path, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(
            "soul.md updated. your personality will reflect these changes on the next message."
                .into(),
        )
    }
}

