use std::path::Path;

use rig::embeddings::EmbeddingsBuilder;
use rig::providers::openai;
use rig::vector_store::request::VectorSearchRequest;
use rig::vector_store::VectorStoreIndex;
use rig_sqlite::SqliteVectorStore;
use rusqlite::ffi::{sqlite3, sqlite3_api_routines, sqlite3_auto_extension};
use sqlite_vec::sqlite3_vec_init;
use std::sync::Once;
use tokio_rusqlite::Connection;

use crate::domain::chat::ChatMessage;
use crate::domain::memory::MemoryFact;
use crate::services::llm::LlmBackend;

type SqliteExtensionFn =
    unsafe extern "C" fn(*mut sqlite3, *mut *mut i8, *const sqlite3_api_routines) -> i32;

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

/// Retrieve relevant memories and format them as a prompt section.
/// If an embedding model is available, uses semantic search.
/// Otherwise falls back to reading all facts from facts.md.
pub async fn retrieve_and_format(
    workspace_dir: &Path,
    instance_slug: &str,
    query: &str,
    embedding_model: Option<&openai::EmbeddingModel>,
) -> String {
    let facts = if let Some(model) = embedding_model {
        match retrieve_semantic(workspace_dir, instance_slug, query, model).await {
            Ok(facts) if !facts.is_empty() => facts,
            Ok(_) => retrieve_from_facts_md(workspace_dir, instance_slug),
            Err(e) => {
                log::warn!("semantic retrieval failed, falling back to facts.md: {e}");
                retrieve_from_facts_md(workspace_dir, instance_slug)
            }
        }
    } else {
        retrieve_from_facts_md(workspace_dir, instance_slug)
    };

    build_memory_prompt(&facts)
}

async fn retrieve_semantic(
    workspace_dir: &Path,
    instance_slug: &str,
    query: &str,
    model: &openai::EmbeddingModel,
) -> Result<Vec<MemoryFact>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = open_db(workspace_dir, instance_slug).await?;
    let store = SqliteVectorStore::<openai::EmbeddingModel, MemoryFact>::new(conn, model).await?;
    let index = store.index(model.clone());

    let req = VectorSearchRequest::builder()
        .query(query)
        .samples(8)
        .build()?;

    let results: Vec<(f64, String, MemoryFact)> = index.top_n(req).await?;
    Ok(results.into_iter().map(|(_, _, fact)| fact).collect())
}

fn retrieve_from_facts_md(workspace_dir: &Path, instance_slug: &str) -> Vec<MemoryFact> {
    let facts_path = memory_dir(workspace_dir, instance_slug).join("facts.md");
    let content = match std::fs::read_to_string(&facts_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Parse bullet points from facts.md as simple MemoryFact entries
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

fn build_memory_prompt(facts: &[MemoryFact]) -> String {
    let mut prompt = String::from("## memory\nyou have persistent memory across conversations. you quietly remember things the user tells you.\n");
    if !facts.is_empty() {
        prompt.push_str("\nwhat you know about me:\n");
        for fact in facts {
            prompt.push_str(&format!("- {}\n", fact.content));
        }
    }
    prompt.push_str("\nuse these memories naturally. don't announce that you remember — just know.");
    prompt
}

/// Extract new facts from recent messages, embed and store them.
/// Called as a background task after each chat turn.
pub async fn extract_and_store(
    workspace_dir: &Path,
    instance_slug: &str,
    recent_messages: &[ChatMessage],
    llm: &LlmBackend,
    embedding_model: Option<&openai::EmbeddingModel>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load existing facts for dedup context
    let existing_facts = load_all_facts_from_db_or_md(workspace_dir, instance_slug).await;

    let existing_summary = if existing_facts.is_empty() {
        String::from("(none yet)")
    } else {
        existing_facts
            .iter()
            .map(|f| format!("- {}", f.content))
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
        r#"extract any new facts about the user from this conversation. only extract facts that are NOT already known.

existing facts:
{existing_summary}

recent conversation:
{conversation}

respond with a JSON array of objects, each with "content" (the fact) and "category" (one of: personal, preference, project, opinion, goal, routine). only include genuinely new information. if nothing new, respond with an empty array [].

respond ONLY with the JSON array, no other text."#
    );

    let response = llm
        .chat(
            "you are a fact extraction assistant. you extract personal facts about the user from conversations. respond only with valid JSON.",
            &extraction_prompt,
            vec![],
        )
        .await?;

    let extracted: Vec<ExtractedFact> = parse_extracted_facts(&response);

    if extracted.is_empty() {
        return Ok(());
    }

    let new_facts: Vec<MemoryFact> = extracted
        .into_iter()
        .map(|f| MemoryFact {
            id: uuid::Uuid::new_v4().to_string(),
            content: f.content,
            category: f.category,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .collect();

    if let Some(model) = embedding_model {
        store_with_embeddings(workspace_dir, instance_slug, &new_facts, model).await?;
    }

    // Always regenerate facts.md (works with or without embeddings)
    let all_facts = if embedding_model.is_some() {
        load_all_facts_from_db(workspace_dir, instance_slug).await.unwrap_or_default()
    } else {
        // Without embeddings, append to existing parsed facts
        let mut all = load_all_facts_from_db_or_md(workspace_dir, instance_slug).await;
        all.extend(new_facts);
        all
    };

    regenerate_facts_md(workspace_dir, instance_slug, &all_facts);

    Ok(())
}

async fn store_with_embeddings(
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

async fn load_all_facts_from_db_or_md(
    workspace_dir: &Path,
    instance_slug: &str,
) -> Vec<MemoryFact> {
    match load_all_facts_from_db(workspace_dir, instance_slug).await {
        Ok(facts) if !facts.is_empty() => facts,
        _ => retrieve_from_facts_md(workspace_dir, instance_slug),
    }
}

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

    // Any facts with unrecognized categories
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

#[derive(serde::Deserialize)]
struct ExtractedFact {
    content: String,
    category: String,
}

fn parse_extracted_facts(response: &str) -> Vec<ExtractedFact> {
    // Try parsing the response as JSON directly
    if let Ok(facts) = serde_json::from_str::<Vec<ExtractedFact>>(response) {
        return facts;
    }

    // Try extracting JSON array from markdown code block
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

    serde_json::from_str::<Vec<ExtractedFact>>(json_str).unwrap_or_default()
}
