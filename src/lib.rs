pub mod config;
pub mod errors;
pub mod models;
pub mod service;
pub mod stats;

// Re-export main functions for testing
use axum::{
    extract::State,
    http::StatusCode,
    response::{Json, Response},
};
use std::sync::Arc;
use service::LoroService;

pub async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "message": "Loro AI Voice Assistant - Fast Response API",
        "mode": "streaming_only",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy"
    }))
}

pub async fn chat_completions(
    State(service): State<Arc<LoroService>>,
    Json(request): Json<models::ChatCompletionRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    use tracing::warn;
    use errors::LoroError;

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

pub async fn get_metrics(State(service): State<Arc<LoroService>>) -> Json<serde_json::Value> {
    Json(service.get_metrics().await)
}

pub async fn reset_metrics(State(service): State<Arc<LoroService>>) -> Json<serde_json::Value> {
    service.reset_metrics().await;
    Json(serde_json::json!({
        "message": "Metrics reset successfully"
    }))
}
