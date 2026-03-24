use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadMeta {
    pub id: String,
    pub original_name: String,
    pub stored_name: String,
    pub mime_type: String,
    pub size: u64,
    pub uploaded_at: String,
    /// Anthropic Files API file_id (for efficient API usage without base64).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anthropic_file_id: Option<String>,
}
