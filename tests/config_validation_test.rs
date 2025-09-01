use loro::config::{Config, ModelConfig};
use secrecy::Secret;
use std::env;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_config_from_env_missing_keys() {
    // Clear required environment variables
    env::remove_var("SMALL_MODEL_API_KEY");
    env::remove_var("LARGE_MODEL_API_KEY");
    
    // Should fail when required keys are missing
    let result = Config::from_env();
    assert!(result.is_err());
}

#[tokio::test]
#[serial]
async fn test_config_from_env_valid() {
    // Set all required environment variables
    env::set_var("SMALL_MODEL_API_KEY", "test-small-key");
    env::set_var("LARGE_MODEL_API_KEY", "test-large-key");
    env::set_var("HOST", "127.0.0.1");
    env::set_var("PORT", "8080");
    
    let result = Config::from_env();
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
}

#[tokio::test]
#[serial]
async fn test_config_default_values() {
    // Set only required environment variables
    env::set_var("SMALL_MODEL_API_KEY", "test-small-key");
    env::set_var("LARGE_MODEL_API_KEY", "test-large-key");
    
    // Remove optional variables to test defaults
    env::remove_var("HOST");
    env::remove_var("PORT");
    env::remove_var("LOG_LEVEL");
    
    let result = Config::from_env();
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert_eq!(config.host, "0.0.0.0");  // Default host
    assert_eq!(config.port, 8000);       // Default port
    assert_eq!(config.log_level, "info"); // Default log level
}

#[tokio::test]
#[serial]
async fn test_config_invalid_port() {
    // Set environment variables with invalid port
    env::set_var("SMALL_MODEL_API_KEY", "test-small-key");
    env::set_var("LARGE_MODEL_API_KEY", "test-large-key");
    env::set_var("PORT", "invalid_port");
    
    let result = Config::from_env();
    assert!(result.is_err());
}

#[test]
fn test_model_config_debug_hides_secret() {
    let model_config = ModelConfig {
        api_key: Secret::new("secret-key-123".to_string()),
        base_url: "https://api.example.com/v1".to_string(),
        model_name: "test-model".to_string(),
    };
    
    let debug_output = format!("{:?}", model_config);
    
    // Should not contain the actual secret
    assert!(!debug_output.contains("secret-key-123"));
    // Should contain redacted indicator
    assert!(debug_output.contains("[REDACTED"));
}

#[test] 
fn test_config_clone() {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 8080,
        log_level: "debug".to_string(),
        small_model: ModelConfig {
            api_key: Secret::new("small-key".to_string()),
            base_url: "https://small.api.com/v1".to_string(),
            model_name: "small-model".to_string(),
        },
        large_model: ModelConfig {
            api_key: Secret::new("large-key".to_string()),
            base_url: "https://large.api.com/v1".to_string(),
            model_name: "large-model".to_string(),
        },
        http_timeout_secs: 30,
        small_model_timeout_secs: 10,
        max_retries: 3,
        stats_max_entries: 1000,
    };
    
    let cloned_config = config.clone();
    assert_eq!(config.host, cloned_config.host);
    assert_eq!(config.port, cloned_config.port);
    assert_eq!(config.log_level, cloned_config.log_level);
}

#[test]
fn test_config_validation_empty_api_key() {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 8080,
        log_level: "debug".to_string(),
        small_model: ModelConfig {
            api_key: Secret::new("".to_string()), // Empty key
            base_url: "https://api.example.com/v1".to_string(),
            model_name: "test-model".to_string(),
        },
        large_model: ModelConfig {
            api_key: Secret::new("valid-key".to_string()),
            base_url: "https://api.example.com/v1".to_string(),
            model_name: "test-model".to_string(),
        },
        http_timeout_secs: 30,
        small_model_timeout_secs: 10,
        max_retries: 3,
        stats_max_entries: 1000,
    };
    
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_invalid_url() {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 8080,
        log_level: "debug".to_string(),
        small_model: ModelConfig {
            api_key: Secret::new("valid-key".to_string()),
            base_url: "invalid-url".to_string(), // Invalid URL
            model_name: "test-model".to_string(),
        },
        large_model: ModelConfig {
            api_key: Secret::new("valid-key".to_string()),
            base_url: "https://api.example.com/v1".to_string(),
            model_name: "test-model".to_string(),
        },
        http_timeout_secs: 30,
        small_model_timeout_secs: 10,
        max_retries: 3,
        stats_max_entries: 1000,
    };
    
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_validation_timeout_ranges() {
    // Test invalid timeout values
    let mut config = Config {
        host: "127.0.0.1".to_string(),
        port: 8080,
        log_level: "debug".to_string(),
        small_model: ModelConfig {
            api_key: Secret::new("valid-key".to_string()),
            base_url: "https://api.example.com/v1".to_string(),
            model_name: "test-model".to_string(),
        },
        large_model: ModelConfig {
            api_key: Secret::new("valid-key".to_string()),
            base_url: "https://api.example.com/v1".to_string(),
            model_name: "test-model".to_string(),
        },
        http_timeout_secs: 30,
        small_model_timeout_secs: 10,
        max_retries: 3,
        stats_max_entries: 1000,
    };
    
    // Test invalid http timeout (too low)
    config.http_timeout_secs = 1;
    assert!(config.validate().is_err());
    
    // Test invalid http timeout (too high)
    config.http_timeout_secs = 500;
    assert!(config.validate().is_err());
    
    // Test valid timeout
    config.http_timeout_secs = 30;
    assert!(config.validate().is_ok());
}
