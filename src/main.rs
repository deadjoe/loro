use axum::{
    extract::State,
    http::StatusCode,
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};

mod models;
mod service;
mod stats;
mod config;

use config::Config;
use service::LoroService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "loro=debug,tower_http=debug".into()),
        )
        .init();

    // Load configuration
    let config = Config::from_env()?;
    info!("Starting Loro AI Voice Assistant on {}:{}", config.host, config.port);

    // Initialize service
    let loro_service = Arc::new(LoroService::new(config.clone()).await?);

    // Build router
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/metrics", get(get_metrics))
        .route("/metrics/reset", post(reset_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(loro_service);

    // Start server
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;
    info!("🚀 Loro server listening on {}", listener.local_addr()?);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Loro AI Voice Assistant - Fast Response API",
        "mode": "streaming_only",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy"
    }))
}

async fn chat_completions(
    State(service): State<Arc<LoroService>>,
    Json(request): Json<models::ChatCompletionRequest>,
) -> Result<Response, StatusCode> {
    match service.chat_completion(request).await {
        Ok(response) => Ok(response),
        Err(e) => {
            warn!("Chat completion error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_metrics(
    State(service): State<Arc<LoroService>>,
) -> Json<serde_json::Value> {
    Json(service.get_metrics().await)
}

async fn reset_metrics(
    State(service): State<Arc<LoroService>>,
) -> Json<serde_json::Value> {
    service.reset_metrics().await;
    Json(serde_json::json!({
        "message": "Metrics reset successfully"
    }))
}