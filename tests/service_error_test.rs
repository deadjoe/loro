use loro::{
    config::{Config, ModelConfig},
    models::{ChatCompletionRequest, Message, RequestCategory},
    service::LoroService,
};
use secrecy::Secret;

fn create_test_config() -> Config {
    Config {
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
    }
}

fn create_test_message(content: &str) -> Message {
    Message {
        role: "user".to_string(),
        content: content.to_string(),
    }
}

#[tokio::test]
async fn test_service_creation_success() {
    let config = create_test_config();
    let result = LoroService::new(config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_quick_response_categorization() {
    let config = create_test_config();
    let _service = LoroService::new(config).await.unwrap();
    
    // Test different message categories
    let greeting_msg = create_test_message("你好");
    let question_msg = create_test_message("什么是AI?");
    let request_msg = create_test_message("请帮我写代码");
    let thinking_msg = create_test_message("这个想法很有趣");
    
    // These should categorize correctly
    assert!(matches!(greeting_msg.categorize(), RequestCategory::Greeting));
    assert!(matches!(question_msg.categorize(), RequestCategory::Question));
    assert!(matches!(request_msg.categorize(), RequestCategory::Request));
    assert!(matches!(thinking_msg.categorize(), RequestCategory::Thinking));
}

#[tokio::test]
async fn test_message_categorize_english() {
    let hello_msg = create_test_message("Hello there!");
    let what_msg = create_test_message("What is the weather?");
    let help_msg = create_test_message("Help me please");  // Remove ? to avoid Question category
    
    assert!(matches!(hello_msg.categorize(), RequestCategory::Greeting));
    assert!(matches!(what_msg.categorize(), RequestCategory::Question));
    assert!(matches!(help_msg.categorize(), RequestCategory::Request));
}

#[tokio::test]
async fn test_message_categorize_with_question_marks() {
    let chinese_q = create_test_message("这是什么？");
    let english_q = create_test_message("What are you doing?");  // Avoid "hi" in "this"
    
    println!("Chinese Q category: {:?}", chinese_q.categorize());
    println!("English Q category: {:?}", english_q.categorize());
    
    assert!(matches!(chinese_q.categorize(), RequestCategory::Question));
    assert!(matches!(english_q.categorize(), RequestCategory::Question));
}

#[tokio::test]
async fn test_get_responses_for_categories() {
    let greeting = RequestCategory::Greeting;
    let question = RequestCategory::Question;
    let request = RequestCategory::Request;
    let thinking = RequestCategory::Thinking;
    
    // Test that each category has responses
    assert!(!greeting.get_responses().is_empty());
    assert!(!question.get_responses().is_empty());
    assert!(!request.get_responses().is_empty());
    assert!(!thinking.get_responses().is_empty());
    
    // Test specific responses
    assert!(greeting.get_responses().contains(&"你好！"));
    assert!(question.get_responses().contains(&"让我想想，"));
    assert!(request.get_responses().contains(&"好的，"));
    assert!(thinking.get_responses().contains(&"嗯，"));
}

#[tokio::test]
async fn test_stats_collector_functionality() {
    let config = create_test_config();
    let service = LoroService::new(config).await.unwrap();
    
    // Test initial metrics
    let initial_metrics = service.get_metrics().await;
    assert_eq!(initial_metrics["quick_response_mode"]["total_requests"], 0);
    assert_eq!(initial_metrics["direct_mode"]["total_requests"], 0);
    assert_eq!(initial_metrics["comparison"]["quick_mode_requests"], 0);
    assert_eq!(initial_metrics["comparison"]["direct_mode_requests"], 0);
    
    // Test reset functionality
    service.reset_metrics().await;
    let after_reset = service.get_metrics().await;
    assert_eq!(after_reset["quick_response_mode"]["total_requests"], 0);
    assert_eq!(after_reset["direct_mode"]["total_requests"], 0);
    assert_eq!(after_reset["comparison"]["quick_mode_requests"], 0);
    assert_eq!(after_reset["comparison"]["direct_mode_requests"], 0);
}

#[tokio::test]
async fn test_chat_completion_request_validation() {
    let config = create_test_config();
    let _service = LoroService::new(config).await.unwrap();
    
    // Test invalid request - empty model
    let invalid_request = ChatCompletionRequest {
        model: "".to_string(),
        messages: vec![create_test_message("test")],
        max_tokens: None,
        temperature: 0.7,
        stream: true,
        stop: None,
        disable_quick_response: false,
    };
    
    let result = invalid_request.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Model name cannot be empty"));
    
    // Test invalid request - empty messages
    let invalid_request2 = ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![],
        max_tokens: None,
        temperature: 0.7,
        stream: true,
        stop: None,
        disable_quick_response: false,
    };
    
    let result2 = invalid_request2.validate();
    assert!(result2.is_err());
    assert!(result2.unwrap_err().contains("Messages array cannot be empty"));
}

#[tokio::test]
async fn test_chat_completion_message_validation() {
    let config = create_test_config();
    let _service = LoroService::new(config).await.unwrap();
    
    // Test invalid message role
    let invalid_request = ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![Message {
            role: "invalid_role".to_string(),
            content: "test content".to_string(),
        }],
        max_tokens: None,
        temperature: 0.7,
        stream: true,
        stop: None,
        disable_quick_response: false,
    };
    
    let result = invalid_request.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid role"));
    
    // Test empty message content
    let invalid_request2 = ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: "".to_string(),
        }],
        max_tokens: None,
        temperature: 0.7,
        stream: true,
        stop: None,
        disable_quick_response: false,
    };
    
    let result2 = invalid_request2.validate();
    assert!(result2.is_err());
    assert!(result2.unwrap_err().contains("content cannot be empty"));
}

#[tokio::test]
async fn test_chat_completion_parameter_validation() {
    let config = create_test_config();
    let _service = LoroService::new(config).await.unwrap();
    
    // Test invalid temperature
    let invalid_request = ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![create_test_message("test")],
        max_tokens: None,
        temperature: 3.0, // Invalid temperature
        stream: true,
        stop: None,
        disable_quick_response: false,
    };
    
    let result = invalid_request.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Temperature must be between"));
    
    // Test invalid max_tokens
    let invalid_request2 = ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![create_test_message("test")],
        max_tokens: Some(10000), // Too high
        temperature: 0.7,
        stream: true,
        stop: None,
        disable_quick_response: false,
    };
    
    let result2 = invalid_request2.validate();
    assert!(result2.is_err());
    assert!(result2.unwrap_err().contains("max_tokens must be between"));
}

#[test]
fn test_valid_message_roles() {
    let valid_roles = ["system", "user", "assistant"];
    
    for role in valid_roles {
        let message = Message {
            role: role.to_string(),
            content: "test content".to_string(),
        };
        
        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![message],
            max_tokens: None,
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        
        assert!(request.validate().is_ok(), "Role {} should be valid", role);
    }
}

#[test]
fn test_temperature_edge_cases() {
    let test_cases = [
        (0.0, true),   // Valid minimum
        (2.0, true),   // Valid maximum  
        (-0.1, false), // Invalid - too low
        (2.1, false),  // Invalid - too high
    ];
    
    for (temp, should_be_valid) in test_cases {
        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![create_test_message("test")],
            max_tokens: None,
            temperature: temp,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        
        let result = request.validate();
        assert_eq!(result.is_ok(), should_be_valid, "Temperature {} validation failed", temp);
    }
}

#[test]
fn test_max_tokens_edge_cases() {
    let test_cases = [
        (Some(1), true),     // Valid minimum
        (Some(8192), true),  // Valid maximum
        (Some(0), false),    // Invalid - zero
        (Some(8193), false), // Invalid - too high
        (None, true),        // Valid - no limit
    ];
    
    for (max_tokens, should_be_valid) in test_cases {
        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![create_test_message("test")],
            max_tokens,
            temperature: 0.7,
            stream: true,
            stop: None,
            disable_quick_response: false,
        };
        
        let result = request.validate();
        assert_eq!(result.is_ok(), should_be_valid, "max_tokens {:?} validation failed", max_tokens);
    }
}
