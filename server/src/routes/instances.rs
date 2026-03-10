use axum::{Json, Router, extract::{Path, State}, routing::get};
use serde::Serialize;

use crate::{app::state::AppState, domain::instance::InstanceSummary, services::{tools, workspace}};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/instances", get(list_instances))
        .route("/api/instances/{instance_slug}/mood", get(get_mood))
}

async fn list_instances(State(state): State<AppState>) -> Json<Vec<InstanceSummary>> {
    let instances = workspace::read_instances(&state.workspace_dir.join("instances"))
        .unwrap_or_default();
    Json(instances)
}

#[derive(Serialize)]
struct MoodResponse {
    mood: String,
}

async fn get_mood(
    State(state): State<AppState>,
    Path(instance_slug): Path<String>,
) -> Json<MoodResponse> {
    let instance_dir = state.workspace_dir.join("instances").join(&instance_slug);
    let mood_state = tools::load_mood_state(&instance_dir);
    Json(MoodResponse {
        mood: mood_state.companion_mood,
    })
}
