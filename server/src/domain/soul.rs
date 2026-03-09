use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Soul {
    pub content: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSoulRequest {
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyTemplateRequest {
    pub template_id: String,
}
