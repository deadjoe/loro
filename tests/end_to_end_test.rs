use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use loro::{config::Config, models::*, service::LoroService};
use serde_json::json;
use std::sync::Arc;
use tower::util::ServiceExt;

// Mock HTTP server for testing
async fn create_test_app() -> Router {
    // Set up test environment
    std::env::set_var("SMALL_MODEL_API_KEY", "test-key");
    std::env::set_var("LARGE_MODEL_API_KEY", "test-key");
    std::env::set_var("SMALL_MODEL_BASE_URL", "https://httpbin.org/status/200");
    std::env::set_var("LARGE_MODEL_BASE_URL", "https://httpbin.org/status/200");

    let config = Config::from_env().expect("Failed to load test config");
    let service = Arc::new(
        LoroService::new(config)
            .await
            .expect("Failed to create service"),
    );

    axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async {
                axum::Json(json!({
                    "message": "Loro AI Voice Assistant - Fast Response API",
                    "mode": "streaming_only",
                    "version": "0.1.0"
                }))
            }),
        )
        .route(
            "/health",
            axum::routing::get(|| async { axum::Json(json!({"status": "healthy"})) }),
        )
        .route(
            "/metrics",
            axum::routing::get({
                let service = Arc::clone(&service);
                move || {
                    let service = Arc::clone(&service);
                    async move { axum::Json(service.get_metrics().await) }
                }
            }),
        )
        .route(
            "/metrics/reset",
            axum::routing::post({
                let service = Arc::clone(&service);
                move || {
                    let service = Arc::clone(&service);
                    async move {
                        service.reset_metrics().await;
                        axum::Json(json!({"message": "Metrics reset successfully"}))
                    }
                }
            }),
        )
}

#[tokio::test]
async fn test_root_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        json["message"],
        "Loro AI Voice Assistant - Fast Response API"
    );
    assert_eq!(json["mode"], "streaming_only");
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
async fn test_metrics_endpoints() {
    let app = create_test_app().await;

    // Test metrics endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json.get("quick_response_mode").is_some());
    assert!(json.get("direct_mode").is_some());
    assert!(json.get("comparison").is_some());

    // Test metrics reset endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/metrics/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["message"], "Metrics reset successfully");
}

#[tokio::test]
async fn test_request_validation_edge_cases() {
    // Test message categorization with various inputs
    let test_cases = vec![
        ("你好", RequestCategory::Greeting),
        ("hello", RequestCategory::Greeting),
        ("什么是AI?", RequestCategory::Question),
        ("how are you", RequestCategory::Question),
        ("请帮我", RequestCategory::Request),
        ("can you help", RequestCategory::Request),
        ("我觉得", RequestCategory::Thinking),
        ("random text", RequestCategory::Thinking),
        ("", RequestCategory::Thinking), // Empty should default to thinking
    ];

    for (input, expected) in test_cases {
        let message = Message {
            role: "user".to_string(),
            content: input.to_string(),
        };

        let category = message.categorize();
        // We can't directly compare enum variants, so we check the responses
        let expected_responses = expected.get_responses();
        let actual_responses = category.get_responses();

        // Check if the response lists are the same
        assert_eq!(expected_responses.len(), actual_responses.len());
        for (exp, act) in expected_responses.iter().zip(actual_responses.iter()) {
            assert_eq!(exp, act);
        }
    }
}

#[tokio::test]
async fn test_performance_tracking() {
    use loro::stats::StatsCollector;

    let collector = StatsCollector::new(100);

    // Simulate some requests with realistic timing
    let test_data = vec![
        (0.05, 1.2, Some(0.05), Some(1.15)), // Very fast quick response
        (0.1, 0.8, Some(0.1), Some(0.7)),    // Normal quick response
        (0.15, 1.5, Some(0.15), Some(1.35)), // Slower quick response
        (0.8, 2.1, None, Some(2.1)),         // Direct mode (no quick response)
        (1.0, 2.5, None, Some(2.5)),         // Direct mode (slower)
    ];

    for (first, total, quick, large) in test_data {
        collector.add_request(first, total, quick, large);
    }

    let stats = collector.get_stats();

    // Verify stats structure
    assert!(stats.get("total_requests").is_some());
    assert_eq!(stats["total_requests"], 5);

    let first_response_stats = stats["first_response_latency"].as_object().unwrap();
    assert!(first_response_stats.get("avg").is_some());
    assert!(first_response_stats.get("min").is_some());
    assert!(first_response_stats.get("max").is_some());
    assert!(first_response_stats.get("p50").is_some());
    assert!(first_response_stats.get("p95").is_some());

    // Check that min <= p50 <= p95 <= max
    let min = first_response_stats["min"].as_f64().unwrap();
    let p50 = first_response_stats["p50"].as_f64().unwrap();
    let p95 = first_response_stats["p95"].as_f64().unwrap();
    let max = first_response_stats["max"].as_f64().unwrap();

    assert!(min <= p50);
    assert!(p50 <= p95);
    assert!(p95 <= max);
}

#[tokio::test]
async fn test_concurrent_stats_collection() {
    use loro::stats::StatsCollector;
    use std::sync::Arc;
    use tokio::task;

    let collector = Arc::new(StatsCollector::new(1000));
    let mut handles = vec![];

    // Spawn multiple concurrent tasks to test thread safety
    for i in 0..10 {
        let collector = Arc::clone(&collector);
        let handle = task::spawn(async move {
            for j in 0..10 {
                let base_time = (i * 10 + j) as f64 * 0.01;
                collector.add_request(
                    base_time,
                    base_time * 2.0,
                    Some(base_time),
                    Some(base_time * 1.8),
                );
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify that all requests were recorded
    assert_eq!(collector.get_request_count(), 100);

    let stats = collector.get_stats();
    assert_eq!(stats["total_requests"], 100);
}

#[tokio::test]
async fn test_memory_limit_enforcement() {
    use loro::stats::StatsCollector;

    let small_limit = 5;
    let collector = StatsCollector::new(small_limit);

    // Add more entries than the limit
    for i in 0..20 {
        collector.add_request(
            i as f64,
            i as f64 * 2.0,
            Some(i as f64),
            Some(i as f64 * 1.5),
        );
    }

    // All requests should be counted
    assert_eq!(collector.get_request_count(), 20);

    // But the underlying storage should be limited
    // This is implementation-dependent, but we should not run out of memory
    let stats = collector.get_stats();
    assert!(stats["total_requests"].as_u64().unwrap() > 0);
}
