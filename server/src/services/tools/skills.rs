use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::domain::skill::SkillKind;
use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// list_skills
// ---------------------------------------------------------------------------

pub struct ListSkillsTool {
    workspace_dir: PathBuf,
    api_key: String,
}

impl ListSkillsTool {
    pub fn new(workspace_dir: &Path, api_key: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            api_key: api_key.to_string(),
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
            description: "List installed skills with name, description, and status.".into(),
            parameters: openai_schema::<ListSkillsArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut skills = crate::services::skills::list_skills(&self.workspace_dir);

        // Merge Anthropic skills from API
        if !self.api_key.is_empty() {
            if let Ok(remote) = crate::services::anthropic_skills::fetch_available_skills(&self.api_key).await {
                let local_ids: std::collections::HashSet<_> = skills.iter().map(|s| s.id.clone()).collect();
                for s in remote {
                    if !local_ids.contains(&s.id) {
                        skills.push(s);
                    }
                }
            }
        }

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
            let kind_label = match s.kind {
                SkillKind::Local => "local",
                SkillKind::Anthropic => "anthropic",
            };
            let source = s.source.as_ref().map(|src| format!(" (from {})", src.repo)).unwrap_or_default();
            out.push_str(&format!(
                "- {} [{}]{}: {}\n",
                s.name, kind_label, source, s.description
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
    api_key: String,
    /// Shared set of activated Anthropic skill IDs for this chat session.
    activated_anthropic: Arc<RwLock<HashSet<String>>>,
}

impl ActivateSkillTool {
    pub fn new(workspace_dir: &Path, api_key: &str, activated: Arc<RwLock<HashSet<String>>>) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            api_key: api_key.to_string(),
            activated_anthropic: activated,
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
            description: "Activate a skill before using it. Local skills return instructions; Anthropic skills are loaded into the code execution sandbox.".into(),
            parameters: openai_schema::<ActivateSkillArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut skills = crate::services::skills::list_skills(&self.workspace_dir);
        // Merge Anthropic skills so activate_skill can find them
        if !self.api_key.is_empty() {
            if let Ok(remote) = crate::services::anthropic_skills::fetch_available_skills(&self.api_key).await {
                let local_ids: std::collections::HashSet<_> = skills.iter().map(|s| s.id.clone()).collect();
                for s in remote {
                    if !local_ids.contains(&s.id) {
                        skills.push(s);
                    }
                }
            }
        }
        let needle = args.skill_name.to_lowercase();
        let found = skills.iter().find(|s| s.name.to_lowercase() == needle || s.id == needle);
        match found {
            Some(s) if s.enabled => {
                match s.kind {
                    SkillKind::Local => {
                        // Local skill: return instructions (prompt injection)
                        let mut result = format!("# {} skill activated\n\n{}", s.name, s.instructions);
                        let refs: Vec<_> = s
                            .resources
                            .iter()
                            .filter(|r| r.starts_with("references/"))
                            .collect();
                        if !refs.is_empty() {
                            result.push_str("\n\n## reference files\n\
                                **MANDATORY**: read these BEFORE running any commands — use `read_skill_reference` with skill_id=\"");
                            result.push_str(&s.id);
                            result.push_str("\":\n");
                            for r in refs {
                                result.push_str(&format!("- {}\n", r));
                            }
                        }
                        Ok(result)
                    }
                    SkillKind::Anthropic => {
                        // Anthropic skill: add to container for code execution
                        if let Some(ref sid) = s.anthropic_skill_id {
                            let mut activated = self.activated_anthropic.write().await;
                            activated.insert(sid.clone());
                            Ok(format!(
                                "# {} skill activated\n\nThis skill runs in the code execution sandbox. \
                                 Write Python code to use it — the skill files are available at /skills/. \
                                 Generated files will be automatically sent to the user.",
                                s.name
                            ))
                        } else {
                            Err(ToolExecError(format!("skill '{}' is missing Anthropic skill ID", s.name)))
                        }
                    }
                }
            }
            Some(s) => Err(ToolExecError(format!("skill '{}' is disabled", s.name))),
            None => Err(ToolExecError(format!("skill '{}' not found", args.skill_name))),
        }
    }
}

// ---------------------------------------------------------------------------
// read_skill_reference
// ---------------------------------------------------------------------------

pub struct ReadSkillReferenceTool {
    workspace_dir: PathBuf,
}

impl ReadSkillReferenceTool {
    pub fn new(workspace_dir: &Path) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadSkillReferenceArgs {
    /// The skill ID (directory name) of the installed skill.
    pub skill_id: String,
    /// The resource path to read, e.g. "references/core-exporting.md".
    pub filename: String,
}

impl Tool for ReadSkillReferenceTool {
    const NAME: &'static str = "read_skill_reference";
    type Error = ToolExecError;
    type Args = ReadSkillReferenceArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_skill_reference".into(),
            description: "Read a reference file bundled with a skill.".into(),
            parameters: openai_schema::<ReadSkillReferenceArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let skill = crate::services::skills::get_skill(&self.workspace_dir, &args.skill_id)
            .ok_or_else(|| ToolExecError(format!("skill '{}' not found", args.skill_id)))?;

        if !skill.resources.contains(&args.filename) {
            let available = if skill.resources.is_empty() {
                "this skill has no reference files".to_string()
            } else {
                format!("available: {}", skill.resources.join(", "))
            };
            return Err(ToolExecError(format!(
                "'{}' not found in skill '{}'. {}",
                args.filename, args.skill_id, available
            )));
        }

        let path = self
            .workspace_dir
            .join("skills")
            .join(&args.skill_id)
            .join(&args.filename);

        std::fs::read_to_string(&path).map_err(|e| {
            ToolExecError(format!("failed to read '{}': {}", args.filename, e))
        })
    }
}
