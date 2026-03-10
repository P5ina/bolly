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

    let port = config.port;
    let state = app::state::AppState::new(config);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    // Start background scheduler for scheduled messages
    services::scheduler::start(&state.workspace_dir, state.events.clone());

    // Start heartbeat — companion's autonomous inner life
    services::heartbeat::start(
        &state.workspace_dir,
        state.llm.clone(),
        state.events.clone(),
    );

    let app = app::router::build_router(state);

    info!("Starting server on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind tcp listener");
    axum::serve(listener, app)
        .await
        .expect("server exited unexpectedly");
}
