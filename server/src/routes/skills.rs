use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
};

use crate::{
    app::state::AppState,
    domain::skill::Skill,
    services::skills,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/skills", get(list_skills))
        .route("/api/skills", post(create_skill))
        .route("/api/skills/{skill_id}", get(get_skill))
        .route("/api/skills/{skill_id}", delete(delete_skill))
}

async fn list_skills(State(state): State<AppState>) -> Json<Vec<Skill>> {
    Json(skills::list_skills(&state.workspace_dir))
}

async fn get_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> Result<Json<Skill>, StatusCode> {
    skills::get_skill(&state.workspace_dir, &skill_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_skill(
    State(state): State<AppState>,
    Json(skill): Json<Skill>,
) -> Result<Json<Skill>, StatusCode> {
    skills::create_skill(&state.workspace_dir, &skill)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(skill))
}

async fn delete_skill(
    State(state): State<AppState>,
    Path(skill_id): Path<String>,
) -> StatusCode {
    match skills::delete_skill(&state.workspace_dir, &skill_id) {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
