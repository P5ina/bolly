//! Qdrant vector store wrapper — manages collections, upserts, and similarity search.

use std::path::Path;

use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter, PointStruct,
    SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use uuid::Uuid;

use super::embedding;

/// UUID v5 namespace for deterministic point IDs.
const NS: Uuid = Uuid::from_bytes([
    0x6b, 0x6f, 0x6c, 0x6c, 0x79, 0x2d, 0x76, 0x65,
    0x63, 0x74, 0x6f, 0x72, 0x2d, 0x6e, 0x73, 0x21,
]);

pub struct VectorStore {
    client: Qdrant,
}

#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub path: String,
    pub source_type: String,
    pub content_preview: String,
    pub score: f32,
    pub upload_id: Option<String>,
}

/// Extract a string field from Qdrant payload.
fn payload_str(payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| match &v.kind {
            Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

impl VectorStore {
    /// Connect to Qdrant at the given gRPC URL. Panics if connection fails.
    pub async fn connect(url: &str) -> Self {
        let client = Qdrant::from_url(url)
            .build()
            .unwrap_or_else(|e| panic!("failed to build Qdrant client for {url}: {e}"));

        client
            .health_check()
            .await
            .unwrap_or_else(|e| panic!("Qdrant health check failed at {url}: {e}"));

        log::info!("[vector] connected to Qdrant at {url}");
        Self { client }
    }

    /// Collection name for an instance.
    fn collection(instance_slug: &str) -> String {
        format!("memories_{instance_slug}")
    }

    /// Deterministic point ID from (source_type, path, chunk_index).
    fn point_id(source_type: &str, path: &str, chunk_index: u32) -> String {
        let input = format!("{source_type}:{path}:{chunk_index}");
        Uuid::new_v5(&NS, input.as_bytes()).to_string()
    }

    /// Drop and recreate the collection (reset all vectors).
    pub async fn reset_collection(&self, instance_slug: &str) -> Result<(), String> {
        let name = Self::collection(instance_slug);
        let exists = self
            .client
            .collection_exists(&name)
            .await
            .map_err(|e| format!("qdrant collection_exists: {e}"))?;

        if exists {
            self.client
                .delete_collection(&name)
                .await
                .map_err(|e| format!("qdrant delete_collection: {e}"))?;
            log::info!("[vector] deleted collection {name}");
        }

        self.ensure_collection(instance_slug).await
    }

    /// Ensure the collection exists, create if not.
    pub async fn ensure_collection(&self, instance_slug: &str) -> Result<(), String> {
        let name = Self::collection(instance_slug);
        let exists = self
            .client
            .collection_exists(&name)
            .await
            .map_err(|e| format!("qdrant collection_exists: {e}"))?;

        if !exists {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&name).vectors_config(
                        VectorParamsBuilder::new(embedding::output_dim() as u64, Distance::Cosine),
                    ),
                )
                .await
                .map_err(|e| format!("qdrant create_collection: {e}"))?;
            log::info!("[vector] created collection {name}");
        }

        Ok(())
    }

    /// Upsert text memory chunks.
    pub async fn upsert_text_memory(
        &self,
        instance_slug: &str,
        path: &str,
        chunks: Vec<(String, Vec<f32>)>,
    ) -> Result<(), String> {
        let collection = Self::collection(instance_slug);

        // First delete any existing points for this path (in case chunk count changed)
        self.delete_by_filter(&collection, "path", path).await?;

        let points: Vec<PointStruct> = chunks
            .into_iter()
            .enumerate()
            .map(|(i, (text, vector))| {
                let preview: String = text.chars().take(500).collect();
                PointStruct::new(
                    Self::point_id("text_memory", path, i as u32),
                    vector,
                    [
                        ("source_type", "text_memory".into()),
                        ("path", path.into()),
                        ("chunk_index", (i as i64).into()),
                        ("content_preview", preview.into()),
                        ("timestamp", chrono::Utc::now().timestamp_millis().into()),
                    ],
                )
            })
            .collect();

        if points.is_empty() {
            return Ok(());
        }

        self.client
            .upsert_points(UpsertPointsBuilder::new(&collection, points))
            .await
            .map_err(|e| format!("qdrant upsert: {e}"))?;

        Ok(())
    }

    /// Upsert a media embedding (image, video, audio).
    pub async fn upsert_media(
        &self,
        instance_slug: &str,
        upload_id: &str,
        source_type: &str,
        mime_type: &str,
        original_name: &str,
        content_preview: &str,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        let collection = Self::collection(instance_slug);
        let preview: String = content_preview.chars().take(500).collect();

        let point = PointStruct::new(
            Self::point_id(source_type, upload_id, 0),
            vector,
            [
                ("source_type", source_type.into()),
                ("path", upload_id.into()),
                ("upload_id", upload_id.into()),
                ("mime_type", mime_type.into()),
                ("original_name", original_name.into()),
                ("content_preview", preview.into()),
                ("chunk_index", 0i64.into()),
                ("timestamp", chrono::Utc::now().timestamp_millis().into()),
            ],
        );

        self.client
            .upsert_points(UpsertPointsBuilder::new(&collection, vec![point]))
            .await
            .map_err(|e| format!("qdrant upsert media: {e}"))?;

        Ok(())
    }

    /// Delete all points matching a path value.
    pub async fn delete_by_path(&self, instance_slug: &str, path: &str) -> Result<(), String> {
        let collection = Self::collection(instance_slug);
        self.delete_by_filter(&collection, "path", path).await
    }

    /// Search for similar vectors.
    pub async fn search(
        &self,
        instance_slug: &str,
        query_vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, String> {
        let collection = Self::collection(instance_slug);

        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&collection, query_vector, limit as u64)
                    .with_payload(true),
            )
            .await
            .map_err(|e| format!("qdrant search: {e}"))?;

        let out = results
            .result
            .into_iter()
            .map(|point| {
                let p = &point.payload;
                VectorSearchResult {
                    path: payload_str(p, "path"),
                    source_type: payload_str(p, "source_type"),
                    content_preview: payload_str(p, "content_preview"),
                    score: point.score,
                    upload_id: p.get("upload_id").and_then(|v| v.as_str().map(|s| s.to_string())),
                }
            })
            .collect();

        Ok(out)
    }

    /// List all points with metadata (for debugging).
    pub async fn list_all(
        &self,
        instance_slug: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, String> {
        let collection = Self::collection(instance_slug);

        // Use a zero vector to get all points sorted by... nothing useful,
        // but it's the simplest way to scroll without pagination.
        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(
                    &collection,
                    vec![0.0; super::embedding::output_dim() as usize],
                    limit as u64,
                )
                .with_payload(true),
            )
            .await
            .map_err(|e| format!("qdrant list: {e}"))?;

        Ok(results
            .result
            .into_iter()
            .map(|point| {
                let p = &point.payload;
                VectorSearchResult {
                    path: payload_str(p, "path"),
                    source_type: payload_str(p, "source_type"),
                    content_preview: payload_str(p, "content_preview"),
                    score: point.score,
                    upload_id: p.get("upload_id").and_then(|v| v.as_str().map(|s| s.to_string())),
                }
            })
            .collect())
    }

    /// Find pairs of similar text memories above a threshold (for consolidation).
    pub async fn find_similar_pairs(
        &self,
        instance_slug: &str,
        threshold: f32,
    ) -> Result<Vec<(String, String, f32)>, String> {
        let collection = Self::collection(instance_slug);

        // Get all text_memory points with vectors
        let all_points = self
            .client
            .search_points(
                SearchPointsBuilder::new(
                    &collection,
                    vec![0.0; embedding::output_dim() as usize],
                    500,
                )
                .with_payload(true)
                .with_vectors(true)
                .filter(Filter::must([Condition::matches(
                    "source_type",
                    "text_memory".to_string(),
                )])),
            )
            .await
            .map_err(|e| format!("qdrant scroll: {e}"))?;

        let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut pairs = Vec::new();

        for point in &all_points.result {
            let path = payload_str(&point.payload, "path");

            if path.is_empty() || !seen_paths.insert(path.clone()) {
                continue;
            }

            // Extract vector from the point
            let vector: Vec<f32> = match &point.vectors {
                Some(vectors) => {
                    if let Some(vec) = vectors.vectors_options.as_ref() {
                        match vec {
                            qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(v) => {
                                v.data.clone()
                            }
                            _ => continue,
                        }
                    } else {
                        continue;
                    }
                }
                None => continue,
            };

            let similar = self
                .client
                .search_points(
                    SearchPointsBuilder::new(&collection, vector, 5)
                        .with_payload(true)
                        .score_threshold(threshold)
                        .filter(Filter::must([Condition::matches(
                            "source_type",
                            "text_memory".to_string(),
                        )])),
                )
                .await
                .map_err(|e| format!("qdrant similar search: {e}"))?;

            for sim in similar.result {
                let sim_path = payload_str(&sim.payload, "path");
                if sim_path != path && sim.score >= threshold {
                    pairs.push((path.clone(), sim_path, sim.score));
                }
            }
        }

        Ok(pairs)
    }

    /// Backfill all existing text memories into Qdrant.
    pub async fn backfill_text_memories(
        &self,
        workspace_dir: &Path,
        instance_slug: &str,
        google_ai_key: &str,
    ) -> Result<usize, String> {
        use super::memory;

        self.ensure_collection(instance_slug).await?;

        let entries = memory::scan_library(workspace_dir, instance_slug);
        let mut count = 0;

        for entry in &entries {
            let file_path = workspace_dir
                .join("instances")
                .join(instance_slug)
                .join("memory")
                .join(&entry.path);

            let content = match std::fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let chunks = chunk_text(&content);
            let mut chunk_vectors = Vec::new();

            for chunk in &chunks {
                match embedding::embed_text(
                    google_ai_key,
                    chunk,
                    embedding::TaskType::RetrievalDocument,
                )
                .await
                {
                    Ok(vec) => chunk_vectors.push((chunk.clone(), vec)),
                    Err(e) => {
                        log::warn!("[vector] backfill embed error for {}: {e}", entry.path);
                        continue;
                    }
                }

                // Rate limit: small delay between API calls
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            if !chunk_vectors.is_empty() {
                self.upsert_text_memory(instance_slug, &entry.path, chunk_vectors)
                    .await?;
                count += chunks.len();
            }
        }

        Ok(count)
    }

    // --- Internal ---

    async fn delete_by_filter(
        &self,
        collection: &str,
        field: &str,
        value: &str,
    ) -> Result<(), String> {
        let filter = Filter::must([Condition::matches(field.to_string(), value.to_string())]);

        self.client
            .delete_points(DeletePointsBuilder::new(collection).points(filter))
            .await
            .map_err(|e| format!("qdrant delete: {e}"))?;

        Ok(())
    }
}

/// Chunk text into ~600 byte paragraphs (matching BM25 chunking in memory.rs).
pub fn chunk_text(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![];
    }
    if text.len() < 800 {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if current.len() + line.len() > 600 && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks.retain(|c| !c.trim().is_empty());
    chunks
}
