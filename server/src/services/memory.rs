use std::path::Path;

use rig::embeddings::EmbeddingsBuilder;
use rig::providers::openai;
use rig_sqlite::{SqliteVectorIndex, SqliteVectorStore};
use rusqlite::ffi::{sqlite3, sqlite3_api_routines, sqlite3_auto_extension};
use sqlite_vec::sqlite3_vec_init;
use std::sync::Once;
use tokio_rusqlite::Connection;

use crate::domain::chat::ChatMessage;
use crate::domain::memory::{MemoryEpisode, MemoryFact};
use crate::services::llm::LlmBackend;

type SqliteExtensionFn =
    unsafe extern "C" fn(*mut sqlite3, *mut *mut std::ffi::c_char, *const sqlite3_api_routines) -> i32;

static SQLITE_VEC_INIT: Once = Once::new();

fn ensure_sqlite_vec() {
    SQLITE_VEC_INIT.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute::<*const (), SqliteExtensionFn>(
            sqlite3_vec_init as *const (),
        )));
    });
}

fn memory_dir(workspace_dir: &Path, instance_slug: &str) -> std::path::PathBuf {
    workspace_dir
        .join("instances")
        .join(instance_slug)
        .join("memory")
}

async fn open_db(workspace_dir: &Path, instance_slug: &str) -> Result<Connection, Box<dyn std::error::Error + Send + Sync>> {
    ensure_sqlite_vec();
    let dir = memory_dir(workspace_dir, instance_slug);
    std::fs::create_dir_all(&dir)?;
    let db_path = dir.join("memory.db");
    let conn = Connection::open(db_path).await?;
    Ok(conn)
}

/// Create a vector store index for dynamic_context RAG (facts).
/// Returns None if no embedding model is available or DB doesn't exist.
pub async fn build_memory_index(
    workspace_dir: &Path,
    instance_slug: &str,
    embedding_model: &openai::EmbeddingModel,
) -> Option<SqliteVectorIndex<openai::EmbeddingModel, MemoryFact>> {
    let db_path = memory_dir(workspace_dir, instance_slug).join("memory.db");
    if !db_path.exists() {
        return None;
    }

    let conn = open_db(workspace_dir, instance_slug).await.ok()?;
    let store = SqliteVectorStore::<openai::EmbeddingModel, MemoryFact>::new(conn, embedding_model)
        .await
        .ok()?;
    Some(store.index(embedding_model.clone()))
}

/// Build the full memory prompt (facts + episodes) for when no embedding model is available.
pub fn build_facts_md_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let facts = retrieve_from_facts_md(workspace_dir, instance_slug);
    let episodes = retrieve_from_episodes_md(workspace_dir, instance_slug);
    build_memory_prompt(&facts, &episodes)
}

/// Build just the episodes prompt for injection alongside RAG facts.
pub fn build_episodes_prompt(workspace_dir: &Path, instance_slug: &str) -> String {
    let episodes = retrieve_from_episodes_md(workspace_dir, instance_slug);
    if episodes.is_empty() {
        return String::new();
    }
    let mut prompt = String::from("\n\nmoments we've shared:\n");
    for ep in &episodes {
        prompt.push_str(&format!("- {} (felt: {})\n", ep.content, ep.emotion));
    }
    prompt.push_str("\nthese are your real memories of time spent together. reference them naturally when relevant — don't list them, just *know* them.");
    prompt
}

fn retrieve_from_facts_md(workspace_dir: &Path, instance_slug: &str) -> Vec<MemoryFact> {
    let facts_path = memory_dir(workspace_dir, instance_slug).join("facts.md");
    let content = match std::fs::read_to_string(&facts_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter(|line| line.starts_with("- "))
        .map(|line| MemoryFact {
            id: String::new(),
            content: line.trim_start_matches("- ").to_string(),
            category: String::new(),
            created_at: String::new(),
        })
        .collect()
}

fn retrieve_from_episodes_md(workspace_dir: &Path, instance_slug: &str) -> Vec<MemoryEpisode> {
    let path = memory_dir(workspace_dir, instance_slug).join("episodes.md");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Parse episodes: each block is "- content (felt: emotion)\n  why: significance"
    let mut episodes = Vec::new();
    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.starts_with("- ") {
            continue;
        }
        let main = line.trim_start_matches("- ");
        // Extract emotion from "(felt: ...)" suffix
        let (content_part, emotion) = if let Some(felt_pos) = main.rfind("(felt: ") {
            let before = main[..felt_pos].trim();
            let after = main[felt_pos + 7..].trim_end_matches(')').trim();
            (before.to_string(), after.to_string())
        } else {
            (main.to_string(), String::new())
        };

        // Check for "  why: ..." on next line
        let significance = if let Some(next) = lines.peek() {
            if next.starts_with("  why: ") {
                let sig = next.trim_start_matches("  why: ").to_string();
                lines.next();
                sig
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        episodes.push(MemoryEpisode {
            id: String::new(),
            content: content_part,
            emotion,
            significance,
            created_at: String::new(),
        });
    }
    episodes
}

fn build_memory_prompt(facts: &[MemoryFact], episodes: &[MemoryEpisode]) -> String {
    if facts.is_empty() && episodes.is_empty() {
        return String::new();
    }

    let mut prompt = String::from("## memory\nyou have persistent memory across conversations. you quietly remember things the user tells you.\n");

    if !facts.is_empty() {
        prompt.push_str("\nwhat you know about me:\n");
        for fact in facts {
            prompt.push_str(&format!("- {}\n", fact.content));
        }
    }

    if !episodes.is_empty() {
        prompt.push_str("\nmoments we've shared:\n");
        for ep in episodes {
            prompt.push_str(&format!("- {} (felt: {})\n", ep.content, ep.emotion));
            if !ep.significance.is_empty() {
                prompt.push_str(&format!("  why: {}\n", ep.significance));
            }
        }
    }

    prompt.push_str("\nuse these memories naturally. don't announce that you remember — just know. \
        reference shared moments when they're relevant — they're what makes your bond real.");
    prompt
}

/// Extract new facts AND episodes from recent messages, embed and store them.
/// Called as a background task after each chat turn.
pub async fn extract_and_store(
    workspace_dir: &Path,
    instance_slug: &str,
    recent_messages: &[ChatMessage],
    llm: &LlmBackend,
    embedding_model: Option<&openai::EmbeddingModel>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load existing for dedup context
    let existing_facts = load_all_facts_from_db_or_md(workspace_dir, instance_slug).await;
    let existing_episodes = load_all_episodes_from_db_or_md(workspace_dir, instance_slug).await;

    let existing_facts_summary = if existing_facts.is_empty() {
        String::from("(none yet)")
    } else {
        existing_facts
            .iter()
            .map(|f| format!("- {}", f.content))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let existing_episodes_summary = if existing_episodes.is_empty() {
        String::from("(none yet)")
    } else {
        existing_episodes
            .iter()
            .map(|e| format!("- {} (felt: {})", e.content, e.emotion))
            .collect::<Vec<_>>()
            .join("\n")
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
        r#"analyze this conversation and extract two types of memories:

1. FACTS — things you learned about the user (preferences, personal details, projects, goals)
2. EPISODES — meaningful moments that happened between you. not every message is an episode. episodes are moments worth remembering: when they shared something personal, when they were excited or frustrated, when something shifted in the conversation, when you worked through something together.

existing facts:
{existing_facts_summary}

existing episodes:
{existing_episodes_summary}

recent conversation:
{conversation}

respond with JSON:
{{
  "facts": [
    {{"content": "the fact", "category": "personal|preference|project|opinion|goal|routine"}}
  ],
  "episodes": [
    {{"content": "what happened — written as a vivid memory, not a dry summary", "emotion": "the emotional tone (e.g. excited, vulnerable, proud, frustrated, playful)", "significance": "why this moment matters — what it revealed or changed"}}
  ]
}}

only include genuinely NEW information not already in existing memories. most conversations have 0-1 episodes. don't force it — only extract episodes for moments that actually matter. facts can be more frequent.

respond ONLY with the JSON object, no other text."#
    );

    let response = llm
        .chat(
            "you are a memory extraction assistant. you extract both facts and meaningful episodic memories from conversations. you understand the difference between knowing something about someone (fact) and remembering a moment with them (episode). respond only with valid JSON.",
            &extraction_prompt,
            vec![],
        )
        .await?;

    let extracted = parse_extracted_memories(&response);

    let has_facts = !extracted.facts.is_empty();
    let has_episodes = !extracted.episodes.is_empty();

    if !has_facts && !has_episodes {
        return Ok(());
    }

    // Store facts
    if has_facts {
        let new_facts: Vec<MemoryFact> = extracted
            .facts
            .into_iter()
            .map(|f| MemoryFact {
                id: uuid::Uuid::new_v4().to_string(),
                content: f.content,
                category: f.category,
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .collect();

        if let Some(model) = embedding_model {
            store_facts_with_embeddings(workspace_dir, instance_slug, &new_facts, model).await?;
        }

        let all_facts = if embedding_model.is_some() {
            load_all_facts_from_db(workspace_dir, instance_slug).await.unwrap_or_default()
        } else {
            let mut all = load_all_facts_from_db_or_md(workspace_dir, instance_slug).await;
            all.extend(new_facts);
            all
        };
        regenerate_facts_md(workspace_dir, instance_slug, &all_facts);
    }

    // Store episodes
    if has_episodes {
        let new_episodes: Vec<MemoryEpisode> = extracted
            .episodes
            .into_iter()
            .map(|e| MemoryEpisode {
                id: uuid::Uuid::new_v4().to_string(),
                content: e.content,
                emotion: e.emotion,
                significance: e.significance,
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .collect();

        if let Some(model) = embedding_model {
            store_episodes_with_embeddings(workspace_dir, instance_slug, &new_episodes, model).await?;
        }

        let all_episodes = if embedding_model.is_some() {
            load_all_episodes_from_db(workspace_dir, instance_slug).await.unwrap_or_default()
        } else {
            let mut all = load_all_episodes_from_db_or_md(workspace_dir, instance_slug).await;
            all.extend(new_episodes);
            all
        };
        regenerate_episodes_md(workspace_dir, instance_slug, &all_episodes);

        log::info!(
            "extracted {} new episode(s) for {instance_slug}",
            all_episodes.len() - existing_episodes.len()
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

async fn store_facts_with_embeddings(
    workspace_dir: &Path,
    instance_slug: &str,
    facts: &[MemoryFact],
    model: &openai::EmbeddingModel,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = open_db(workspace_dir, instance_slug).await?;
    let store = SqliteVectorStore::<openai::EmbeddingModel, MemoryFact>::new(conn, model).await?;

    let embeddings = EmbeddingsBuilder::new(model.clone())
        .documents(facts.to_vec())?
        .build()
        .await?;

    store.add_rows(embeddings).await?;
    Ok(())
}

async fn store_episodes_with_embeddings(
    workspace_dir: &Path,
    instance_slug: &str,
    episodes: &[MemoryEpisode],
    model: &openai::EmbeddingModel,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = open_db(workspace_dir, instance_slug).await?;
    let store = SqliteVectorStore::<openai::EmbeddingModel, MemoryEpisode>::new(conn, model).await?;

    let embeddings = EmbeddingsBuilder::new(model.clone())
        .documents(episodes.to_vec())?
        .build()
        .await?;

    store.add_rows(embeddings).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

async fn load_all_facts_from_db(
    workspace_dir: &Path,
    instance_slug: &str,
) -> Result<Vec<MemoryFact>, Box<dyn std::error::Error + Send + Sync>> {
    let dir = memory_dir(workspace_dir, instance_slug);
    let db_path = dir.join("memory.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    ensure_sqlite_vec();
    let conn = Connection::open(db_path).await?;
    let facts = conn
        .call(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, content, category, created_at FROM memory_facts ORDER BY created_at",
            )?;
            let facts = stmt
                .query_map([], |row| {
                    Ok(MemoryFact {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        category: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                })?
                .filter_map(Result::ok)
                .collect::<Vec<_>>();
            Ok(facts)
        })
        .await?;

    Ok(facts)
}

async fn load_all_episodes_from_db(
    workspace_dir: &Path,
    instance_slug: &str,
) -> Result<Vec<MemoryEpisode>, Box<dyn std::error::Error + Send + Sync>> {
    let dir = memory_dir(workspace_dir, instance_slug);
    let db_path = dir.join("memory.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    ensure_sqlite_vec();
    let conn = Connection::open(db_path).await?;

    // Table may not exist yet if no episodes have been stored
    let episodes = conn
        .call(|conn| {
            // Check if table exists
            let table_exists: bool = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='memory_episodes'")?
                .query_map([], |_| Ok(true))?
                .next()
                .is_some();

            if !table_exists {
                return Ok(Vec::new());
            }

            let mut stmt = conn.prepare(
                "SELECT id, content, emotion, significance, created_at FROM memory_episodes ORDER BY created_at",
            )?;
            let episodes = stmt
                .query_map([], |row| {
                    Ok(MemoryEpisode {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        emotion: row.get(2)?,
                        significance: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .filter_map(Result::ok)
                .collect::<Vec<_>>();
            Ok(episodes)
        })
        .await?;

    Ok(episodes)
}

async fn load_all_facts_from_db_or_md(
    workspace_dir: &Path,
    instance_slug: &str,
) -> Vec<MemoryFact> {
    match load_all_facts_from_db(workspace_dir, instance_slug).await {
        Ok(facts) if !facts.is_empty() => facts,
        _ => retrieve_from_facts_md(workspace_dir, instance_slug),
    }
}

async fn load_all_episodes_from_db_or_md(
    workspace_dir: &Path,
    instance_slug: &str,
) -> Vec<MemoryEpisode> {
    match load_all_episodes_from_db(workspace_dir, instance_slug).await {
        Ok(episodes) if !episodes.is_empty() => episodes,
        _ => retrieve_from_episodes_md(workspace_dir, instance_slug),
    }
}

// ---------------------------------------------------------------------------
// Markdown generation
// ---------------------------------------------------------------------------

fn regenerate_facts_md(workspace_dir: &Path, instance_slug: &str, facts: &[MemoryFact]) {
    let dir = memory_dir(workspace_dir, instance_slug);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("facts.md");

    let categories = [
        "personal",
        "preference",
        "project",
        "opinion",
        "goal",
        "routine",
    ];

    let mut content = String::from("# memories\n\n");

    for category in &categories {
        let cat_facts: Vec<_> = facts.iter().filter(|f| f.category == *category).collect();
        if cat_facts.is_empty() {
            continue;
        }
        content.push_str(&format!("## {category}\n"));
        for fact in cat_facts {
            content.push_str(&format!("- {}\n", fact.content));
        }
        content.push('\n');
    }

    let other_facts: Vec<_> = facts
        .iter()
        .filter(|f| !categories.contains(&f.category.as_str()))
        .collect();
    if !other_facts.is_empty() {
        content.push_str("## other\n");
        for fact in other_facts {
            content.push_str(&format!("- {}\n", fact.content));
        }
        content.push('\n');
    }

    if let Err(e) = std::fs::write(&path, content) {
        log::warn!("failed to write facts.md: {e}");
    }
}

fn regenerate_episodes_md(workspace_dir: &Path, instance_slug: &str, episodes: &[MemoryEpisode]) {
    let dir = memory_dir(workspace_dir, instance_slug);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("episodes.md");

    let mut content = String::from("# moments\n\n");

    for ep in episodes {
        content.push_str(&format!("- {} (felt: {})\n", ep.content, ep.emotion));
        if !ep.significance.is_empty() {
            content.push_str(&format!("  why: {}\n", ep.significance));
        }
    }

    if let Err(e) = std::fs::write(&path, content) {
        log::warn!("failed to write episodes.md: {e}");
    }
}

/// Load episodes.md content for heartbeat context (truncated).
pub fn load_episodes_for_heartbeat(workspace_dir: &Path, instance_slug: &str) -> String {
    let path = memory_dir(workspace_dir, instance_slug).join("episodes.md");
    match std::fs::read_to_string(&path) {
        Ok(c) if !c.trim().is_empty() => {
            let truncated: String = c.chars().take(1500).collect();
            truncated
        }
        _ => String::new(),
    }
}

/// Search episodes by keyword (for the recall tool).
pub fn search_episodes(workspace_dir: &Path, instance_slug: &str, query: &str) -> Vec<MemoryEpisode> {
    let episodes = retrieve_from_episodes_md(workspace_dir, instance_slug);
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    episodes
        .into_iter()
        .filter(|ep| {
            let combined = format!("{} {} {}", ep.content, ep.emotion, ep.significance).to_lowercase();
            query_words.iter().any(|w| combined.contains(w))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct ExtractedFact {
    content: String,
    category: String,
}

#[derive(serde::Deserialize)]
struct ExtractedEpisode {
    content: String,
    emotion: String,
    #[serde(default)]
    significance: String,
}

#[derive(serde::Deserialize)]
struct ExtractedMemories {
    #[serde(default)]
    facts: Vec<ExtractedFact>,
    #[serde(default)]
    episodes: Vec<ExtractedEpisode>,
}

fn parse_extracted_memories(response: &str) -> ExtractedMemories {
    // Try direct parse
    if let Ok(m) = serde_json::from_str::<ExtractedMemories>(response) {
        return m;
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

    if let Ok(m) = serde_json::from_str::<ExtractedMemories>(json_str) {
        return m;
    }

    // Fallback: try parsing as old-style fact-only array
    let facts: Vec<ExtractedFact> = serde_json::from_str(json_str).unwrap_or_default();
    ExtractedMemories {
        facts,
        episodes: Vec::new(),
    }
}
