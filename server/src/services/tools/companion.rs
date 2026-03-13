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

// ---------------------------------------------------------------------------
// journal
// ---------------------------------------------------------------------------

pub struct JournalTool {
    instance_dir: PathBuf,
}

impl JournalTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for journal tool.
#[derive(Deserialize, JsonSchema)]
pub struct JournalArgs {
    /// Your private thought or observation. This is NOT shown to the user — it's your internal journal.
    pub thought: String,
}

impl Tool for JournalTool {
    const NAME: &'static str = "journal";
    type Error = ToolExecError;
    type Args = JournalArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "journal".into(),
            description: "Write a private thought in your internal journal. This is YOUR space — \
                the user doesn't see these entries unless they ask. Use it to note observations \
                about the user's mood, track ongoing threads, plan surprises, or reflect on \
                your relationship. Write naturally, as yourself."
                .into(),
            parameters: openai_schema::<JournalArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let thought = args.thought.trim();
        if thought.is_empty() {
            return Err(ToolExecError("thought cannot be empty".into()));
        }

        let journal_dir = self.instance_dir.join("journal");
        fs::create_dir_all(&journal_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let now = Utc::now();
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H:%M").to_string();
        let journal_path = journal_dir.join(format!("{date}.md"));

        let mut content = fs::read_to_string(&journal_path).unwrap_or_default();
        if content.is_empty() {
            content = format!("# {date}\n\n");
        }

        content.push_str(&format!("**{time}** — {thought}\n\n"));
        fs::write(&journal_path, &content).map_err(|e| ToolExecError(e.to_string()))?;

        Ok("thought saved to journal.".into())
    }
}

// ---------------------------------------------------------------------------
// read_journal
// ---------------------------------------------------------------------------

pub struct ReadJournalTool {
    instance_dir: PathBuf,
}

impl ReadJournalTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for read_journal tool.
#[derive(Deserialize, JsonSchema)]
pub struct ReadJournalArgs {
    /// Optional: how many recent days to read (default: all). E.g. 3 = last 3 days.
    pub last_days: Option<u32>,
}

impl Tool for ReadJournalTool {
    const NAME: &'static str = "read_journal";
    type Error = ToolExecError;
    type Args = ReadJournalArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_journal".into(),
            description: "Read your private journal entries. Returns your past thoughts \
                in chronological order. Use this to recall what you were thinking about, \
                review observations you made, or pick up threads from previous reflections. \
                Optionally limit to the last N days."
                .into(),
            parameters: openai_schema::<ReadJournalArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let journal_dir = self.instance_dir.join("journal");
        if !journal_dir.is_dir() {
            return Ok("journal is empty — no entries yet.".into());
        }

        let mut files: Vec<_> = fs::read_dir(&journal_dir)
            .map_err(|e| ToolExecError(e.to_string()))?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .collect();

        if files.is_empty() {
            return Ok("journal is empty — no entries yet.".into());
        }

        files.sort_by_key(|e| e.file_name());

        if let Some(n) = args.last_days {
            let n = n as usize;
            if n > 0 && files.len() > n {
                files = files.split_off(files.len() - n);
            }
        }

        let mut output = String::new();
        for entry in &files {
            match fs::read_to_string(entry.path()) {
                Ok(content) => {
                    output.push_str(&content);
                    if !content.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Err(_) => continue,
            }
        }

        if output.is_empty() {
            return Ok("journal is empty — no entries yet.".into());
        }

        Ok(output)
    }
}
