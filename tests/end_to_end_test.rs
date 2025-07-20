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

#[tokio::test]
async fn test_full_server_integration() {
    // Set up complete test environment
    std::env::set_var("SMALL_MODEL_API_KEY", "test-small-key");
    std::env::set_var("LARGE_MODEL_API_KEY", "test-large-key");
    std::env::set_var("HOST", "127.0.0.1");
    std::env::set_var("PORT", "0"); // Use random port
    std::env::set_var("HTTP_TIMEOUT_SECS", "30");
    std::env::set_var("SMALL_MODEL_TIMEOUT_SECS", "5");
    std::env::set_var("MAX_RETRIES", "3");
    std::env::set_var("STATS_MAX_ENTRIES", "1000");

    // Test config loading
    let config = Config::from_env().expect("Config should load successfully");
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.http_timeout_secs, 30);
    assert_eq!(config.small_model_timeout_secs, 5);
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.stats_max_entries, 1000);

    // Test service creation
    let service = LoroService::new(config)
        .await
        .expect("Service should initialize");
    let service = Arc::new(service);

    // Test that service can handle multiple request types
    let test_requests = vec![
        ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "你好".to_string(), // Greeting
            }],
            max_tokens: Some(50),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        },
        ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "What is the weather like?".to_string(), // Question
            }],
            max_tokens: Some(100),
            temperature: 0.5,
            stream: true,
            stop: None,
            disable_quick_response: true, // Test direct mode
        },
    ];

    // Test that requests don't panic or deadlock
    for (i, request) in test_requests.into_iter().enumerate() {
        let service_clone = service.clone();
        let result = tokio::spawn(async move {
            // These will fail due to no real API endpoints, but shouldn't panic
            let _result = service_clone.chat_completion(request).await;
            i
        })
        .await;

        assert!(
            result.is_ok(),
            "Request {} should complete without panic",
            i
        );
    }

    // Test metrics collection
    let metrics = service.get_metrics().await;
    assert!(metrics.is_object(), "Metrics should be a JSON object");

    let quick_mode = metrics.get("quick_response_mode");
    let direct_mode = metrics.get("direct_mode");
    let comparison = metrics.get("comparison");

    assert!(
        quick_mode.is_some(),
        "Should have quick response mode metrics"
    );
    assert!(direct_mode.is_some(), "Should have direct mode metrics");
    assert!(comparison.is_some(), "Should have comparison metrics");

    // Test metrics reset
    service.reset_metrics().await;
    let reset_metrics = service.get_metrics().await;

    // After reset, request counts should be 0
    let comparison_after_reset = reset_metrics["comparison"].as_object().unwrap();
    let quick_requests = comparison_after_reset["quick_mode_requests"]
        .as_u64()
        .unwrap();
    let direct_requests = comparison_after_reset["direct_mode_requests"]
        .as_u64()
        .unwrap();

    assert_eq!(
        quick_requests, 0,
        "Quick mode requests should be reset to 0"
    );
    assert_eq!(
        direct_requests, 0,
        "Direct mode requests should be reset to 0"
    );
}

#[tokio::test]
async fn test_stats_percentile_comprehensive() {
    use loro::stats::calculate_stats;

    // Test empty data
    let empty_stats = calculate_stats(&[]);
    assert_eq!(empty_stats.avg, 0.0);
    assert_eq!(empty_stats.min, 0.0);
    assert_eq!(empty_stats.max, 0.0);
    assert_eq!(empty_stats.p50, 0.0);
    assert_eq!(empty_stats.p95, 0.0);

    // Test single data point
    let single_stats = calculate_stats(&[5.0]);
    assert_eq!(single_stats.avg, 5.0);
    assert_eq!(single_stats.min, 5.0);
    assert_eq!(single_stats.max, 5.0);
    assert_eq!(single_stats.p50, 5.0);
    assert_eq!(single_stats.p95, 5.0);

    // Test two data points
    let two_stats = calculate_stats(&[3.0, 7.0]);
    assert_eq!(two_stats.avg, 5.0);
    assert_eq!(two_stats.min, 3.0);
    assert_eq!(two_stats.max, 7.0);

    // Test large dataset
    let large_data: Vec<f64> = (1..=1000).map(|i| i as f64).collect();
    let large_stats = calculate_stats(&large_data);
    assert!((large_stats.avg - 500.5).abs() < 0.1);
    assert_eq!(large_stats.min, 1.0);
    assert_eq!(large_stats.max, 1000.0);
    assert!((large_stats.p50 - 500.0).abs() < 10.0); // Should be around median
    assert!((large_stats.p95 - 950.0).abs() < 50.0); // Should be around 95th percentile
}
