use axum::{Json, Router, extract::State, routing::get};

use crate::{app::state::AppState, domain::instance::InstanceSummary, services::workspace};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/instances", get(list_instances))
}

async fn list_instances(State(state): State<AppState>) -> Json<Vec<InstanceSummary>> {
    let instances = workspace::read_instances(&state.workspace_dir.join("instances"))
        .unwrap_or_default();
    Json(instances)
}
