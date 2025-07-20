use serde::{Deserialize, Serialize};
use std::env;
use anyhow::{Result, Context};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub small_model: ModelConfig,
    pub large_model: ModelConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        let small_model = ModelConfig {
            api_key: env::var("SMALL_MODEL_API_KEY")
                .context("SMALL_MODEL_API_KEY environment variable is required")?,
            base_url: env::var("SMALL_MODEL_BASE_URL")
                .unwrap_or_else(|_| "https://api.siliconflow.cn/v1".to_string()),
            model_name: env::var("SMALL_MODEL_NAME")
                .unwrap_or_else(|_| "Qwen/Qwen2-1.5B-Instruct".to_string()),
        };

        let large_model = ModelConfig {
            api_key: env::var("LARGE_MODEL_API_KEY")
                .context("LARGE_MODEL_API_KEY environment variable is required")?,
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
        };

        Ok(config)
    }
}