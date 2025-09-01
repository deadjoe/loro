use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use loro::{
    config::{Config, ModelConfig},
    service::LoroService,
};
use secrecy::Secret;

async fn create_test_app() -> Router {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 3000,
        log_level: "info".to_string(),
        small_model: ModelConfig {
            api_key: Secret::new("test-key-small".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            model_name: "gpt-3.5-turbo".to_string(),
        },
        large_model: ModelConfig {
            api_key: Secret::new("test-key-large".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            model_name: "gpt-4".to_string(),
        },
        http_timeout_secs: 30,
        small_model_timeout_secs: 10,
        max_retries: 3,
        stats_max_entries: 1000,
    };

    let loro_service = Arc::new(LoroService::new(config).await.unwrap());

    Router::new()
        .route("/", get(loro::root))
        .route("/health", get(loro::health))
        .route("/v1/chat/completions", post(loro::chat_completions))
        .route("/metrics", get(loro::get_metrics))
        .route("/metrics/reset", post(loro::reset_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(loro_service)
}

#[tokio::test]
async fn test_root_endpoint() {
    let app = create_test_app().await;

    let request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["message"].as_str().unwrap().contains("Loro AI Voice Assistant"));
    assert_eq!(json["mode"].as_str().unwrap(), "streaming_only");
    assert!(json["version"].as_str().is_some());
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app().await;

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"].as_str().unwrap(), "healthy");
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = create_test_app().await;

    let request = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Check the nested structure
    assert!(json["direct_mode"]["total_requests"].is_number());
    assert!(json["quick_response_mode"]["total_requests"].is_number());
    assert!(json["direct_mode"]["first_response_latency"].is_object());
    assert!(json["quick_response_mode"]["first_response_latency"].is_object());
    assert!(json["comparison"].is_object());
}

#[tokio::test]
async fn test_metrics_reset_endpoint() {
    let app = create_test_app().await;

    let request = Request::builder()
        .method("POST")
        .uri("/metrics/reset")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["message"].as_str().unwrap().contains("reset successfully"));
}

#[tokio::test]
async fn test_chat_completion_validation_error() {
    let app = create_test_app().await;

    let invalid_request = json!({
        "model": "",
        "messages": []
    });

    let request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(invalid_request.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["error"].is_object());
    assert!(json["error"]["type"].as_str().unwrap() == "invalid_request_error");
    assert!(json["error"]["code"].as_str().unwrap() == "validation_failed");
}

#[tokio::test]
async fn test_chat_completion_empty_messages() {
    let app = create_test_app().await;

    let invalid_request = json!({
        "model": "gpt-3.5-turbo",
        "messages": []
    });

    let request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(invalid_request.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["error"]["message"].as_str().unwrap().contains("cannot be empty"));
}

#[tokio::test]
async fn test_chat_completion_invalid_role() {
    let app = create_test_app().await;

    let invalid_request = json!({
        "model": "gpt-3.5-turbo",
        "messages": [
            {
                "role": "invalid_role",
                "content": "Hello"
            }
        ]
    });

    let request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(invalid_request.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["error"]["message"].as_str().unwrap().contains("invalid role"));
}

#[tokio::test]
async fn test_chat_completion_invalid_temperature() {
    let app = create_test_app().await;

    let invalid_request = json!({
        "model": "gpt-3.5-turbo",
        "messages": [
            {
                "role": "user",
                "content": "Hello"
            }
        ],
        "temperature": 3.0
    });

    let request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(invalid_request.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["error"]["message"].as_str().unwrap().contains("Temperature must be"));
}

#[tokio::test]
async fn test_chat_completion_invalid_max_tokens() {
    let app = create_test_app().await;

    let invalid_request = json!({
        "model": "gpt-3.5-turbo",
        "messages": [
            {
                "role": "user",
                "content": "Hello"
            }
        ],
        "max_tokens": 0
    });

    let request = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(invalid_request.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["error"]["message"].as_str().unwrap().contains("max_tokens must be"));
}

#[tokio::test] 
async fn test_nonexistent_endpoint() {
    let app = create_test_app().await;

    let request = Request::builder()
        .uri("/nonexistent")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}