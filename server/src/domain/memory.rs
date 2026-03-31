use serde::{Deserialize, Serialize};

/// A memory file entry in the library catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Relative path within the memory directory (e.g. "about/basics.md").
    pub path: String,
    /// First non-empty line of the file (used as summary in the catalog).
    pub summary: String,
    /// File size in bytes.
    pub size: usize,
}

/// Undirected graph of connections between memory files.
/// Each edge is a sorted pair of paths (a < b) to avoid duplicates.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryGraph {
    pub edges: Vec<[String; 2]>,
}
