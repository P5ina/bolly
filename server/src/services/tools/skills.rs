use std::path::{Path, PathBuf};

use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// list_skills
// ---------------------------------------------------------------------------

pub struct ListSkillsTool {
    workspace_dir: PathBuf,
}

impl ListSkillsTool {
    pub fn new(workspace_dir: &Path) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ListSkillsArgs {
    /// Optional filter: "enabled", "disabled", or "all" (default: "all").
    pub filter: Option<String>,
}

impl Tool for ListSkillsTool {
    const NAME: &'static str = "list_skills";
    type Error = ToolExecError;
    type Args = ListSkillsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_skills".into(),
            description: "List all installed skills, showing their name, description, and enabled status."
                .into(),
            parameters: openai_schema::<ListSkillsArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let skills = crate::services::skills::list_skills(&self.workspace_dir);
        if skills.is_empty() {
            return Ok("no skills installed".into());
        }

        let filter = args.filter.as_deref().unwrap_or("all").to_lowercase();
        let filtered: Vec<_> = skills
            .iter()
            .filter(|s| match filter.as_str() {
                "enabled" => s.enabled,
                "disabled" => !s.enabled,
                _ => true,
            })
            .collect();

        if filtered.is_empty() {
            return Ok(format!("no {filter} skills"));
        }

        let mut out = String::new();
        for s in &filtered {
            let status = if s.enabled { "enabled" } else { "disabled" };
            let source = s.source.as_ref().map(|src| format!(" (from {})", src.repo)).unwrap_or_default();
            out.push_str(&format!(
                "- {} [{}]{}: {}\n",
                s.name, status, source, s.description
            ));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// activate_skill
// ---------------------------------------------------------------------------

pub struct ActivateSkillTool {
    workspace_dir: PathBuf,
}

impl ActivateSkillTool {
    pub fn new(workspace_dir: &Path) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ActivateSkillArgs {
    /// The name of the skill to activate (must match an installed, enabled skill).
    pub skill_name: String,
}

impl Tool for ActivateSkillTool {
    const NAME: &'static str = "activate_skill";
    type Error = ToolExecError;
    type Args = ActivateSkillArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "activate_skill".into(),
            description: "Activate a skill before using it. Call this when you decide to apply \
                a skill so the user can see which skill is guiding your response."
                .into(),
            parameters: openai_schema::<ActivateSkillArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let skills = crate::services::skills::list_skills(&self.workspace_dir);
        let needle = args.skill_name.to_lowercase();
        let found = skills.iter().find(|s| s.name.to_lowercase() == needle || s.id == needle);
        match found {
            Some(s) if s.enabled => Ok(format!("{} skill activated", s.name)),
            Some(s) => Err(ToolExecError(format!("skill '{}' is disabled", s.name))),
            None => Err(ToolExecError(format!("skill '{}' not found", args.skill_name))),
        }
    }
}
