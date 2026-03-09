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
