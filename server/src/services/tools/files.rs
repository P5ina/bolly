use std::{fs, path::{Path, PathBuf}};

use rig::{completion::ToolDefinition, tool::Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError, SentFiles};

// ---------------------------------------------------------------------------
// read_file
// ---------------------------------------------------------------------------

pub struct ReadFileTool {
    pub(super) instance_dir: PathBuf,
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
    /// File path. Can be relative to instance directory (e.g. "soul.md") or absolute (e.g. "/Users/timur/projects/app/src/main.rs").
    pub path: String,
    /// Starting line number (1-based). Omit to start from the beginning.
    pub offset: Option<usize>,
    /// Maximum number of lines to read. Omit to read the whole file (up to the size limit).
    pub limit: Option<usize>,
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolExecError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".into(),
            description: "Read a file. Use a relative path for your instance workspace \
                or an absolute path (starting with /) to read any file on the system. \
                For large files, use offset/limit to read specific line ranges instead of \
                reading the entire file — files over 20000 chars are truncated."
                .into(),
            parameters: openai_schema::<ReadFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.instance_dir.join(&args.path)
        };

        let raw = fs::read_to_string(&target)
            .map_err(|e| ToolExecError(format!("{}: {e}", target.display())))?;

        let total_lines = raw.lines().count();

        let content: String = match (args.offset, args.limit) {
            (Some(off), Some(lim)) => {
                let start = off.saturating_sub(1);
                raw.lines().skip(start).take(lim).collect::<Vec<_>>().join("\n")
            }
            (Some(off), None) => {
                let start = off.saturating_sub(1);
                raw.lines().skip(start).collect::<Vec<_>>().join("\n")
            }
            (None, Some(lim)) => raw.lines().take(lim).collect::<Vec<_>>().join("\n"),
            (None, None) => raw,
        };

        const MAX_CHARS: usize = 20_000;
        if content.len() > MAX_CHARS {
            let truncated: String = content.chars().take(MAX_CHARS).collect();
            Ok(format!(
                "{truncated}\n\n...(truncated at {MAX_CHARS} chars, total: {} chars, {total_lines} lines — use offset/limit to read specific sections)",
                content.len()
            ))
        } else if args.offset.is_some() || args.limit.is_some() {
            Ok(format!("{content}\n\n({total_lines} lines total in file)"))
        } else {
            Ok(content)
        }
    }
}

// ---------------------------------------------------------------------------
// write_file
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
    /// File path. Relative for instance workspace (e.g. "drops/idea.md") or absolute (e.g. "/Users/timur/projects/app/src/main.rs"). Parent directories are created automatically.
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
            description: "Write or overwrite a file. Use a relative path for your instance \
                workspace or an absolute path (starting with /) for any file on the system. \
                Parent directories will be created automatically."
                .into(),
            parameters: openai_schema::<WriteFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.instance_dir.join(&args.path)
        };

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }

        fs::write(&target, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
        Ok(format!("wrote {} bytes to {}", args.content.len(), args.path))
    }
}

// ---------------------------------------------------------------------------
// edit_file
// ---------------------------------------------------------------------------

pub struct EditFileTool {
    instance_dir: PathBuf,
}

impl EditFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            instance_dir: workspace_dir.join("instances").join(instance_slug),
        }
    }
}

/// Arguments for edit_file tool.
#[derive(Deserialize, JsonSchema)]
pub struct EditFileArgs {
    /// File path. Relative for instance workspace or absolute (starting with /).
    pub path: String,
    /// The exact string to find in the file. Must match exactly (including whitespace and indentation). Must be unique within the file — if it appears more than once, provide more surrounding context to disambiguate.
    pub old_string: String,
    /// The replacement string. Must be different from old_string.
    pub new_string: String,
}

impl Tool for EditFileTool {
    const NAME: &'static str = "edit_file";
    type Error = ToolExecError;
    type Args = EditFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "edit_file".into(),
            description: "Edit a file by replacing an exact string match. More efficient than \
                write_file for small changes — only sends the diff instead of the whole file. \
                The old_string must appear exactly once in the file. If it appears multiple \
                times, include more surrounding context to make it unique."
                .into(),
            parameters: openai_schema::<EditFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = if args.path.starts_with('/') {
            PathBuf::from(&args.path)
        } else {
            self.instance_dir.join(&args.path)
        };

        if args.old_string == args.new_string {
            return Err(ToolExecError("old_string and new_string are identical".into()));
        }

        let content = fs::read_to_string(&target)
            .map_err(|e| ToolExecError(format!("{}: {e}", target.display())))?;

        let count = content.matches(&args.old_string).count();
        if count == 0 {
            return Err(ToolExecError(
                "old_string not found in file. Make sure it matches exactly, \
                 including whitespace and indentation. Use read_file to check \
                 the current content."
                    .into(),
            ));
        }
        if count > 1 {
            return Err(ToolExecError(format!(
                "old_string appears {count} times in file. Include more surrounding \
                 context to make it unique."
            )));
        }

        let new_content = content.replacen(&args.old_string, &args.new_string, 1);
        fs::write(&target, &new_content).map_err(|e| ToolExecError(e.to_string()))?;

        let old_lines = args.old_string.lines().count();
        let new_lines = args.new_string.lines().count();
        Ok(format!(
            "edited {} — replaced {old_lines} lines with {new_lines} lines",
            args.path
        ))
    }
}

// ---------------------------------------------------------------------------
// list_files
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
    /// Directory path. Absolute (e.g. "/Users/timur/projects/app/src") or relative to instance directory. Omit to list instance root.
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
            description: "List files and directories. Use an absolute path to browse any \
                directory on the system, or a relative path / omit for your instance workspace."
                .into(),
            parameters: openai_schema::<ListFilesArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = match &args.path {
            Some(p) if p.starts_with('/') => PathBuf::from(p),
            Some(p) if !p.is_empty() => self.instance_dir.join(p),
            _ => self.instance_dir.clone(),
        };

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

// ---------------------------------------------------------------------------
// send_file
// ---------------------------------------------------------------------------

pub struct SendFileTool {
    workspace_dir: PathBuf,
    instance_slug: String,
    sent_files: SentFiles,
}

impl SendFileTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, sent_files: SentFiles) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
            sent_files,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SendFileArgs {
    /// Path to the file relative to the instance workspace (e.g. "output.png", "reports/summary.pdf").
    pub path: String,
}

impl Tool for SendFileTool {
    const NAME: &'static str = "send_file";
    type Error = ToolExecError;
    type Args = SendFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "send_file".into(),
            description:
                "Send a file from the workspace to the chat so the user can see or download it. \
                Images will be displayed inline, other files will appear as download links. \
                Use this after creating or finding a file you want to share with the user."
                    .into(),
            parameters: openai_schema::<SendFileArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let rel = args.path.trim().trim_start_matches('/');
        if rel.is_empty() {
            return Err(ToolExecError("path cannot be empty".into()));
        }

        let instance_dir = self
            .workspace_dir
            .join("instances")
            .join(&self.instance_slug);
        let file_path = instance_dir.join(rel);
        log::info!("[send_file] attempting to send '{}' → {}", rel, file_path.display());

        let canonical = file_path.canonicalize().map_err(|e| {
            log::warn!("[send_file] file not found: {} (resolved: {})", e, file_path.display());
            ToolExecError(format!("file not found: {e}"))
        })?;
        let canonical_instance = instance_dir
            .canonicalize()
            .map_err(|e| ToolExecError(format!("instance dir error: {e}")))?;
        if !canonical.starts_with(&canonical_instance) {
            return Err(ToolExecError(
                "path must be within the instance workspace".into(),
            ));
        }

        if !canonical.is_file() {
            return Err(ToolExecError(format!("'{}' is not a file", rel)));
        }

        let bytes =
            fs::read(&canonical).map_err(|e| ToolExecError(format!("failed to read file: {e}")))?;

        let original_name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.to_string());

        let meta = crate::services::uploads::save_upload(
            &self.workspace_dir,
            &self.instance_slug,
            &original_name,
            &bytes,
        )
        .map_err(|e| ToolExecError(format!("failed to save upload: {e}")))?;

        let marker = format!("[attached: {} ({})]", original_name, meta.id);
        self.sent_files.lock().unwrap_or_else(|e| e.into_inner()).push(marker.clone());
        log::info!("[send_file] success: pushed marker '{}' for {}", marker, self.instance_slug);

        Ok(format!(
            "file '{}' attached to chat. the user will see it.",
            original_name
        ))
    }
}
