use rig::Embed;
use rig_sqlite::{Column, ColumnValue, SqliteVectorStoreTable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Embed)]
pub struct MemoryFact {
    pub id: String,
    #[embed]
    pub content: String,
    pub category: String,
    pub created_at: String,
}

impl SqliteVectorStoreTable for MemoryFact {
    fn name() -> &'static str {
        "memory_facts"
    }

    fn schema() -> Vec<Column> {
        vec![
            Column::new("id", "TEXT PRIMARY KEY"),
            Column::new("content", "TEXT"),
            Column::new("category", "TEXT").indexed(),
            Column::new("created_at", "TEXT"),
        ]
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
        vec![
            ("id", Box::new(self.id.clone())),
            ("content", Box::new(self.content.clone())),
            ("category", Box::new(self.category.clone())),
            ("created_at", Box::new(self.created_at.clone())),
        ]
    }
}

/// An episodic memory — a shared moment, not just a fact.
/// "you showed me your first game at 2am and were so excited"
/// vs a fact: "you are a game developer"
#[derive(Debug, Clone, Serialize, Deserialize, Embed)]
pub struct MemoryEpisode {
    pub id: String,
    /// What happened — the moment itself, written as a vivid recollection.
    #[embed]
    pub content: String,
    /// The emotional tone of the moment (e.g. "excited", "vulnerable", "proud").
    pub emotion: String,
    /// Why this moment matters — what it revealed or changed.
    pub significance: String,
    pub created_at: String,
}

impl SqliteVectorStoreTable for MemoryEpisode {
    fn name() -> &'static str {
        "memory_episodes"
    }

    fn schema() -> Vec<Column> {
        vec![
            Column::new("id", "TEXT PRIMARY KEY"),
            Column::new("content", "TEXT"),
            Column::new("emotion", "TEXT"),
            Column::new("significance", "TEXT"),
            Column::new("created_at", "TEXT"),
        ]
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn column_values(&self) -> Vec<(&'static str, Box<dyn ColumnValue>)> {
        vec![
            ("id", Box::new(self.id.clone())),
            ("content", Box::new(self.content.clone())),
            ("emotion", Box::new(self.emotion.clone())),
            ("significance", Box::new(self.significance.clone())),
            ("created_at", Box::new(self.created_at.clone())),
        ]
    }
}
