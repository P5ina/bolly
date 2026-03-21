use std::{fs, path::{Path, PathBuf}, sync::Arc};

use crate::services::tool::{ToolDefinition, Tool};
use crate::services::vector::VectorStore;
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// memory_write — create or update a memory file
// ---------------------------------------------------------------------------

pub struct MemoryWriteTool {
    memory_dir: PathBuf,
    instance_slug: String,
    vector_store: Arc<VectorStore>,
    google_ai_key: String,
}

impl MemoryWriteTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, vector_store: Arc<VectorStore>, google_ai_key: &str) -> Self {
        Self {
            memory_dir: workspace_dir.join("instances").join(instance_slug).join("memory"),
            instance_slug: instance_slug.to_string(),
            vector_store,
            google_ai_key: google_ai_key.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct MemoryWriteArgs {
    /// Path within the memory library (e.g. "about/basics.md", "moments/first-chat.md").
    /// Folders will be created automatically. Must end with .md.
    pub path: String,
    /// Content to write. For "write" mode, replaces the file. For "append" mode, adds to the end.
    pub content: String,
    /// "write" (default) to create/replace, or "append" to add to existing file.
    #[serde(default = "default_write_mode")]
    pub mode: String,
}

fn default_write_mode() -> String {
    "write".to_string()
}

impl Tool for MemoryWriteTool {
    const NAME: &'static str = "memory_write";
    type Error = ToolExecError;
    type Args = MemoryWriteArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_write".into(),
            description: "Create or update a memory file. Organize by folder (about/, preferences/, moments/, etc). \
                Files in pinned/ are always loaded into your context — use for triggers, rituals, critical references.".into(),
            parameters: openai_schema::<MemoryWriteArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let clean_path = sanitize_path(&args.path);
        if clean_path.is_empty() {
            return Err(ToolExecError("invalid path".into()));
        }

        let full_path = self.memory_dir.join(&clean_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
        }

        let final_content = match args.mode.as_str() {
            "append" => {
                let mut existing = fs::read_to_string(&full_path).unwrap_or_default();
                if !existing.ends_with('\n') && !existing.is_empty() {
                    existing.push('\n');
                }
                existing.push_str(&args.content);
                fs::write(&full_path, &existing).map_err(|e| ToolExecError(e.to_string()))?;
                existing
            }
            _ => {
                fs::write(&full_path, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
                args.content.clone()
            }
        };

        // Embed into vector store
        embed_memory_to_vector(&self.vector_store, &self.google_ai_key, &self.instance_slug, &clean_path, &final_content).await;

        Ok(format!("{} {clean_path}", if args.mode == "append" { "appended to" } else { "wrote" }))
    }
}

// ---------------------------------------------------------------------------
// memory_read — read a memory file or folder listing
// ---------------------------------------------------------------------------

pub struct MemoryReadTool {
    memory_dir: PathBuf,
}

impl MemoryReadTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            memory_dir: workspace_dir.join("instances").join(instance_slug).join("memory"),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct MemoryReadArgs {
    /// Path to read — a file path (e.g. "about/basics.md") returns its content,
    /// a folder path (e.g. "about/") lists its contents.
    pub path: String,
}

impl Tool for MemoryReadTool {
    const NAME: &'static str = "memory_read";
    type Error = ToolExecError;
    type Args = MemoryReadArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_read".into(),
            description: "Read a memory file or list folder contents.".into(),
            parameters: openai_schema::<MemoryReadArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let clean_path = args.path.trim().trim_start_matches('/');
        let full_path = self.memory_dir.join(clean_path);

        // Prevent traversal
        if !full_path.starts_with(&self.memory_dir) {
            return Err(ToolExecError("invalid path".into()));
        }

        if full_path.is_dir() {
            // List directory contents
            let entries = fs::read_dir(&full_path).map_err(|e| ToolExecError(e.to_string()))?;
            let mut items = Vec::new();
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = entry.path().is_dir();
                items.push(if is_dir {
                    format!("{name}/")
                } else {
                    name
                });
            }
            items.sort();
            if items.is_empty() {
                Ok("(empty folder)".into())
            } else {
                Ok(items.join("\n"))
            }
        } else if full_path.exists() {
            fs::read_to_string(&full_path).map_err(|e| ToolExecError(e.to_string()))
        } else {
            Err(ToolExecError(format!("not found: {clean_path}")))
        }
    }
}

// ---------------------------------------------------------------------------
// memory_list — browse the full library structure
// ---------------------------------------------------------------------------

pub struct MemoryListTool {
    workspace_dir: PathBuf,
    instance_slug: String,
}

impl MemoryListTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            workspace_dir: workspace_dir.to_path_buf(),
            instance_slug: instance_slug.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct MemoryListArgs {
    /// Optional: filter by folder prefix (e.g. "moments/"). Omit to list everything.
    #[serde(default)]
    pub prefix: String,
}

impl Tool for MemoryListTool {
    const NAME: &'static str = "memory_list";
    type Error = ToolExecError;
    type Args = MemoryListArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_list".into(),
            description: "List all memory files with summaries. Optional folder filter.".into(),
            parameters: openai_schema::<MemoryListArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let entries = crate::services::memory::scan_library(&self.workspace_dir, &self.instance_slug);

        if entries.is_empty() {
            return Ok("(empty library — no memories yet)".into());
        }

        let prefix = args.prefix.trim().trim_start_matches('/');
        let filtered: Vec<_> = if prefix.is_empty() {
            entries
        } else {
            entries.into_iter().filter(|e| e.path.starts_with(prefix)).collect()
        };

        if filtered.is_empty() {
            return Ok(format!("no memories under \"{prefix}\""));
        }

        let mut result = String::new();
        for entry in &filtered {
            result.push_str(&format!("{} — {}\n", entry.path, entry.summary));
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// memory_forget — delete a memory file
// ---------------------------------------------------------------------------

pub struct MemoryForgetTool {
    memory_dir: PathBuf,
    instance_slug: String,
    vector_store: Arc<VectorStore>,
    #[allow(dead_code)]
    google_ai_key: String,
}

impl MemoryForgetTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, vector_store: Arc<VectorStore>, google_ai_key: &str) -> Self {
        Self {
            memory_dir: workspace_dir.join("instances").join(instance_slug).join("memory"),
            instance_slug: instance_slug.to_string(),
            vector_store,
            google_ai_key: google_ai_key.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct MemoryForgetArgs {
    /// Path of the memory file to delete (e.g. "about/old-job.md").
    /// Or a search query — all files containing this text will be listed for confirmation.
    pub target: String,
}

impl Tool for MemoryForgetTool {
    const NAME: &'static str = "memory_forget";
    type Error = ToolExecError;
    type Args = MemoryForgetArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_forget".into(),
            description: "Delete a memory file by path.".into(),
            parameters: openai_schema::<MemoryForgetArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let target = args.target.trim();

        // If it looks like a path (contains / or ends with .md), try direct delete
        if target.contains('/') || target.ends_with(".md") {
            let clean = target.trim_start_matches('/');
            let full_path = self.memory_dir.join(clean);

            if !full_path.starts_with(&self.memory_dir) {
                return Err(ToolExecError("invalid path".into()));
            }

            if full_path.exists() {
                fs::remove_file(&full_path).map_err(|e| ToolExecError(e.to_string()))?;
                if let Some(parent) = full_path.parent() {
                    let _ = cleanup_empty_dirs(parent, &self.memory_dir);
                }
                if let Err(e) = self.vector_store.delete_by_path(&self.instance_slug, clean).await {
                    log::warn!("[memory_forget] vector delete failed: {e}");
                }
                return Ok(format!("deleted {clean}"));
            }
        }

        // Otherwise, search and delete matching files
        let workspace_dir = self.memory_dir.parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(&self.memory_dir);
        let instance_slug = self.memory_dir.parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let removed = crate::services::memory::forget_memories(workspace_dir, instance_slug, target);
        if removed == 0 {
            Ok(format!("no memories matched \"{target}\""))
        } else {
            Ok(format!("deleted {removed} memory file(s) matching \"{target}\""))
        }
    }
}

// ---------------------------------------------------------------------------
// memory_search — BM25-style semantic search across memory files
// ---------------------------------------------------------------------------

pub struct MemorySearchTool {
    instance_slug: String,
    vector_store: Arc<VectorStore>,
    google_ai_key: String,
}

impl MemorySearchTool {
    pub fn new(_workspace_dir: &Path, instance_slug: &str, vector_store: Arc<VectorStore>, google_ai_key: &str) -> Self {
        Self {
            instance_slug: instance_slug.to_string(),
            vector_store,
            google_ai_key: google_ai_key.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct MemorySearchArgs {
    /// Natural language search query. Can be a question, keywords, or a topic.
    /// Examples: "what does the user do for work", "music preferences", "that bug we discussed"
    pub query: String,
    /// Maximum number of results to return. Default: 5.
    pub limit: Option<usize>,
}

impl Tool for MemorySearchTool {
    const NAME: &'static str = "memory_search";
    type Error = ToolExecError;
    type Args = MemorySearchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_search".into(),
            description: "Search the memory library using natural language. \
                Finds relevant memories by matching words and concepts across all files. \
                Large files are searched at chunk level for precise results. \
                Use this instead of memory_list when looking for something specific."
                .into(),
            parameters: openai_schema::<MemorySearchArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let query = args.query.trim();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }
        let limit = args.limit.unwrap_or(5).min(20);

        let query_vec = crate::services::embedding::embed_text(
            &self.google_ai_key,
            query,
            crate::services::embedding::TaskType::RetrievalQuery,
        )
        .await
        .map_err(|e| ToolExecError(format!("embed query failed: {e}")))?;

        let results = self
            .vector_store
            .search(&self.instance_slug, query_vec, limit)
            .await
            .map_err(|e| ToolExecError(format!("vector search failed: {e}")))?;

        if results.is_empty() {
            return Ok(format!("no memories matched \"{query}\""));
        }

        let mut output = format!("found {} relevant memories:\n\n", results.len());
        for (i, r) in results.iter().enumerate() {
            let preview = r.content_preview.trim();
            output.push_str(&format!(
                "--- [{}/{} · {} · score: {:.4}] ---\n{preview}\n\n",
                i + 1,
                results.len(),
                r.path,
                r.score,
            ));
        }
        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sanitize_path(path: &str) -> String {
    let path = path.trim().trim_start_matches('/');
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == ".." || p.starts_with('.')) {
        return String::new();
    }
    let result = parts.join("/");
    if !result.ends_with(".md") {
        format!("{result}.md")
    } else {
        result
    }
}

fn cleanup_empty_dirs(dir: &Path, base: &Path) -> std::io::Result<()> {
    if dir == base || !dir.starts_with(base) {
        return Ok(());
    }
    if dir.is_dir() {
        let is_empty = fs::read_dir(dir)?.next().is_none();
        if is_empty {
            fs::remove_dir(dir)?;
            if let Some(parent) = dir.parent() {
                cleanup_empty_dirs(parent, base)?;
            }
        }
    }
    Ok(())
}

/// Embed a memory file into the vector store.
async fn embed_memory_to_vector(
    vector_store: &VectorStore,
    google_ai_key: &str,
    instance_slug: &str,
    path: &str,
    content: &str,
) {
    use crate::services::{embedding, vector};

    let chunks = vector::chunk_text(content);
    let mut chunk_vectors = Vec::new();

    for chunk in &chunks {
        match embedding::embed_text(google_ai_key, chunk, embedding::TaskType::RetrievalDocument).await {
            Ok(vec) => chunk_vectors.push((chunk.clone(), vec)),
            Err(e) => {
                log::warn!("[memory_tool] embed error for {path}: {e}");
                return;
            }
        }
    }

    if let Err(e) = vector_store.upsert_text_memory(instance_slug, path, chunk_vectors).await {
        log::warn!("[memory_tool] vector upsert failed for {path}: {e}");
    }
}
