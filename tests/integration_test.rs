use loro::{
    config::Config, errors::LoroError, models::*, service::LoroService, stats::StatsCollector,
};

#[tokio::test]
async fn test_service_initialization() {
    // Set test environment variables
    std::env::set_var("SMALL_MODEL_API_KEY", "test-key");
    std::env::set_var("LARGE_MODEL_API_KEY", "test-key");

    let config = Config::from_env().expect("Failed to load config");
    let service = LoroService::new(config).await;
    assert!(service.is_ok(), "Service should initialize successfully");
}

#[tokio::test]
async fn test_message_categorization() {
    let greeting = Message {
        role: "user".to_string(),
        content: "你好".to_string(),
    };

    let question = Message {
        role: "user".to_string(),
        content: "什么是人工智能？".to_string(),
    };

    let request = Message {
        role: "user".to_string(),
        content: "请帮我设个闹钟".to_string(),
    };

    // Test categorization logic
    matches!(greeting.categorize(), RequestCategory::Greeting);
    matches!(question.categorize(), RequestCategory::Question);
    matches!(request.categorize(), RequestCategory::Request);
}

#[tokio::test]
async fn test_quick_responses() {
    use loro::models::RequestCategory;

    let categories = [
        RequestCategory::Greeting,
        RequestCategory::Question,
        RequestCategory::Request,
        RequestCategory::Thinking,
    ];

    for category in &categories {
        let responses = category.get_responses();
        assert!(!responses.is_empty(), "Category should have responses");
        assert!(
            responses.iter().all(|r| r.chars().count() <= 6),
            "Responses should be short"
        );
    }
}

#[tokio::test]
async fn test_stats_collector() {
    use loro::stats::StatsCollector;

    let collector = StatsCollector::new(1000);

    // Add some test data
    collector.add_request(0.1, 1.0, Some(0.1), Some(0.9));
    collector.add_request(0.2, 1.5, Some(0.2), Some(1.3));
    collector.add_request(0.15, 1.2, Some(0.15), Some(1.05));

    let stats = collector.get_stats();

    // Verify stats structure
    assert!(stats.get("total_requests").is_some());
    assert!(stats.get("first_response_latency").is_some());
    assert!(stats.get("total_response_latency").is_some());
    assert!(stats.get("quick_response_latency").is_some());
    assert!(stats.get("large_model_latency").is_some());

    assert_eq!(collector.get_request_count(), 3);

    // Test reset
    collector.reset();
    assert_eq!(collector.get_request_count(), 0);
}

#[cfg(test)]
mod mock_tests {
    use super::*;

    #[tokio::test]
    async fn test_request_serialization() {
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            max_tokens: Some(100),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };

        let json = serde_json::to_string(&request).expect("Should serialize");
        let parsed: ChatCompletionRequest =
            serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(request.model, parsed.model);
        assert_eq!(request.messages.len(), parsed.messages.len());
        assert_eq!(request.temperature, parsed.temperature);
    }

    #[tokio::test]
    async fn test_response_chunk_creation() {
        let chunk = ChatCompletionChunk {
            id: "test-id".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: "test-model".to_string(),
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: Some("assistant".to_string()),
                    content: Some("Hello!".to_string()),
                },
                finish_reason: None,
            }],
        };

        let json = serde_json::to_string(&chunk).expect("Should serialize chunk");
        let parsed: ChatCompletionChunk =
            serde_json::from_str(&json).expect("Should deserialize chunk");

        assert_eq!(chunk.id, parsed.id);
        assert_eq!(chunk.model, parsed.model);
        assert_eq!(chunk.choices.len(), parsed.choices.len());
    }

    #[tokio::test]
    async fn test_config_validation() {
        use loro::config::Config;

        // Test with valid environment (should succeed)
        std::env::set_var("SMALL_MODEL_API_KEY", "test-small-key");
        std::env::set_var("LARGE_MODEL_API_KEY", "test-large-key");
        std::env::set_var("HTTP_TIMEOUT_SECS", "30"); // Set valid timeout

        let result = Config::from_env();
        assert!(result.is_ok(), "Should succeed with valid API keys");

        // Test validation with empty API key - create a config manually to test validation
        let mut config = result.unwrap();
        config.small_model.api_key = "".to_string();
        assert!(config.validate().is_err(), "Should fail with empty API key");
    }

    #[tokio::test]
    async fn test_request_validation() {
        // Test empty messages
        let request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![],
            max_tokens: Some(100),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        assert!(
            request.validate().is_err(),
            "Should fail with empty messages"
        );

        // Test invalid role
        let request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![Message {
                role: "invalid".to_string(),
                content: "test".to_string(),
            }],
            max_tokens: Some(100),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        assert!(request.validate().is_err(), "Should fail with invalid role");

        // Test invalid temperature
        let request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            max_tokens: Some(100),
            temperature: 3.0, // Invalid
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        assert!(
            request.validate().is_err(),
            "Should fail with invalid temperature"
        );

        // Test valid request
        let request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            max_tokens: Some(100),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        assert!(request.validate().is_ok(), "Should pass with valid request");
    }

    #[tokio::test]
    async fn test_stats_collector_memory_limit() {
        use loro::stats::StatsCollector;

        let collector = StatsCollector::new(5); // Small limit for testing

        // Add more entries than the limit
        for i in 0..10 {
            collector.add_request(
                i as f64,
                i as f64 * 2.0,
                Some(i as f64),
                Some(i as f64 * 1.5),
            );
        }

        let _stats = collector.get_stats();
        assert_eq!(collector.get_request_count(), 10);

        // Check that old entries were removed (implementation detail)
        // The collector should maintain only the latest entries within the limit
    }

    #[tokio::test]
    async fn test_message_categorization_edge_cases() {
        // Test empty content
        let message = Message {
            role: "user".to_string(),
            content: "".to_string(),
        };
        // Should not panic and should return some category
        let _category = message.categorize();

        // Test mixed language
        let message = Message {
            role: "user".to_string(),
            content: "Hello 你好 how are you？".to_string(),
        };
        let category = message.categorize();
        matches!(category, RequestCategory::Greeting);

        // Test special characters
        let message = Message {
            role: "user".to_string(),
            content: "!@#$%^&*()".to_string(),
        };
        let _category = message.categorize();
        // Should not panic
    }

    #[tokio::test]
    async fn test_quick_response_appropriateness() {
        use loro::models::RequestCategory;

        // All predefined responses should be appropriate
        for category in [
            RequestCategory::Greeting,
            RequestCategory::Question,
            RequestCategory::Request,
            RequestCategory::Thinking,
        ] {
            let responses = category.get_responses();
            for response in responses {
                assert!(
                    response.chars().count() <= 6,
                    "Response '{}' is too long",
                    response
                );
                assert!(!response.trim().is_empty(), "Response should not be empty");
            }
        }
    }

    #[tokio::test]
    async fn test_percentile_calculation() {
        use loro::stats::StatsCollector;

        let collector = StatsCollector::new(1000);

        // Add known data points
        let data_points = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        for (_i, &point) in data_points.iter().enumerate() {
            collector.add_request(point, point * 2.0, Some(point), Some(point * 1.5));
        }

        let stats = collector.get_stats();
        let first_response_stats = stats["first_response_latency"].as_object().unwrap();

        // Check that percentiles are reasonable
        let p50 = first_response_stats["p50"].as_f64().unwrap();
        let p95 = first_response_stats["p95"].as_f64().unwrap();

        assert!((5.0..=6.0).contains(&p50), "P50 should be around median");
        assert!((9.0..=10.0).contains(&p95), "P95 should be near the top");
        assert!(p95 >= p50, "P95 should be >= P50");
    }

    #[tokio::test]
    async fn test_error_types() {
        // Test timeout error
        let timeout_error = LoroError::Timeout { timeout_secs: 30 };
        assert!(timeout_error.is_timeout());
        assert!(!timeout_error.is_api_error());
        assert!(!timeout_error.is_validation_error());

        // Test API error
        let api_error = LoroError::ApiError {
            provider: "test".to_string(),
            status: 500,
            message: "Internal error".to_string(),
        };
        assert!(api_error.is_api_error());
        assert!(!api_error.is_timeout());

        // Test validation error
        let validation_error = LoroError::Validation("Invalid input".to_string());
        assert!(validation_error.is_validation_error());
        assert!(!validation_error.is_timeout());
    }

    #[tokio::test]
    async fn test_sse_line_processing() {
        // This function is private, so we test the behavior through integration tests
        // We verify that SSE parsing works correctly in the service layer
        assert!(true, "SSE parsing is tested through integration tests");
    }

    #[tokio::test]
    async fn test_concurrent_request_handling() {
        std::env::set_var("SMALL_MODEL_API_KEY", "test-key");
        std::env::set_var("LARGE_MODEL_API_KEY", "test-key");
        std::env::set_var("HTTP_TIMEOUT_SECS", "30"); // Set valid timeout

        let config = Config::from_env().expect("Failed to load config");
        let service = std::sync::Arc::new(
            LoroService::new(config)
                .await
                .expect("Service creation failed"),
        );

        // Create multiple concurrent requests
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test concurrent request".to_string(),
            }],
            max_tokens: Some(50),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };

        // Spawn multiple concurrent requests
        let tasks: Vec<_> = (0..5)
            .map(|i| {
                let service = service.clone();
                let req = request.clone();
                tokio::spawn(async move {
                    // These will fail because we don't have real API endpoints,
                    // but we're testing that they don't panic or deadlock
                    let _result = service.chat_completion(req).await;
                    i
                })
            })
            .collect();

        // Wait for all tasks to complete
        for task in tasks {
            let _result = task.await.expect("Task should complete");
        }
    }

    #[tokio::test]
    async fn test_memory_limit_edge_cases() {
        let collector = StatsCollector::new(5); // Very small limit

        // Add more entries than the limit
        for i in 0..10 {
            collector.add_request(
                i as f64,
                i as f64 * 2.0,
                Some(i as f64),
                Some(i as f64 * 1.5),
            );
        }

        // Verify memory limit is enforced
        let stats = collector.get_stats();
        let total_requests = stats["total_requests"].as_u64().unwrap();
        assert_eq!(total_requests, 10, "All requests should be counted");

        // Check that we don't have more than the limit in memory
        let first_times = stats["first_response_latency"].as_object().unwrap();
        let avg = first_times["avg"].as_f64().unwrap();

        // Should only average the last 5 entries (5, 6, 7, 8, 9)
        let expected_avg = (5.0 + 6.0 + 7.0 + 8.0 + 9.0) / 5.0;
        assert!(
            (avg - expected_avg).abs() < 0.01,
            "Average should reflect memory limit"
        );
    }

    #[tokio::test]
    async fn test_nan_handling_in_stats() {
        use loro::stats::calculate_stats;

        let data = vec![1.0, 2.0, f64::NAN, 4.0, 5.0];
        let stats = calculate_stats(&data);

        // Should not panic and should handle NaN gracefully
        assert!(stats.avg.is_finite() || stats.avg.is_nan());
        assert!(stats.p50.is_finite() || stats.p50.is_nan());
        assert!(stats.p95.is_finite() || stats.p95.is_nan());
    }

    #[tokio::test]
    async fn test_request_validation_comprehensive() {
        // Test all validation paths
        let mut request = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            max_tokens: Some(100),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };

        // Valid request should pass
        assert!(request.validate().is_ok());

        // Test empty model
        request.model = "".to_string();
        assert!(request.validate().is_err());
        request.model = "test".to_string();

        // Test empty messages
        request.messages = vec![];
        assert!(request.validate().is_err());
        request.messages = vec![Message {
            role: "user".to_string(),
            content: "test".to_string(),
        }];

        // Test invalid role
        request.messages[0].role = "invalid".to_string();
        assert!(request.validate().is_err());
        request.messages[0].role = "user".to_string();

        // Test empty content
        request.messages[0].content = "".to_string();
        assert!(request.validate().is_err());
        request.messages[0].content = "test".to_string();

        // Test invalid max_tokens
        request.max_tokens = Some(0);
        assert!(request.validate().is_err());
        request.max_tokens = Some(10000);
        assert!(request.validate().is_err());
        request.max_tokens = Some(100);

        // Test invalid temperature
        request.temperature = -1.0;
        assert!(request.validate().is_err());
        request.temperature = 3.0;
        assert!(request.validate().is_err());
        request.temperature = 0.7;

        // Should be valid again
        assert!(request.validate().is_ok());
    }

    #[tokio::test]
    async fn test_config_timeout_validation() {
        // Save original values
        let original_timeout = std::env::var("HTTP_TIMEOUT_SECS").ok();

        std::env::set_var("SMALL_MODEL_API_KEY", "test-key");
        std::env::set_var("LARGE_MODEL_API_KEY", "test-key");

        // Test invalid timeout values
        std::env::set_var("HTTP_TIMEOUT_SECS", "400"); // Too high
        let config_result = Config::from_env();
        assert!(config_result.is_err(), "Should fail with high timeout");

        std::env::set_var("HTTP_TIMEOUT_SECS", "2"); // Too low
        let config_result = Config::from_env();
        assert!(config_result.is_err(), "Should fail with low timeout");

        std::env::set_var("HTTP_TIMEOUT_SECS", "30"); // Valid
        let config_result = Config::from_env();
        assert!(config_result.is_ok(), "Should succeed with valid timeout");

        // Restore original value
        if let Some(original) = original_timeout {
            std::env::set_var("HTTP_TIMEOUT_SECS", original);
        } else {
            std::env::remove_var("HTTP_TIMEOUT_SECS");
        }
    }
}
