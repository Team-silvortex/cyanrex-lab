use cyanrex_engine::{build_router, build_state, config::AppConfig};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = AppConfig::from_env();
    let state = build_state();
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port))
        .await
        .expect("failed to bind listener");

    tracing::info!("cyanrex-engine listening on {}:{}", config.host, config.port);
    axum::serve(listener, app).await.expect("server error");
}
