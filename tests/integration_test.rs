use loro::{config::Config, service::LoroService, models::*};

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
        assert!(responses.iter().all(|r| r.chars().count() <= 6), "Responses should be short");
    }
}

#[tokio::test]
async fn test_stats_collector() {
    use loro::stats::StatsCollector;
    
    let collector = StatsCollector::new();
    
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
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                }
            ],
            max_tokens: Some(100),
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        
        let json = serde_json::to_string(&request).expect("Should serialize");
        let parsed: ChatCompletionRequest = serde_json::from_str(&json).expect("Should deserialize");
        
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
            choices: vec![
                ChoiceDelta {
                    index: 0,
                    delta: MessageDelta {
                        role: Some("assistant".to_string()),
                        content: Some("Hello!".to_string()),
                    },
                    finish_reason: None,
                }
            ],
        };
        
        let json = serde_json::to_string(&chunk).expect("Should serialize chunk");
        let parsed: ChatCompletionChunk = serde_json::from_str(&json).expect("Should deserialize chunk");
        
        assert_eq!(chunk.id, parsed.id);
        assert_eq!(chunk.model, parsed.model);
        assert_eq!(chunk.choices.len(), parsed.choices.len());
    }
}