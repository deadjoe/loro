use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

use loro::{config::Config, service::LoroService};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    // Load configuration first to get unified log level
    let config = Config::from_env()?;

    // Initialize tracing using Config.log_level (unified entrypoint)
    let level = config.log_level.to_lowercase();
    let filter = tracing_subscriber::EnvFilter::new(format!("{},tower_http={}", level, level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Performance hint: Consider setting thread affinity in production
    // e.g., use taskset on Linux or thread affinity APIs
    info!(
        "Starting Loro AI Voice Assistant on {}:{}",
        config.host, config.port
    );

    // Initialize service
    let loro_service = Arc::new(LoroService::new(config.clone()).await?);

    // Build router
    let app = Router::new()
        .route("/", get(loro::root))
        .route("/health", get(loro::health))
        .route("/v1/chat/completions", post(loro::chat_completions))
        .route("/metrics", get(loro::get_metrics))
        .route("/metrics/reset", post(loro::reset_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(loro_service);

    // Start server
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;
    info!("🚀 Loro server listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
