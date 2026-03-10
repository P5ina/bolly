use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct InstanceSummary {
    pub slug: String,
    pub companion_name: String,
    pub soul_exists: bool,
    pub drops_count: usize,
    pub has_memory: bool,
    pub has_skin: bool,
}
