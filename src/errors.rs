use thiserror::Error;

#[derive(Error, Debug)]
pub enum LoroError {
    #[error("Configuration error: {0}")]
    Config(#[from] anyhow::Error),

    #[error("HTTP request timeout: {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    #[error("API error from {provider}: {status} - {message}")]
    ApiError {
        provider: String,
        status: u16,
        message: String,
    },

    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Small model failed: {0}")]
    SmallModelFailed(String),

    #[error("Large model failed: {0}")]
    LargeModelFailed(String),

    #[error("Stream processing error: {0}")]
    StreamProcessing(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl LoroError {
    pub fn is_timeout(&self) -> bool {
        matches!(self, LoroError::Timeout { .. })
    }

    pub fn is_api_error(&self) -> bool {
        matches!(self, LoroError::ApiError { .. })
    }

    pub fn is_validation_error(&self) -> bool {
        matches!(self, LoroError::Validation(_))
    }
}
