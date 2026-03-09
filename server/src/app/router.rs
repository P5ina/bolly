use axum::Router;

use crate::{app::state::AppState, routes};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(routes::health::router())
        .merge(routes::meta::router())
        .merge(routes::instances::router())
        .merge(routes::chat::router())
        .merge(routes::config::router())
        .merge(routes::ws::router())
        .with_state(state)
}
