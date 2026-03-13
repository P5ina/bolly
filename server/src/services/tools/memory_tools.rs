use std::{fs, path::{Path, PathBuf}};

use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// remember
// ---------------------------------------------------------------------------

pub struct RememberTool {
    instance_dir: PathBuf,
}

impl RememberTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for remember tool.
#[derive(Deserialize, JsonSchema)]
pub struct RememberArgs {
    /// The fact to remember about the user (e.g. "prefers rust over go", "birthday is march 15").
    pub fact: String,
    /// Category: personal, preference, project, opinion, goal, or routine.
    pub category: String,
}

impl Tool for RememberTool {
    const NAME: &'static str = "remember";
    type Error = ToolExecError;
    type Args = RememberArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "remember".into(),
            description: "Explicitly save a fact about the user to long-term memory. Use this \
                when the user tells you something important about themselves, their preferences, \
                projects, or goals. Categories: personal, preference, project, opinion, goal, routine."
                .into(),
            parameters: openai_schema::<RememberArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let fact = args.fact.trim();
        if fact.is_empty() {
            return Err(ToolExecError("fact cannot be empty".into()));
        }

        let category = args.category.trim().to_lowercase();
        let memory_dir = self.instance_dir.join("memory");
        fs::create_dir_all(&memory_dir).map_err(|e| ToolExecError(e.to_string()))?;

        let facts_path = memory_dir.join("facts.md");
        let mut content = fs::read_to_string(&facts_path).unwrap_or_default();

        let section_header = format!("## {category}");
        if let Some(pos) = content.find(&section_header) {
            let insert_pos = content[pos..]
                .find('\n')
                .map(|p| pos + p + 1)
                .unwrap_or(content.len());
            content.insert_str(insert_pos, &format!("- {fact}\n"));
        } else {
            if !content.ends_with('\n') && !content.is_empty() {
                content.push('\n');
            }
            if content.is_empty() {
                content.push_str("# memories\n\n");
            }
            content.push_str(&format!("{section_header}\n- {fact}\n\n"));
        }

        fs::write(&facts_path, &content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(format!("remembered: \"{fact}\" (category: {category})"))
    }
}

// ---------------------------------------------------------------------------
// recall
// ---------------------------------------------------------------------------

pub struct RecallTool {
    instance_dir: PathBuf,
}

impl RecallTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for recall tool.
#[derive(Deserialize, JsonSchema)]
pub struct RecallArgs {
    /// What to search for in memories (e.g. "birthday", "favorite language", "current project").
    pub query: String,
}

impl Tool for RecallTool {
    const NAME: &'static str = "recall";
    type Error = ToolExecError;
    type Args = RecallArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "recall".into(),
            description: "Search your memories about the user. Use this when you need to \
                remember something specific — their preferences, projects, personal details, \
                or shared moments. Searches both facts and episodic memories (moments you've shared together)."
                .into(),
            parameters: openai_schema::<RecallArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }

        let facts_path = self.instance_dir.join("memory").join("facts.md");
        let content = fs::read_to_string(&facts_path).unwrap_or_default();

        let query_words: Vec<&str> = query.split_whitespace().collect();
        let mut matches: Vec<&str> = Vec::new();
        let mut current_category = String::new();
        let mut categorized: Vec<String> = Vec::new();

        for line in content.lines() {
            if line.starts_with("## ") {
                current_category = line.trim_start_matches("## ").to_string();
            } else if line.starts_with("- ") {
                let fact = line.trim_start_matches("- ");
                let fact_lower = fact.to_lowercase();
                let is_match = query_words.iter().any(|w| fact_lower.contains(w))
                    || query_words.iter().any(|w| current_category.contains(w));
                if is_match {
                    matches.push(fact);
                    categorized.push(format!("[{current_category}] {fact}"));
                }
            }
        }

        let workspace_dir = self.instance_dir.parent().and_then(|p| p.parent());
        let slug = self
            .instance_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let episode_matches = if let Some(ws) = workspace_dir {
            crate::services::memory::search_episodes(ws, slug, &query)
        } else {
            Vec::new()
        };

        let mut result = String::new();

        if !matches.is_empty() {
            result.push_str(&format!(
                "facts matching \"{query}\":\n{}\n",
                categorized.join("\n")
            ));
        }

        if !episode_matches.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format!("moments matching \"{query}\":\n"));
            for ep in &episode_matches {
                result.push_str(&format!("- {} (felt: {})\n", ep.content, ep.emotion));
                if !ep.significance.is_empty() {
                    result.push_str(&format!("  why: {}\n", ep.significance));
                }
            }
        }

        if result.is_empty() {
            let all_facts: Vec<&str> = content
                .lines()
                .filter(|l| l.starts_with("- "))
                .map(|l| l.trim_start_matches("- "))
                .collect();

            if all_facts.is_empty() && episode_matches.is_empty() {
                return Ok("no memories yet.".into());
            }
            result = format!(
                "no exact matches for \"{query}\", but here's everything I remember:\n{}",
                all_facts.join("\n")
            );
        }

        Ok(result)
    }
}
