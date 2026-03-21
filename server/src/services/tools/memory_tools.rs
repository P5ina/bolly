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
    uploads_dir: PathBuf,
    instance_slug: String,
    vector_store: Arc<VectorStore>,
    google_ai_key: String,
}

impl MemoryWriteTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, vector_store: Arc<VectorStore>, google_ai_key: &str) -> Self {
        Self {
            memory_dir: workspace_dir.join("instances").join(instance_slug).join("memory"),
            uploads_dir: workspace_dir.join("instances").join(instance_slug).join("uploads"),
            instance_slug: instance_slug.to_string(),
            vector_store,
            google_ai_key: google_ai_key.to_string(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct MemoryWriteArgs {
    /// Path within the memory library (e.g. "about/basics.md", "moments/sunset.jpg").
    /// Folders will be created automatically. For text files must end with .md.
    /// For images, use the original extension (.jpg, .png, .webp).
    pub path: String,
    /// Content to write. For text files: the memory content. For images: leave empty.
    #[serde(default)]
    pub content: String,
    /// "write" (default) to create/replace, or "append" to add to existing file.
    #[serde(default = "default_write_mode")]
    pub mode: String,
    /// Upload ID of an image to save as a memory (e.g. "upload_1234567890").
    /// When provided, the image is copied from uploads to the memory library.
    #[serde(default)]
    pub image_upload_id: Option<String>,
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
                Files in pinned/ are always loaded into your context — use for triggers, rituals, critical references. \
                Can also save images: set image_upload_id to the upload ID and path to where you want it (e.g. moments/sunset.jpg).".into(),
            parameters: openai_schema::<MemoryWriteArgs>(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Image save mode
        if let Some(upload_id) = &args.image_upload_id {
            let clean_path = sanitize_image_path(&args.path);
            if clean_path.is_empty() {
                return Err(ToolExecError("invalid image path".into()));
            }

            // Find the uploaded file
            let meta_path = self.uploads_dir.join(format!("{upload_id}.json"));
            let meta_str = fs::read_to_string(&meta_path)
                .map_err(|_| ToolExecError(format!("upload {upload_id} not found")))?;
            let meta: serde_json::Value = serde_json::from_str(&meta_str)
                .map_err(|_| ToolExecError("invalid upload metadata".into()))?;
            let stored_name = meta["stored_name"].as_str()
                .ok_or_else(|| ToolExecError("missing stored_name".into()))?;
            let mime_type = meta["mime_type"].as_str().unwrap_or("image/jpeg");

            let src = self.uploads_dir.join(stored_name);
            let dst = self.memory_dir.join(&clean_path);

            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| ToolExecError(e.to_string()))?;
            }
            fs::copy(&src, &dst).map_err(|e| ToolExecError(e.to_string()))?;

            // Embed image into vector store
            if let Ok(bytes) = fs::read(&dst) {
                if bytes.len() < 20 * 1024 * 1024 {
                    match crate::services::embedding::embed_image(&self.google_ai_key, &bytes, mime_type).await {
                        Ok(vec) => {
                            let desc = if args.content.is_empty() {
                                clean_path.clone()
                            } else {
                                args.content.clone()
                            };
                            if let Err(e) = self.vector_store.upsert_media(
                                &self.instance_slug, &clean_path, "media_image",
                                mime_type, &clean_path, &desc, vec,
                            ).await {
                                log::warn!("[memory_write] vector upsert failed for image: {e}");
                            }
                        }
                        Err(e) => log::warn!("[memory_write] embed image failed: {e}"),
                    }
                }
            }

            // If content provided, save it as a description sidecar
            if !args.content.is_empty() {
                let desc_path = format!("{clean_path}.md");
                let desc_full = self.memory_dir.join(&desc_path);
                let _ = fs::write(&desc_full, &args.content);
                embed_memory_to_vector(&self.vector_store, &self.google_ai_key, &self.instance_slug, &desc_path, &args.content).await;
            }

            return Ok(format!("saved image to {clean_path}"));
        }

        // Text file mode
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
    uploads_dir: PathBuf,
    instance_slug: String,
    vector_store: Arc<VectorStore>,
    google_ai_key: String,
    public_url: String,
    auth_token: String,
}

impl MemorySearchTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str, vector_store: Arc<VectorStore>, google_ai_key: &str) -> Self {
        let public_url = std::env::var("BOLLY_PUBLIC_URL").unwrap_or_default();
        let auth_token = std::env::var("BOLLY_AUTH_TOKEN").unwrap_or_default();
        Self {
            uploads_dir: workspace_dir.join("instances").join(instance_slug).join("uploads"),
            instance_slug: instance_slug.to_string(),
            vector_store,
            google_ai_key: google_ai_key.to_string(),
            public_url,
            auth_token,
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

            // For image results, include the image URL so the LLM can see it
            if r.source_type == "media_image" && !self.public_url.is_empty() {
                if let Some(upload_id) = &r.upload_id {
                    let url = format!(
                        "{}/public/files/{}/{upload_id}?token={}",
                        self.public_url, self.instance_slug, self.auth_token,
                    );
                    output.push_str(&format!("__IMAGE_URL__:{url}\n\n"));
                }
            }
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

/// Sanitize path for image files — allows image extensions instead of forcing .md.
fn sanitize_image_path(path: &str) -> String {
    let path = path.trim().trim_start_matches('/');
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == ".." || p.starts_with('.')) {
        return String::new();
    }
    let result = parts.join("/");
    let lower = result.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png")
        || lower.ends_with(".webp") || lower.ends_with(".gif") {
        result
    } else {
        format!("{result}.jpg")
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
