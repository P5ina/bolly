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

    let host = config.host.clone();
    let port = config.port;
    let static_dir = if config.static_dir.is_empty() {
        None
    } else {
        let path = std::path::PathBuf::from(&config.static_dir);
        if path.is_dir() {
            Some(path)
        } else {
            log::warn!("static_dir {} does not exist, skipping", config.static_dir);
            None
        }
    };

    let state = app::state::AppState::new(config);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .unwrap_or_else(|_| {
            log::warn!("invalid host:port {host}:{port}, falling back to 0.0.0.0:{port}");
            SocketAddr::from(([0, 0, 0, 0], port))
        });

    // Notify active chats that the server restarted
    services::chat::notify_restart(&state.workspace_dir, &state.events);

    // Start background scheduler for scheduled messages
    services::scheduler::start(&state.workspace_dir, state.events.clone());

    // Start heartbeat — companion's autonomous inner life
    services::heartbeat::start(
        &state.workspace_dir,
        state.llm.clone(),
        state.events.clone(),
    );

    let app = app::router::build_router(state, static_dir);

    info!("Starting server on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind tcp listener");
    axum::serve(listener, app)
        .await
        .expect("server exited unexpectedly");
}
