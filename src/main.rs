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

mod config;
mod errors;
mod models;
mod service;
mod stats;

use config::Config;
use errors::LoroError;
use service::LoroService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with performance optimizations
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "loro=debug,tower_http=debug".into()),
        )
        .with_target(false) // Reduce logging overhead in production
        .init();

    // Performance hint: Consider setting thread affinity in production
    // e.g., use taskset on Linux or thread affinity APIs

    // Load configuration
    let config = Config::from_env()?;
    info!(
        "Starting Loro AI Voice Assistant on {}:{}",
        config.host, config.port
    );

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
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Validate request
    if let Err(validation_error) = request.validate() {
        warn!("Request validation failed: {}", validation_error);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "message": validation_error,
                    "type": "invalid_request_error",
                    "code": "validation_failed"
                }
            })),
        ));
    }

    match service.chat_completion(request).await {
        Ok(response) => Ok(response),
        Err(e) => {
            warn!("Chat completion error: {}", e);
            let error_response = match e.downcast_ref::<LoroError>() {
                Some(LoroError::Timeout { timeout_secs }) => (
                    StatusCode::REQUEST_TIMEOUT,
                    Json(serde_json::json!({
                        "error": {
                            "message": format!("Request timeout after {}s", timeout_secs),
                            "type": "timeout_error",
                            "code": "request_timeout"
                        }
                    })),
                ),
                Some(LoroError::ApiError {
                    provider,
                    status: _,
                    message,
                }) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": {
                            "message": format!("API error from {}: {}", provider, message),
                            "type": "api_error",
                            "code": "upstream_error"
                        }
                    })),
                ),
                Some(LoroError::Validation(msg)) => (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": {
                            "message": msg,
                            "type": "invalid_request_error",
                            "code": "validation_failed"
                        }
                    })),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": {
                            "message": "Internal server error",
                            "type": "internal_error",
                            "code": "internal_error"
                        }
                    })),
                ),
            };
            Err(error_response)
        }
    }
}

async fn get_metrics(State(service): State<Arc<LoroService>>) -> Json<serde_json::Value> {
    Json(service.get_metrics().await)
}

async fn reset_metrics(State(service): State<Arc<LoroService>>) -> Json<serde_json::Value> {
    service.reset_metrics().await;
    Json(serde_json::json!({
        "message": "Metrics reset successfully"
    }))
}
