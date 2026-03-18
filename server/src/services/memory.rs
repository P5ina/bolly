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
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

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

    let mut prompt = format!(
        "## memory\nyou have {} memories:\n",
        entries.len()
    );
    for entry in &entries {
        prompt.push_str(&format!("- {}\n", entry.path));
    }

    let path = catalog_snapshot_path(workspace_dir, instance_slug);
    if let Err(e) = std::fs::write(&path, &prompt) {
        log::warn!("[memory] failed to write catalog snapshot: {e}");
    } else {
        log::info!("[memory] catalog snapshot rebuilt: {} files", entries.len());
    }
}

/// Load the static memory catalog snapshot from disk.
/// Returns empty string if no snapshot exists yet (first boot / pre-compaction).
pub fn load_catalog_snapshot(workspace_dir: &Path, instance_slug: &str) -> String {
    let path = catalog_snapshot_path(workspace_dir, instance_slug);
    std::fs::read_to_string(&path).unwrap_or_default()
}

/// Build the memory prompt for the system prompt.
/// For small libraries (< 6000 chars total), inline all file contents.
/// For larger ones, show a catalog with summaries + inline the smallest files.
/// NOTE: this scans disk on every call — use load_catalog_snapshot() for the static version.
pub fn build_memory_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let entries = scan_library(workspace_dir, instance_slug);
    if entries.is_empty() {
        return String::new();
    }

    let mut prompt = String::from(
        "## memory\nyou have a personal memory library. use `recall` to read memories when relevant.\n\
         catalog ({} files):\n",
    )
    .replace("{}", &entries.len().to_string());

    for entry in &entries {
        prompt.push_str(&format!("- {} — {}\n", entry.path, entry.summary));
    }

    prompt.push_str(
        "\nuse these memories naturally — `recall` what you need. \
         don't announce that you remember — just know.",
    );
    prompt
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

    let conversation = recent_messages
        .iter()
        .map(|m| {
            let role = match m.role {
                crate::domain::chat::ChatRole::User => "user",
                crate::domain::chat::ChatRole::Assistant => "assistant",
            };
            format!("{role}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let extraction_prompt = format!(
        r#"analyze this conversation and decide what to remember.

your memory library currently contains:
{existing_summary}

recent conversation:
{conversation}

respond with JSON — an array of file operations:
{{
  "ops": [
    {{"action": "write", "path": "folder/file.md", "content": "the memory content"}},
    {{"action": "append", "path": "folder/file.md", "content": "additional info to add"}},
    {{"action": "delete", "path": "folder/file.md"}}
  ]
}}

rules:
- organize memories into folders by topic (e.g. about/, preferences/, moments/, projects/)
- each file should cover one coherent topic or moment
- file names should be descriptive kebab-case (e.g. "about/work.md", "moments/late-night-debugging.md")
- use "write" to create new files or replace outdated ones
- use "append" to add new info to an EXISTING file (prefer this over creating new files)
- use "delete" to remove files with outdated/wrong info
- DON'T duplicate info that's already in the library
- DON'T force it — most conversations produce 0-1 ops
- DON'T create a new file if you can append to an existing one on the same topic
- keep files concise — a few lines each, not essays
- there are currently {file_count} files. aim for quality over quantity — merge related topics

respond ONLY with the JSON object, no other text."#
    );

    let (response, _) = llm
        .chat(
            "you are a memory librarian. you organize memories into a clean file-based library. \
             you understand the difference between facts (knowing something) and moments (shared experiences). \
             respond only with valid JSON.",
            &extraction_prompt,
            vec![],
        )
        .await?;

    let ops = parse_memory_ops(&response);
    if ops.is_empty() {
        return Ok(());
    }

    for op in &ops {
        // Sanitize path — prevent directory traversal
        let clean_path = sanitize_memory_path(&op.path);
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
            }
            "delete" => {
                if full_path.exists() {
                    std::fs::remove_file(&full_path)?;
                    // Clean up empty parent dirs
                    if let Some(parent) = full_path.parent() {
                        let _ = cleanup_empty_dirs(parent, &dir);
                    }
                    log::info!("memory: deleted {clean_path} for {instance_slug}");
                }
            }
            _ => {
                log::warn!("memory: unknown action '{}' for {instance_slug}", op.action);
            }
        }
    }

    Ok(())
}

/// Remove empty directories up to (but not including) the base memory dir.
fn cleanup_empty_dirs(dir: &Path, base: &Path) -> std::io::Result<()> {
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
// BM25 search
// ---------------------------------------------------------------------------

/// A search result with path, matched chunk text, and relevance score.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    /// File path (e.g. "about/work.md"). May have #chunkN suffix for large files.
    pub path: String,
    /// The matched text chunk.
    pub text: String,
    /// BM25 relevance score (higher = better).
    pub score: f64,
}

/// Search the memory library using BM25 scoring with chunking for large files.
pub fn search(workspace_dir: &Path, instance_slug: &str, query: &str, limit: usize) -> Vec<SearchResult> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let terms: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| w.len() > 1)
        .collect();

    if terms.is_empty() {
        return Vec::new();
    }

    let dir = memory_dir(workspace_dir, instance_slug);
    let entries = scan_library(workspace_dir, instance_slug);
    if entries.is_empty() {
        return Vec::new();
    }

    // Build chunks from all files
    let mut all_chunks: Vec<(String, String)> = Vec::new(); // (path, text)
    for entry in &entries {
        let full_path = dir.join(&entry.path);
        let content = std::fs::read_to_string(&full_path).unwrap_or_default();

        if content.len() < 800 {
            all_chunks.push((entry.path.clone(), content));
        } else {
            let paragraphs: Vec<&str> = content.split("\n\n").collect();
            let mut current = String::new();
            let mut idx = 0;
            for para in paragraphs {
                current.push_str(para);
                current.push_str("\n\n");
                if current.len() >= 600 {
                    all_chunks.push((format!("{}#chunk{}", entry.path, idx), current.clone()));
                    current.clear();
                    idx += 1;
                }
            }
            if !current.trim().is_empty() {
                all_chunks.push((format!("{}#chunk{}", entry.path, idx), current));
            }
        }
    }

    let total = all_chunks.len() as f64;

    // Document frequency per term
    let mut df: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (_, text) in &all_chunks {
        let lower = text.to_lowercase();
        for term in &terms {
            if lower.contains(term) {
                *df.entry(term).or_insert(0) += 1;
            }
        }
    }

    // BM25 scoring
    let k1 = 1.5f64;
    let b = 0.75f64;
    let avg_len = all_chunks.iter().map(|(_, t)| t.len() as f64).sum::<f64>() / total.max(1.0);

    let mut results: Vec<SearchResult> = Vec::new();

    for (path, text) in &all_chunks {
        let lower = text.to_lowercase();
        let doc_len = text.len() as f64;
        let mut score = 0.0f64;

        for term in &terms {
            let tf = lower.matches(term).count() as f64;
            if tf == 0.0 { continue; }
            let n = *df.get(term).unwrap_or(&0) as f64;
            let idf = ((total - n + 0.5) / (n + 0.5) + 1.0).ln();
            let tf_norm = (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * doc_len / avg_len));
            score += idf * tf_norm;
        }

        // Path bonus
        let path_lower = path.to_lowercase();
        for term in &terms {
            if path_lower.contains(term) {
                score += 2.0;
            }
        }

        if score > 0.0 {
            results.push(SearchResult {
                path: path.clone(),
                text: text.clone(),
                score,
            });
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    results
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
