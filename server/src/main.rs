mod app;
mod config;
mod domain;
mod routes;
mod services;

use std::net::SocketAddr;

use log::info;

#[tokio::main]
async fn main() {
    env_logger::init();

    let config = config::load_config().unwrap_or_else(|err| {
        panic!(
            "failed to load config from {}: {err}",
            config::config_path().display()
        )
    });

    let state = app::state::AppState::new(config);
    let addr = SocketAddr::from(([127, 0, 0, 1], state.config.port));
    let app = app::router::build_router(state);

    info!("Starting server on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind tcp listener");
    axum::serve(listener, app)
        .await
        .expect("server exited unexpectedly");
}
