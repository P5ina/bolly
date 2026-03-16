use std::{fs, path::{Path, PathBuf}};

use crate::services::tool::{ToolDefinition, Tool};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{openai_schema, ToolExecError};

// ---------------------------------------------------------------------------
// memory_write — create or update a memory file
// ---------------------------------------------------------------------------

pub struct MemoryWriteTool {
    memory_dir: PathBuf,
}

impl MemoryWriteTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            memory_dir: workspace_dir.join("instances").join(instance_slug).join("memory"),
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
            description: "Create or update a memory file. Organize by folder (about/, preferences/, etc).".into(),
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

        match args.mode.as_str() {
            "append" => {
                let mut existing = fs::read_to_string(&full_path).unwrap_or_default();
                if !existing.ends_with('\n') && !existing.is_empty() {
                    existing.push('\n');
                }
                existing.push_str(&args.content);
                fs::write(&full_path, &existing).map_err(|e| ToolExecError(e.to_string()))?;
                Ok(format!("appended to {clean_path}"))
            }
            _ => {
                fs::write(&full_path, &args.content).map_err(|e| ToolExecError(e.to_string()))?;
                Ok(format!("wrote {clean_path}"))
            }
        }
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
}

impl MemoryForgetTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            memory_dir: workspace_dir.join("instances").join(instance_slug).join("memory"),
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
                // Clean up empty parent dirs
                if let Some(parent) = full_path.parent() {
                    let _ = cleanup_empty_dirs(parent, &self.memory_dir);
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
    memory_dir: PathBuf,
}

impl MemorySearchTool {
    pub fn new(workspace_dir: &Path, instance_slug: &str) -> Self {
        Self {
            memory_dir: workspace_dir.join("instances").join(instance_slug).join("memory"),
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
        let query = args.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(ToolExecError("query cannot be empty".into()));
        }
        let limit = args.limit.unwrap_or(5).min(20);

        // Tokenize query
        let query_terms: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|w| w.len() > 1)
            .collect();

        if query_terms.is_empty() {
            return Err(ToolExecError("query has no searchable terms".into()));
        }

        // Scan all memory files and build chunks
        let entries = crate::services::memory::scan_library(
            self.memory_dir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).unwrap_or(&self.memory_dir),
            self.memory_dir.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or(""),
        );

        if entries.is_empty() {
            return Ok("(empty library — no memories to search)".into());
        }

        // Count document frequency for IDF
        let total_chunks: f64;
        let mut chunks: Vec<ScoredChunk> = Vec::new();

        // Build chunks from all files
        let mut all_chunks: Vec<(String, String)> = Vec::new(); // (path, text)
        for entry in &entries {
            let full_path = self.memory_dir.join(&entry.path);
            let content = fs::read_to_string(&full_path).unwrap_or_default();

            if content.len() < 800 {
                // Small file — treat as single chunk
                all_chunks.push((entry.path.clone(), content));
            } else {
                // Large file — split into chunks by paragraphs
                let paragraphs: Vec<&str> = content.split("\n\n").collect();
                let mut current_chunk = String::new();
                let mut chunk_idx = 0;

                for para in paragraphs {
                    current_chunk.push_str(para);
                    current_chunk.push_str("\n\n");

                    if current_chunk.len() >= 600 {
                        all_chunks.push((
                            format!("{}#chunk{}", entry.path, chunk_idx),
                            current_chunk.clone(),
                        ));
                        current_chunk.clear();
                        chunk_idx += 1;
                    }
                }
                if !current_chunk.trim().is_empty() {
                    all_chunks.push((
                        format!("{}#chunk{}", entry.path, chunk_idx),
                        current_chunk,
                    ));
                }
            }
        }

        total_chunks = all_chunks.len() as f64;

        // Compute document frequency for each query term
        let mut df: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (_, text) in &all_chunks {
            let lower = text.to_lowercase();
            for term in &query_terms {
                if lower.contains(term) {
                    *df.entry(term).or_insert(0) += 1;
                }
            }
        }

        // Score each chunk using BM25-like formula
        let k1 = 1.5f64;
        let b = 0.75f64;
        let avg_len: f64 = all_chunks.iter().map(|(_, t)| t.len() as f64).sum::<f64>() / total_chunks.max(1.0);

        for (path, text) in &all_chunks {
            let lower = text.to_lowercase();
            let doc_len = text.len() as f64;
            let mut score = 0.0f64;

            for term in &query_terms {
                // Term frequency in this chunk
                let tf = lower.matches(term).count() as f64;
                if tf == 0.0 { continue; }

                // IDF: log((N - n + 0.5) / (n + 0.5))
                let n = *df.get(term).unwrap_or(&0) as f64;
                let idf = ((total_chunks - n + 0.5) / (n + 0.5) + 1.0).ln();

                // BM25 score
                let tf_norm = (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * doc_len / avg_len));
                score += idf * tf_norm;
            }

            // Bonus: path contains query terms
            let path_lower = path.to_lowercase();
            for term in &query_terms {
                if path_lower.contains(term) {
                    score += 2.0;
                }
            }

            if score > 0.0 {
                chunks.push(ScoredChunk {
                    path: path.clone(),
                    text: text.clone(),
                    score,
                });
            }
        }

        chunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        chunks.truncate(limit);

        if chunks.is_empty() {
            return Ok(format!("no memories matched \"{query}\""));
        }

        let mut result = format!("found {} relevant memories:\n\n", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            let path_display = chunk.path.split('#').next().unwrap_or(&chunk.path);
            let preview: String = chunk.text.chars().take(500).collect();
            let preview = preview.trim();
            result.push_str(&format!(
                "--- [{}/{} · {path_display} · score: {:.1}] ---\n{preview}\n\n",
                i + 1,
                chunks.len(),
                chunk.score,
            ));
        }
        Ok(result)
    }
}

struct ScoredChunk {
    path: String,
    text: String,
    score: f64,
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
