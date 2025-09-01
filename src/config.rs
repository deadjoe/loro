use anyhow::{Context, Result};
use secrecy::{Secret, ExposeSecret};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub small_model: ModelConfig,
    pub large_model: ModelConfig,
    pub http_timeout_secs: u64,
    pub small_model_timeout_secs: u64,
    pub max_retries: u32,
    pub stats_max_entries: usize,
}

#[derive(Clone)]
pub struct ModelConfig {
    pub api_key: Secret<String>,
    pub base_url: String,
    pub model_name: String,
}

// Custom Debug implementation to hide API keys
impl std::fmt::Debug for ModelConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelConfig")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("model_name", &self.model_name)
            .finish()
    }
}

// Custom Serialize implementation that excludes secrets
impl Serialize for ModelConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ModelConfig", 3)?;
        state.serialize_field("api_key", "[REDACTED]")?;
        state.serialize_field("base_url", &self.base_url)?;
        state.serialize_field("model_name", &self.model_name)?;
        state.end()
    }
}

// Custom Deserialize implementation (for completeness, though not used in practice)
impl<'de> Deserialize<'de> for ModelConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ModelConfigHelper {
            api_key: String,
            base_url: String,
            model_name: String,
        }
        
        let helper = ModelConfigHelper::deserialize(deserializer)?;
        Ok(ModelConfig {
            api_key: Secret::new(helper.api_key),
            base_url: helper.base_url,
            model_name: helper.model_name,
        })
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        let small_model = ModelConfig {
            api_key: Secret::new(env::var("SMALL_MODEL_API_KEY")
                .context("SMALL_MODEL_API_KEY environment variable is required")?),
            base_url: env::var("SMALL_MODEL_BASE_URL")
                .unwrap_or_else(|_| "https://api.siliconflow.cn/v1".to_string()),
            model_name: env::var("SMALL_MODEL_NAME")
                .unwrap_or_else(|_| "Qwen/Qwen2-1.5B-Instruct".to_string()),
        };

        let large_model = ModelConfig {
            api_key: Secret::new(env::var("LARGE_MODEL_API_KEY")
                .context("LARGE_MODEL_API_KEY environment variable is required")?),
            base_url: env::var("LARGE_MODEL_BASE_URL")
                .unwrap_or_else(|_| "https://api.siliconflow.cn/v1".to_string()),
            model_name: env::var("LARGE_MODEL_NAME")
                .unwrap_or_else(|_| "deepseek-ai/DeepSeek-V2.5".to_string()),
        };

        let config = Config {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .context("PORT must be a valid number")?,
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            small_model,
            large_model,
            http_timeout_secs: env::var("HTTP_TIMEOUT_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .context("HTTP_TIMEOUT_SECS must be a valid number")?,
            small_model_timeout_secs: env::var("SMALL_MODEL_TIMEOUT_SECS")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .context("SMALL_MODEL_TIMEOUT_SECS must be a valid number")?,
            max_retries: env::var("MAX_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .context("MAX_RETRIES must be a valid number")?,
            stats_max_entries: env::var("STATS_MAX_ENTRIES")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .context("STATS_MAX_ENTRIES must be a valid number")?,
        };

        // Validate configuration
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        // Validate API keys are not empty (allow "none" for local services like Ollama)
        if self.small_model.api_key.expose_secret().trim().is_empty() {
            return Err(anyhow::anyhow!("SMALL_MODEL_API_KEY cannot be empty"));
        }
        if self.large_model.api_key.expose_secret().trim().is_empty() {
            return Err(anyhow::anyhow!("LARGE_MODEL_API_KEY cannot be empty"));
        }

        // For local services (like Ollama), API key can be "none"
        // No additional validation needed for API key content

        // Note: "none" is accepted as a valid API key for local services like Ollama

        // Validate URLs
        if !self.small_model.base_url.starts_with("http") {
            return Err(anyhow::anyhow!(
                "SMALL_MODEL_BASE_URL must be a valid HTTP(S) URL"
            ));
        }
        if !self.large_model.base_url.starts_with("http") {
            return Err(anyhow::anyhow!(
                "LARGE_MODEL_BASE_URL must be a valid HTTP(S) URL"
            ));
        }

        // Validate model names
        if self.small_model.model_name.trim().is_empty() {
            return Err(anyhow::anyhow!("SMALL_MODEL_NAME cannot be empty"));
        }
        if self.large_model.model_name.trim().is_empty() {
            return Err(anyhow::anyhow!("LARGE_MODEL_NAME cannot be empty"));
        }

        // Validate timeouts and limits
        if self.http_timeout_secs < 5 || self.http_timeout_secs > 300 {
            return Err(anyhow::anyhow!(
                "HTTP_TIMEOUT_SECS must be between 5 and 300 seconds"
            ));
        }
        if self.small_model_timeout_secs < 1 || self.small_model_timeout_secs > 30 {
            return Err(anyhow::anyhow!(
                "SMALL_MODEL_TIMEOUT_SECS must be between 1 and 30 seconds"
            ));
        }
        if self.max_retries > 10 {
            return Err(anyhow::anyhow!("MAX_RETRIES must be <= 10"));
        }
        if self.stats_max_entries < 100 || self.stats_max_entries > 100000 {
            return Err(anyhow::anyhow!(
                "STATS_MAX_ENTRIES must be between 100 and 100000"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_validation() {
        let mut config = Config {
            host: "127.0.0.1".to_string(),
            port: 8000,
            log_level: "info".to_string(),
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
            small_model_timeout_secs: 5,
            max_retries: 3,
            stats_max_entries: 1000,
        };

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Test empty API key
        config.small_model.api_key = Secret::new("".to_string());
        assert!(config.validate().is_err());
        config.small_model.api_key = Secret::new("valid-key".to_string());

        // Test invalid URL
        config.large_model.base_url = "not-a-url".to_string();
        assert!(config.validate().is_err());
        config.large_model.base_url = "https://api.example.com/v1".to_string();

        // Test invalid timeout
        config.http_timeout_secs = 400; // Too high
        assert!(config.validate().is_err());
        config.http_timeout_secs = 2; // Too low
        assert!(config.validate().is_err());
        config.http_timeout_secs = 30;

        // Test invalid stats limit
        config.stats_max_entries = 50; // Too low
        assert!(config.validate().is_err());
        config.stats_max_entries = 200000; // Too high
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_from_explicit_values() {
        // Test that the from_env function works with explicit environment variables
        // Set required env vars
        env::set_var("SMALL_MODEL_API_KEY", "test-small-key");
        env::set_var("LARGE_MODEL_API_KEY", "test-large-key");

        // Set optional env vars to specific values
        env::set_var("HOST", "192.168.1.1");
        env::set_var("PORT", "9000");
        env::set_var("HTTP_TIMEOUT_SECS", "60");
        env::set_var("SMALL_MODEL_TIMEOUT_SECS", "10");
        env::set_var("MAX_RETRIES", "5");
        env::set_var("STATS_MAX_ENTRIES", "5000");

        let config = Config::from_env().unwrap();

        assert_eq!(config.host, "192.168.1.1");
        assert_eq!(config.port, 9000);
        assert_eq!(config.http_timeout_secs, 60);
        assert_eq!(config.small_model_timeout_secs, 10);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.stats_max_entries, 5000);
        assert_eq!(config.small_model.api_key.expose_secret(), "test-small-key");
        assert_eq!(config.large_model.api_key.expose_secret(), "test-large-key");
    }
}
