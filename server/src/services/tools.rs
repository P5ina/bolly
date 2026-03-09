use std::{
    fmt,
    fs,
    path::{Path, PathBuf},
};

use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Shared error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ToolExecError(String);

impl fmt::Display for ToolExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ToolExecError {}

// ---------------------------------------------------------------------------
// edit_soul — lets the companion rewrite its own soul.md
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
            parameters: serde_json::to_value(schemars::schema_for!(EditSoulArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if let Some(parent) = self.soul_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }
        fs::write(&self.soul_path, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok("soul.md updated. your personality will reflect these changes on the next message."
            .into())
    }
}

// ---------------------------------------------------------------------------
// read_file — read a file from the instance workspace
// ---------------------------------------------------------------------------

pub struct ReadFileTool {
    instance_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for read_file tool.
#[derive(Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// Relative path within the instance directory (e.g. "soul.md", "drops/idea.md", "memory/facts.md").
    pub path: String,
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolExecError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".into(),
            description: "Read a file from your instance workspace. The path is relative to \
                your instance directory."
                .into(),
            parameters: serde_json::to_value(schemars::schema_for!(ReadFileArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = self.instance_dir.join(&args.path);

        // prevent path traversal
        if !target.starts_with(&self.instance_dir) {
            return Err(ToolExecError(
                "path must stay within instance directory".into(),
            ));
        }

        fs::read_to_string(&target).map_err(|e| ToolExecError(format!("{}: {e}", args.path)))
    }
}

// ---------------------------------------------------------------------------
// write_file — write a file in the instance workspace
// ---------------------------------------------------------------------------

pub struct WriteFileTool {
    instance_dir: PathBuf,
}

impl WriteFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for write_file tool.
#[derive(Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// Relative path within the instance directory (e.g. "drops/new-idea.md"). Parent directories are created automatically.
    pub path: String,
    /// The full content to write to the file.
    pub content: String,
}

impl Tool for WriteFileTool {
    const NAME: &'static str = "write_file";
    type Error = ToolExecError;
    type Args = WriteFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".into(),
            description: "Write or overwrite a file in your instance workspace. The path is \
                relative to your instance directory. Parent directories will be created \
                automatically."
                .into(),
            parameters: serde_json::to_value(schemars::schema_for!(WriteFileArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = self.instance_dir.join(&args.path);

        // prevent path traversal
        if !target.starts_with(&self.instance_dir) {
            return Err(ToolExecError(
                "path must stay within instance directory".into(),
            ));
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }

        fs::write(&target, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(format!(
            "wrote {} bytes to {}",
            args.content.len(),
            args.path
        ))
    }
}

// ---------------------------------------------------------------------------
// list_files — list files in the instance workspace
// ---------------------------------------------------------------------------

pub struct ListFilesTool {
    instance_dir: PathBuf,
}

impl ListFilesTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for list_files tool.
#[derive(Deserialize, JsonSchema)]
pub struct ListFilesArgs {
    /// Optional relative subdirectory path (e.g. "drops"). Omit to list the root of your instance directory.
    pub path: Option<String>,
}

impl Tool for ListFilesTool {
    const NAME: &'static str = "list_files";
    type Error = ToolExecError;
    type Args = ListFilesArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "list_files".into(),
            description: "List files and directories in your instance workspace.".into(),
            parameters: serde_json::to_value(schemars::schema_for!(ListFilesArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = match &args.path {
            Some(p) if !p.is_empty() => self.instance_dir.join(p),
            _ => self.instance_dir.clone(),
        };

        if !target.starts_with(&self.instance_dir) {
            return Err(ToolExecError(
                "path must stay within instance directory".into(),
            ));
        }

        if !target.is_dir() {
            return Err(ToolExecError(format!(
                "{} is not a directory",
                args.path.as_deref().unwrap_or(".")
            )));
        }

        let mut entries: Vec<String> = fs::read_dir(&target)
            .map_err(|e| ToolExecError(e.to_string()))?
            .filter_map(Result::ok)
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect();

        entries.sort();
        Ok(entries.join("\n"))
    }
}
