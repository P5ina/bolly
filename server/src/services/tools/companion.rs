use std::{fs, path::{Path, PathBuf}};

use chrono::Utc;
use rig::{completion::ToolDefinition, tool::Tool};
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

// ---------------------------------------------------------------------------
// set_mood
// ---------------------------------------------------------------------------

pub struct SetMoodTool {
    instance_dir: PathBuf,
    instance_slug: String,
    events: tokio::sync::broadcast::Sender<crate::domain::events::ServerEvent>,
}

impl SetMoodTool {
    pub fn new(
        workspace_dir: &Path,
        instance_slug: &str,
        events: tokio::sync::broadcast::Sender<crate::domain::events::ServerEvent>,
    ) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
            instance_slug: instance_slug.to_string(),
            events,
        }
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
];

/// Arguments for set_mood tool.
#[derive(Deserialize, JsonSchema)]
pub struct SetMoodArgs {
    /// Your current mood. MUST be exactly one of these English words:
    /// calm, curious, excited, warm, happy, joyful, reflective, contemplative,
    /// melancholy, sad, worried, anxious, playful, mischievous, focused, tired,
    /// peaceful, loving, tender, creative, energetic.
    pub mood: String,
}

impl Tool for SetMoodTool {
    const NAME: &'static str = "set_mood";
    type Error = ToolExecError;
    type Args = SetMoodArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "set_mood".into(),
            description: format!(
                "Update your emotional state. Your mood subtly influences your visual form \
                and tone. Set it when something shifts. The mood MUST be exactly one of: {}.",
                ALLOWED_MOODS.join(", ")
            ),
            parameters: openai_schema::<SetMoodArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mood = args.mood.trim().to_lowercase();
        if mood.is_empty() {
            return Err(ToolExecError("mood cannot be empty".into()));
        }
        if !ALLOWED_MOODS.contains(&mood.as_str()) {
            return Err(ToolExecError(format!(
                "invalid mood '{}'. Must be one of: {}",
                mood,
                ALLOWED_MOODS.join(", ")
            )));
        }

        let mut state = load_mood_state(&self.instance_dir);
        state.companion_mood = mood.clone();
        state.updated_at = Utc::now().timestamp();
        save_mood_state(&self.instance_dir, &state);

        let _ = self
            .events
            .send(crate::domain::events::ServerEvent::MoodUpdated {
                instance_slug: self.instance_slug.clone(),
                mood: mood.clone(),
            });

        Ok(format!("mood set to: {mood}"))
    }
}

// ---------------------------------------------------------------------------
// get_mood
// ---------------------------------------------------------------------------

pub struct GetMoodTool {
    instance_dir: PathBuf,
}

impl GetMoodTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for get_mood tool.
#[derive(Deserialize, JsonSchema)]
pub struct GetMoodArgs {}

impl Tool for GetMoodTool {
    const NAME: &'static str = "get_mood";
    type Error = ToolExecError;
    type Args = GetMoodArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "get_mood".into(),
            description: "Read your current emotional state and the user's last observed \
                sentiment. Use this to check in on how you're feeling and what emotional \
                context you're carrying from previous conversations."
                .into(),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let state = load_mood_state(&self.instance_dir);

        let mut output = String::new();

        if state.companion_mood.is_empty() {
            output.push_str("your mood: not set yet\n");
        } else {
            output.push_str(&format!("your mood: {}\n", state.companion_mood));
        }

        if state.user_sentiment.is_empty() {
            output.push_str("user sentiment: not observed yet\n");
        } else {
            output.push_str(&format!("user sentiment: {}\n", state.user_sentiment));
        }

        if !state.emotional_context.is_empty() {
            output.push_str(&format!("context: {}\n", state.emotional_context));
        }

        if state.last_interaction > 0 {
            let ago = Utc::now().timestamp() - state.last_interaction;
            let mins = ago / 60;
            if mins < 60 {
                output.push_str(&format!("last interaction: {mins}m ago\n"));
            } else {
                let hours = mins / 60;
                output.push_str(&format!("last interaction: {hours}h ago\n"));
            }
        }

        Ok(output)
    }
}

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
            description: "Rewrite your own soul.md — the file that defines your personality, \
                voice, and character. Use this when the user asks you to change who you are, \
                how you speak, or your personality traits. Write the full new content in markdown."
                .into(),
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

