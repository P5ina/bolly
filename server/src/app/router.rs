use std::path::PathBuf;

use axum::{middleware, Router};
use tower_http::services::{ServeDir, ServeFile};

use crate::{app::state::AppState, routes};

use super::auth::auth_middleware;

pub fn build_router(state: AppState, static_dir: Option<PathBuf>) -> Router {
    // API routes — protected by auth middleware
    let api = Router::new()
        .merge(routes::meta::router())
        .merge(routes::instances::router())
        .merge(routes::chat::router())
        .merge(routes::drops::router())
        .merge(routes::config::router())
        .merge(routes::soul::router())
        .merge(routes::uploads::router())
        .merge(routes::ws::router())
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Public routes — no auth
    let health = routes::health::router();
    let auth = routes::auth::router();

    let app = Router::new()
        .merge(health)
        .merge(auth)
        .merge(api)
        .with_state(state);

    // Serve static client files as fallback (SPA routing)
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        let serve = ServeDir::new(dir).not_found_service(ServeFile::new(index));
        app.fallback_service(serve)
    } else {
        app
    }
}
