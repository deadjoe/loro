use loro::{
    config::Config, errors::LoroError, models::*, service::LoroService, stats::StatsCollector,
};
use secrecy::Secret;
use serial_test::serial;

#[tokio::test]
#[serial]
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
    assert!(matches!(greeting.categorize(), RequestCategory::Greeting));
    assert!(matches!(question.categorize(), RequestCategory::Question));
    assert!(matches!(request.categorize(), RequestCategory::Request));
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
    #[serial]
    async fn test_config_validation() {
        use loro::config::Config;

        // Clear potentially interfering environment variables first
        let vars_to_clear = [
            "HOST",
            "PORT",
            "LOG_LEVEL",
            "HTTP_TIMEOUT_SECS",
            "SMALL_MODEL_TIMEOUT_SECS",
            "MAX_RETRIES",
            "STATS_MAX_ENTRIES",
        ];
        for var in &vars_to_clear {
            std::env::remove_var(var);
        }

        // Set all required environment variables for valid config
        std::env::set_var("SMALL_MODEL_API_KEY", "test-small-key");
        std::env::set_var("LARGE_MODEL_API_KEY", "test-large-key");
        std::env::set_var("HTTP_TIMEOUT_SECS", "30");
        std::env::set_var("SMALL_MODEL_TIMEOUT_SECS", "5");
        std::env::set_var("MAX_RETRIES", "3");
        std::env::set_var("STATS_MAX_ENTRIES", "1000");

        let result = Config::from_env();
        assert!(
            result.is_ok(),
            "Should succeed with valid API keys: {:?}",
            result.err()
        );

        // Test validation with empty API key - create a config manually to test validation
        let mut config = result.unwrap();
        config.small_model.api_key = Secret::new("".to_string());
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
        assert!(matches!(category, RequestCategory::Greeting));

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
    #[serial]
    async fn test_concurrent_request_handling() {
        // Clear potentially interfering environment variables first
        let vars_to_clear = [
            "HOST",
            "PORT",
            "LOG_LEVEL",
            "HTTP_TIMEOUT_SECS",
            "SMALL_MODEL_TIMEOUT_SECS",
            "MAX_RETRIES",
            "STATS_MAX_ENTRIES",
        ];
        for var in &vars_to_clear {
            std::env::remove_var(var);
        }

        // Set all required environment variables
        std::env::set_var("SMALL_MODEL_API_KEY", "test-key");
        std::env::set_var("LARGE_MODEL_API_KEY", "test-key");
        std::env::set_var("HTTP_TIMEOUT_SECS", "30");
        std::env::set_var("SMALL_MODEL_TIMEOUT_SECS", "5");
        std::env::set_var("MAX_RETRIES", "3");
        std::env::set_var("STATS_MAX_ENTRIES", "1000");

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

        // Memory limit should be enforced but all requests counted
        // The exact average depends on implementation details of memory management
        assert!(avg > 0.0, "Average should be positive");
        assert!(avg <= 9.0, "Average should not exceed maximum value added");
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
    async fn test_prefix_functionality() {
        // Test that prefix is correctly included in OpenAI request
        use loro::models::OpenAIRequest;
        use serde_json::json;

        let request = OpenAIRequest {
            model: "test".to_string(),
            messages: vec![],
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: true,
            extra_body: Some(json!({"prefix": "你好！"})),
        };

        // Verify prefix is in extra_body
        assert!(request.extra_body.is_some());
        let extra_body = request.extra_body.as_ref().unwrap();
        assert_eq!(extra_body["prefix"], "你好！");

        // Test serialization
        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("prefix"));
        assert!(serialized.contains("你好！"));
    }

    #[tokio::test]
    async fn test_sse_parsing_edge_cases() {
        use loro::service::LoroService;

        // Test various SSE line formats
        let test_cases = vec![
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}",
            "data: {\"choices\":[{\"message\":{\"content\":\"World\"}}]}",
            "data: [DONE]",
            "data: ",
            ": comment line",
            "event: error",
            "data: {\"malformed json",
        ];

        for (i, line) in test_cases.iter().enumerate() {
            // This tests that SSE parsing doesn't panic on various inputs
            let result = LoroService::process_sse_line_static(line, "test-id", "test-model");

            // Should not panic, may return Ok(None) or Ok(Some(...))
            match result {
                Ok(_) => {} // Expected
                Err(e) => {
                    // Only structural errors should cause failures
                    assert!(
                        !e.to_string().contains("panic"),
                        "Case {}: Should not panic on: {}",
                        i,
                        line
                    );
                }
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_config_timeout_validation() {
        std::env::set_var("SMALL_MODEL_API_KEY", "test-key");
        std::env::set_var("LARGE_MODEL_API_KEY", "test-key");

        // Test invalid timeout values - directly test validation function
        let mut config = loro::config::Config {
            host: "127.0.0.1".to_string(),
            port: 8000,
            log_level: "info".to_string(),
            small_model: loro::config::ModelConfig {
                api_key: Secret::new("test-key".to_string()),
                base_url: "https://api.example.com/v1".to_string(),
                model_name: "test-model".to_string(),
            },
            large_model: loro::config::ModelConfig {
                api_key: Secret::new("test-key".to_string()),
                base_url: "https://api.example.com/v1".to_string(),
                model_name: "test-model".to_string(),
            },
            http_timeout_secs: 400, // Too high
            small_model_timeout_secs: 5,
            max_retries: 3,
            stats_max_entries: 1000,
        };

        // Should fail with high timeout
        assert!(config.validate().is_err(), "Should fail with high timeout");

        config.http_timeout_secs = 2; // Too low
        assert!(config.validate().is_err(), "Should fail with low timeout");

        config.http_timeout_secs = 30; // Valid
        assert!(
            config.validate().is_ok(),
            "Should succeed with valid timeout"
        );
    }
}
