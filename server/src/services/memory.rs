use std::path::Path;

use crate::domain::chat::ChatMessage;
use crate::domain::memory::MemoryEntry;
use crate::services::llm::LlmBackend;

fn memory_dir(workspace_dir: &Path, instance_slug: &str) -> std::path::PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("memory")
}

/// Scan the memory library and return a catalog of all .md files.
pub fn scan_library(workspace_dir: &Path, instance_slug: &str) -> Vec<MemoryEntry> {
    let dir = memory_dir(workspace_dir, instance_slug);
    if !dir.exists() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    scan_dir_recursive(&dir, &dir, &mut entries);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn scan_dir_recursive(base: &Path, current: &Path, entries: &mut Vec<MemoryEntry>) {
    let read_dir = match std::fs::read_dir(current) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.filter_map(Result::ok) {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files, legacy archives, and non-directories starting with _
        if file_name.starts_with('.') || file_name.starts_with('_') {
            continue;
        }
        if path.is_dir() {
            scan_dir_recursive(base, &path, entries);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let is_text = ext == "md";
            let is_media = matches!(ext.as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "svg" |
                "pdf" | "mp4" | "mov" | "mp3" | "wav"
            );

            if !is_text && !is_media {
                continue;
            }

            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            if is_media {
                let size = std::fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
                let kind = match ext.as_str() {
                    "jpg" | "jpeg" | "png" | "webp" | "gif" | "svg" => "image",
                    "pdf" => "document",
                    "mp4" | "mov" => "video",
                    "mp3" | "wav" => "audio",
                    _ => "file",
                };
                entries.push(MemoryEntry {
                    path: rel,
                    summary: format!("[{kind}: {ext}]"),
                    size,
                });
            } else {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let summary = content
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .take(120)
                    .collect::<String>();
                let size = content.len();
                entries.push(MemoryEntry {
                    path: rel,
                    summary,
                    size,
                });
            }
        }
    }
}

/// Path to the static memory catalog snapshot file for an instance.
fn catalog_snapshot_path(workspace_dir: &Path, instance_slug: &str) -> std::path::PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("memory_catalog.txt")
}

/// Rebuild and persist the memory catalog snapshot to disk.
/// Call this after context clear or compaction — not on every request.
pub fn rebuild_catalog_snapshot(workspace_dir: &Path, instance_slug: &str) {
    let entries = scan_library(workspace_dir, instance_slug);
    if entries.is_empty() {
        return;
    }

    let dir = memory_dir(workspace_dir, instance_slug);

    // Separate pinned memories (full content in prompt) from regular (catalog only)
    let (pinned, regular): (Vec<_>, Vec<_>) = entries
        .iter()
        .partition(|e| e.path.starts_with("pinned/"));

    let mut prompt = format!(
        "## memory\nyou have a personal memory library. use `memory_read` to read memories when relevant.\n"
    );

    // Pinned memories: always-loaded, full content inline
    if !pinned.is_empty() {
        prompt.push_str("\n### pinned (always loaded)\n");
        for entry in &pinned {
            let full_path = dir.join(&entry.path);
            let content = std::fs::read_to_string(&full_path).unwrap_or_default();
            prompt.push_str(&format!("\n**{}**\n{}\n", entry.path, content.trim()));
        }
    }

    // Regular catalog: paths + summaries
    if !regular.is_empty() {
        prompt.push_str(&format!("\ncatalog ({} files):\n", regular.len()));
        for entry in &regular {
            prompt.push_str(&format!("- {} — {}\n", entry.path, entry.summary));
        }
    }

    prompt.push_str(
        "\nuse these memories naturally — `memory_read` what you need. \
         don't announce that you remember — just know.",
    );

    let path = catalog_snapshot_path(workspace_dir, instance_slug);
    if let Err(e) = std::fs::write(&path, &prompt) {
        log::warn!("[memory] failed to write catalog snapshot: {e}");
    } else {
        log::info!(
            "[memory] catalog snapshot rebuilt: {} pinned, {} catalog",
            pinned.len(), regular.len()
        );
    }
}

/// Load the static memory catalog snapshot from disk.
/// Returns empty string if no snapshot exists yet (first boot / pre-compaction).
pub fn load_catalog_snapshot(workspace_dir: &Path, instance_slug: &str) -> String {
    let path = catalog_snapshot_path(workspace_dir, instance_slug);
    std::fs::read_to_string(&path).unwrap_or_default()
}

/// Build the memory prompt for the system prompt.
/// NOTE: this scans disk on every call — use load_catalog_snapshot() for the static version.
pub fn build_memory_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    // Delegate to rebuild logic (same format as cached version)
    rebuild_catalog_snapshot(workspace_dir, instance_slug);
    load_catalog_snapshot(workspace_dir, instance_slug)
}

/// Build a full library catalog for memory maintenance (heartbeat).
/// Shows every file path, size, and first-line summary.
pub fn build_library_catalog(workspace_dir: &Path, instance_slug: &str) -> String {
    let entries = scan_library(workspace_dir, instance_slug);
    if entries.is_empty() {
        return String::from("(empty library)");
    }

    let total_files = entries.len();
    let total_bytes: usize = entries.iter().map(|e| e.size).sum();

    let mut result = format!("{total_files} files, {total_bytes} bytes total:\n");
    for entry in &entries {
        result.push_str(&format!("- {} ({} bytes) — {}\n", entry.path, entry.size, entry.summary));
    }
    result
}

/// Run legacy migration for all instances in the workspace.
pub fn migrate_all_instances(workspace_dir: &Path) {
    let instances_dir = workspace_dir.join("instances");
    let entries = match std::fs::read_dir(&instances_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(Result::ok) {
        if entry.path().is_dir() {
            let slug = entry.file_name().to_string_lossy().to_string();
            migrate_legacy_memory(workspace_dir, &slug);
        }
    }
}

/// Migrate legacy memory format (facts.md + episodes.md) into the new library structure.
/// Called once per instance — creates a `.migrated` marker to avoid re-running.
pub fn migrate_legacy_memory(workspace_dir: &Path, instance_slug: &str) {
    let dir = memory_dir(workspace_dir, instance_slug);
    let marker = dir.join(".migrated");
    if marker.exists() {
        return;
    }

    let facts_path = dir.join("facts.md");
    let episodes_path = dir.join("episodes.md");
    let db_path = dir.join("memory.db");

    let has_legacy = facts_path.exists() || episodes_path.exists();
    if !has_legacy {
        // No legacy data — just mark as migrated
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&marker, "");
        return;
    }

    log::info!("migrating legacy memory for {instance_slug}");

    // Migrate facts.md → one file per category under facts/
    if facts_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&facts_path) {
            let mut current_category = String::from("general");
            let mut category_facts: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();

            for line in content.lines() {
                let line = line.trim();
                if let Some(cat) = line.strip_prefix("## ") {
                    current_category = cat.trim().to_lowercase();
                } else if let Some(fact) = line.strip_prefix("- ") {
                    category_facts
                        .entry(current_category.clone())
                        .or_default()
                        .push(fact.to_string());
                }
            }

            for (category, facts) in &category_facts {
                if facts.is_empty() {
                    continue;
                }
                let cat_dir = dir.join("facts");
                let _ = std::fs::create_dir_all(&cat_dir);
                let file_path = cat_dir.join(format!("{category}.md"));
                let mut file_content = String::new();
                for fact in facts {
                    file_content.push_str(&format!("- {fact}\n"));
                }
                if let Err(e) = std::fs::write(&file_path, &file_content) {
                    log::warn!("failed to migrate facts/{category}.md: {e}");
                }
            }

            // Archive original
            let archive = dir.join("_legacy_facts.md");
            let _ = std::fs::rename(&facts_path, &archive);
            log::info!("migrated {} fact categories for {instance_slug}", category_facts.len());
        }
    }

    // Migrate episodes.md → moments/ folder, one file per episode
    if episodes_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&episodes_path) {
            let moments_dir = dir.join("moments");
            let _ = std::fs::create_dir_all(&moments_dir);

            let mut episode_idx = 0u32;
            let mut lines = content.lines().peekable();
            while let Some(line) = lines.next() {
                let line = line.trim();
                if !line.starts_with("- ") {
                    continue;
                }
                let main = line.trim_start_matches("- ");

                // Parse "(felt: emotion)" suffix
                let (content_part, emotion) = if let Some(felt_pos) = main.rfind("(felt: ") {
                    let before = main[..felt_pos].trim();
                    let after = main[felt_pos + 7..].trim_end_matches(')').trim();
                    (before.to_string(), after.to_string())
                } else {
                    (main.to_string(), String::new())
                };

                // Check for "  why: ..." on next line
                let significance = if let Some(next) = lines.peek() {
                    if next.trim_start().starts_with("why: ") {
                        let sig = next.trim().trim_start_matches("why: ").to_string();
                        lines.next();
                        sig
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Generate a slug from content
                let slug: String = content_part
                    .to_lowercase()
                    .chars()
                    .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
                    .collect::<String>()
                    .split_whitespace()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join("-");
                let slug = if slug.is_empty() {
                    format!("moment-{episode_idx}")
                } else {
                    slug
                };

                let mut file_content = content_part.clone();
                if !emotion.is_empty() {
                    file_content.push_str(&format!("\n\nfelt: {emotion}"));
                }
                if !significance.is_empty() {
                    file_content.push_str(&format!("\nwhy: {significance}"));
                }

                let file_path = moments_dir.join(format!("{slug}.md"));
                if let Err(e) = std::fs::write(&file_path, &file_content) {
                    log::warn!("failed to migrate moment {slug}: {e}");
                }
                episode_idx += 1;
            }

            // Archive original
            let archive = dir.join("_legacy_episodes.md");
            let _ = std::fs::rename(&episodes_path, &archive);
            if episode_idx > 0 {
                log::info!("migrated {episode_idx} episodes to moments/ for {instance_slug}");
            }
        }
    }

    // Clean up memory.db (no longer needed)
    if db_path.exists() {
        let archive = dir.join("_legacy_memory.db");
        let _ = std::fs::rename(&db_path, &archive);
        log::info!("archived memory.db for {instance_slug}");
    }

    // Mark as migrated
    let _ = std::fs::write(&marker, "migrated");
}

/// Extract new memories from recent messages and store them in the library.
/// Called as a background task after each chat turn.
pub async fn extract_and_store(
    workspace_dir: &Path,
    instance_slug: &str,
    recent_messages: &[ChatMessage],
    llm: &LlmBackend,
    vector_store: &super::vector::VectorStore,
    google_ai_key: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dir = memory_dir(workspace_dir, instance_slug);
    std::fs::create_dir_all(&dir)?;

    // Build existing library context
    let entries = scan_library(workspace_dir, instance_slug);
    let file_count = entries.len();
    let existing_summary = if entries.is_empty() {
        String::from("(empty library — no memories yet)")
    } else {
        let mut s = String::new();
        for entry in &entries {
            let full_path = dir.join(&entry.path);
            let content = std::fs::read_to_string(&full_path).unwrap_or_default();
            s.push_str(&format!("[{}]\n{}\n\n", entry.path, content.trim()));
        }
        // Truncate if too long (find a char boundary to avoid panic)
        if s.len() > 4000 {
            let mut end = 4000;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            s.truncate(end);
            s.push_str("\n...(truncated)");
        }
        s
    };

    // Detect image attachments in messages
    let attachment_re = regex::Regex::new(r"\[attached:\s*(.+?)\s*\((\w+)\)\]").unwrap();
    let uploads_dir = workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("uploads");

    let mut image_uploads: Vec<(String, String)> = Vec::new(); // (upload_id, original_name)
    let conversation = recent_messages
        .iter()
        .map(|m| {
            let role = match m.role {
                crate::domain::chat::ChatRole::User => "user",
                crate::domain::chat::ChatRole::Assistant => "assistant",
            };
            // Collect image attachment IDs
            for cap in attachment_re.captures_iter(&m.content) {
                let name = cap[1].to_string();
                let upload_id = cap[2].to_string();
                let meta_path = uploads_dir.join(format!("{upload_id}.json"));
                if let Ok(meta_str) = std::fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                        let mime = meta["mime_type"].as_str().unwrap_or("");
                        if mime.starts_with("image/") {
                            image_uploads.push((upload_id.clone(), name.clone()));
                        }
                    }
                }
            }
            format!("{role}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let image_context = if image_uploads.is_empty() {
        String::new()
    } else {
        let list = image_uploads
            .iter()
            .map(|(id, name)| format!("  - \"{name}\" (upload_id: {id})"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "\nimages shared in this conversation:\n{list}\n\
             you can save important images using: {{\"action\": \"save_image\", \"upload_id\": \"...\", \"path\": \"folder/name.jpg\", \"description\": \"what this image shows\"}}\n\
             only save images that are meaningful — personal photos, important screenshots, etc. NOT memes or random links.\n"
        )
    };

    let extraction_prompt = format!(
        r#"analyze this conversation and decide what to remember.

your memory library currently contains:
{existing_summary}

recent conversation:
{conversation}
{image_context}
respond with JSON — an array of file operations:
{{
  "ops": [
    {{"action": "write", "path": "folder/file.md", "content": "the memory content"}},
    {{"action": "append", "path": "folder/file.md", "content": "additional info to add"}},
    {{"action": "delete", "path": "folder/file.md"}},
    {{"action": "save_image", "upload_id": "upload_xxx", "path": "moments/photo.jpg", "description": "what this image shows"}}
  ]
}}

rules:
- organize memories into folders by topic (e.g. about/, preferences/, moments/, projects/)
- each file should cover one coherent topic or moment
- file names should be descriptive kebab-case (e.g. "about/work.md", "moments/late-night-debugging.md")
- use "write" to create new files or replace outdated ones
- use "append" to add new info to an EXISTING file (prefer this over creating new files)
- use "delete" to remove files with outdated/wrong info
- use "save_image" to save meaningful images shared in the conversation (only if images were shared)
- DON'T duplicate info that's already in the library
- DON'T force it — most conversations produce 0-1 ops
- DON'T create a new file if you can append to an existing one on the same topic
- keep files concise — a few lines each, not essays
- there are currently {file_count} files. aim for quality over quantity — merge related topics

do NOT save images unless they are clearly meaningful (personal photos, important screenshots). ignore memes, random links, UI screenshots."#
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "ops": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["write", "append", "delete", "save_image"]
                        },
                        "path": { "type": "string" },
                        "content": { "type": "string" },
                        "upload_id": { "type": "string" },
                        "description": { "type": "string" }
                    },
                    "required": ["action", "path"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["ops"],
        "additionalProperties": false
    });

    let (response, _) = llm
        .chat_json(
            "you are a memory librarian. you organize memories into a clean file-based library. \
             you understand the difference between facts (knowing something) and moments (shared experiences). \
             you can also save images that are meaningful to the user.",
            &extraction_prompt,
            schema,
        )
        .await?;

    let ops: Vec<MemoryOp> = match serde_json::from_str::<MemoryOps>(&response) {
        Ok(m) => m.ops,
        Err(e) => {
            log::warn!("memory: failed to parse structured output: {e}");
            parse_memory_ops(&response)
        }
    };
    if ops.is_empty() {
        return Ok(());
    }

    for op in &ops {
        // Sanitize path — allow image extensions for save_image
        let clean_path = if op.action == "save_image" {
            sanitize_media_path(&op.path)
        } else {
            sanitize_memory_path(&op.path)
        };
        if clean_path.is_empty() {
            continue;
        }
        let full_path = dir.join(&clean_path);

        match op.action.as_str() {
            "write" => {
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&full_path, &op.content)?;
                log::info!("memory: wrote {clean_path} for {instance_slug}");
                embed_memory_file(vector_store, google_ai_key, instance_slug, &clean_path, &op.content).await;
            }
            "append" => {
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut existing = std::fs::read_to_string(&full_path).unwrap_or_default();
                if !existing.ends_with('\n') && !existing.is_empty() {
                    existing.push('\n');
                }
                existing.push_str(&op.content);
                std::fs::write(&full_path, &existing)?;
                log::info!("memory: appended to {clean_path} for {instance_slug}");
                embed_memory_file(vector_store, google_ai_key, instance_slug, &clean_path, &existing).await;
            }
            "delete" => {
                if full_path.exists() {
                    std::fs::remove_file(&full_path)?;
                    if let Some(parent) = full_path.parent() {
                        let _ = cleanup_empty_dirs(parent, &dir);
                    }
                    log::info!("memory: deleted {clean_path} for {instance_slug}");
                    if let Err(e) = vector_store.delete_by_path(instance_slug, &clean_path).await {
                        log::warn!("memory: vector delete failed for {clean_path}: {e}");
                    }
                }
            }
            "save_image" => {
                let upload_id = if !op.upload_id.is_empty() {
                    &op.upload_id
                } else {
                    log::warn!("memory: save_image — missing upload_id");
                    continue;
                };

                // Find the source file
                let meta_path = uploads_dir.join(format!("{upload_id}.json"));
                let meta_str = match std::fs::read_to_string(&meta_path) {
                    Ok(s) => s,
                    Err(_) => {
                        log::warn!("memory: save_image — upload {upload_id} not found");
                        continue;
                    }
                };
                let meta: serde_json::Value = match serde_json::from_str(&meta_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let stored_name = match meta["stored_name"].as_str() {
                    Some(s) => s,
                    None => continue,
                };
                let mime_type = meta["mime_type"].as_str().unwrap_or("image/jpeg");

                let src = uploads_dir.join(stored_name);
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::copy(&src, &full_path) {
                    log::warn!("memory: save_image copy failed: {e}");
                    continue;
                }
                log::info!("memory: saved image {clean_path} for {instance_slug}");

                // Embed the image
                let desc = if op.description.is_empty() { &clean_path } else { &op.description };
                if let Ok(bytes) = std::fs::read(&full_path) {
                    if bytes.len() < 20 * 1024 * 1024 {
                        if let Ok(vec) = super::embedding::embed_text_and_image(google_ai_key, desc, &bytes, mime_type).await {
                            if let Err(e) = vector_store.upsert_media(
                                instance_slug, &clean_path, "media_image",
                                mime_type, &clean_path, desc, vec,
                            ).await {
                                log::warn!("memory: image vector upsert failed: {e}");
                            }
                        }
                    }
                }
            }
            _ => {
                log::warn!("memory: unknown action '{}' for {instance_slug}", op.action);
            }
        }
    }

    Ok(())
}

/// Embed a memory file into the vector store (background-safe, logs errors).
async fn embed_memory_file(
    vector_store: &super::vector::VectorStore,
    google_ai_key: &str,
    instance_slug: &str,
    path: &str,
    content: &str,
) {
    use super::{embedding, vector};

    let chunks = vector::chunk_text(content);
    let mut chunk_vectors = Vec::new();

    for chunk in &chunks {
        match embedding::embed_text(google_ai_key, chunk, embedding::TaskType::RetrievalDocument).await {
            Ok(vec) => chunk_vectors.push((chunk.clone(), vec)),
            Err(e) => {
                log::warn!("[memory] embed error for {path}: {e}");
                return;
            }
        }
    }

    if let Err(e) = vector_store.upsert_text_memory(instance_slug, path, chunk_vectors).await {
        log::warn!("[memory] vector upsert failed for {path}: {e}");
    }
}

/// Remove empty directories up to (but not including) the base memory dir.
pub fn cleanup_empty_dirs(dir: &Path, base: &Path) -> std::io::Result<()> {
    if dir == base || !dir.starts_with(base) {
        return Ok(());
    }
    if dir.is_dir() {
        let is_empty = std::fs::read_dir(dir)?.next().is_none();
        if is_empty {
            std::fs::remove_dir(dir)?;
            if let Some(parent) = dir.parent() {
                cleanup_empty_dirs(parent, base)?;
            }
        }
    }
    Ok(())
}

/// Sanitize a memory file path to prevent directory traversal.
fn sanitize_memory_path(path: &str) -> String {
    let path = path.trim().trim_start_matches('/');
    // Reject any path component that is ".." or starts with "."
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == ".." || p.starts_with('.')) {
        return String::new();
    }
    // Ensure .md extension
    let result = parts.join("/");
    if !result.ends_with(".md") {
        format!("{result}.md")
    } else {
        result
    }
}

/// Sanitize path for image/media files — allows image extensions.
fn sanitize_media_path(path: &str) -> String {
    let path = path.trim().trim_start_matches('/');
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == ".." || p.starts_with('.')) {
        return String::new();
    }
    let result = parts.join("/");
    let lower = result.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png")
        || lower.ends_with(".webp") || lower.ends_with(".gif")
        || lower.ends_with(".mp4") || lower.ends_with(".mp3") || lower.ends_with(".wav") {
        result
    } else {
        format!("{result}.jpg")
    }
}

/// Remove memory files matching a query. Returns the number removed.
pub fn forget_memories(workspace_dir: &Path, instance_slug: &str, query: &str) -> usize {
    let dir = memory_dir(workspace_dir, instance_slug);
    let entries = scan_library(workspace_dir, instance_slug);
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    let mut removed = 0;

    for entry in entries {
        let full_path = dir.join(&entry.path);
        let content = std::fs::read_to_string(&full_path).unwrap_or_default();
        let combined = format!("{} {}", entry.path, content).to_lowercase();

        if query_words.iter().any(|w| combined.contains(*w)) {
            if std::fs::remove_file(&full_path).is_ok() {
                removed += 1;
                if let Some(parent) = full_path.parent() {
                    let _ = cleanup_empty_dirs(parent, &dir);
                }
            }
        }
    }

    removed
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct MemoryOp {
    action: String,
    path: String,
    #[serde(default)]
    content: String,
    /// Upload ID for save_image action.
    #[serde(default)]
    upload_id: String,
    /// Description for save_image action.
    #[serde(default)]
    description: String,
}

#[derive(serde::Deserialize)]
struct MemoryOps {
    #[serde(default)]
    ops: Vec<MemoryOp>,
}

fn parse_memory_ops(response: &str) -> Vec<MemoryOp> {
    // Try direct parse
    if let Ok(m) = serde_json::from_str::<MemoryOps>(response) {
        return m.ops;
    }

    // Try stripping markdown code fences
    let trimmed = response.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    if let Ok(m) = serde_json::from_str::<MemoryOps>(json_str) {
        return m.ops;
    }

    Vec::new()
}

/// Consolidate similar memories by merging them with LLM help.
/// Finds pairs of memories with high vector similarity and asks the LLM to merge them.
pub async fn consolidate_similar(
    workspace_dir: &Path,
    instance_slug: &str,
    llm: &LlmBackend,
    vector_store: &super::vector::VectorStore,
    google_ai_key: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let dir = memory_dir(workspace_dir, instance_slug);
    let pairs = vector_store
        .find_similar_pairs(instance_slug, 0.85)
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

    if pairs.is_empty() {
        return Ok(0);
    }

    // Group pairs into clusters (simple: take first N pairs to avoid over-merging)
    let mut merged_count = 0;
    let mut already_merged: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (path_a, path_b, score) in pairs.iter().take(10) {
        if already_merged.contains(path_a) || already_merged.contains(path_b) {
            continue;
        }

        let file_a = dir.join(path_a);
        let file_b = dir.join(path_b);

        let content_a = match std::fs::read_to_string(&file_a) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let content_b = match std::fs::read_to_string(&file_b) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Ask LLM to merge
        let merge_prompt = format!(
            "merge these two similar memory files into one concise file.\n\n\
             file 1 ({path_a}):\n{content_a}\n\n\
             file 2 ({path_b}):\n{content_b}\n\n\
             similarity score: {score:.2}\n\n\
             respond with ONLY the merged content — no explanations, no file path."
        );

        let (merged_content, _) = llm
            .chat(
                "you are a memory librarian. merge the two files into one, keeping all important info. be concise.",
                &merge_prompt,
                vec![],
            )
            .await?;

        // Write merged content to the first file, delete the second
        std::fs::write(&file_a, merged_content.trim())?;
        if file_b.exists() {
            std::fs::remove_file(&file_b)?;
            if let Some(parent) = file_b.parent() {
                let _ = cleanup_empty_dirs(parent, &dir);
            }
        }

        // Update vectors: embed merged file, delete old vectors
        embed_memory_file(vector_store, google_ai_key, instance_slug, path_a, merged_content.trim()).await;
        if let Err(e) = vector_store.delete_by_path(instance_slug, path_b).await {
            log::warn!("[consolidate] vector delete failed for {path_b}: {e}");
        }

        already_merged.insert(path_a.clone());
        already_merged.insert(path_b.clone());
        merged_count += 1;

        log::info!("[consolidate] merged {path_a} + {path_b} (similarity: {score:.2})");
    }

    Ok(merged_count)
}
