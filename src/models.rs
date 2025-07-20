use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_stream")]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    // Custom parameter to disable quick response for comparison
    #[serde(default)]
    pub disable_quick_response: bool,
}

impl ChatCompletionRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.model.trim().is_empty() {
            return Err("Model name cannot be empty".to_string());
        }

        if self.messages.is_empty() {
            return Err("Messages array cannot be empty".to_string());
        }

        for (i, message) in self.messages.iter().enumerate() {
            if message.role.trim().is_empty() {
                return Err(format!("Message {i} role cannot be empty"));
            }
            if message.content.trim().is_empty() {
                return Err(format!("Message {i} content cannot be empty"));
            }
            if !["system", "user", "assistant"].contains(&message.role.as_str()) {
                return Err(format!("Message {i} has invalid role: {}", message.role));
            }
        }

        if let Some(max_tokens) = self.max_tokens {
            if max_tokens == 0 || max_tokens > 8192 {
                return Err("max_tokens must be between 1 and 8192".to_string());
            }
        }

        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err("Temperature must be between 0.0 and 2.0".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Stop {
    Single(String),
    Multiple(Vec<String>),
}

fn default_temperature() -> f32 {
    0.7
}

fn default_stream() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceDelta {
    pub index: u32,
    pub delta: MessageDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChoiceDelta>,
}

// OpenAI API request structures for external calls
#[derive(Debug, Clone, Serialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_body: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIResponse {
    pub choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIChoice {
    pub message: Option<OpenAIMessage>,
    pub delta: Option<OpenAIMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// Quick response categories for voice assistant
#[derive(Debug, Clone)]
pub enum RequestCategory {
    Greeting,
    Question,
    Request,
    Thinking,
}

impl RequestCategory {
    pub fn get_responses(&self) -> &'static [&'static str] {
        match self {
            RequestCategory::Greeting => &["你好！", "嗨！", "您好，", "我在，"],
            RequestCategory::Question => &["好的，", "这个，", "关于，", "我来，"],
            RequestCategory::Request => &["好的，", "明白，", "我来，", "让我，"],
            RequestCategory::Thinking => &["嗯，", "我觉得，", "让我，", "根据，"],
        }
    }
}

impl Message {
    pub fn categorize(&self) -> RequestCategory {
        let content = self.content.to_lowercase();

        // Greeting patterns
        if content.contains("你好")
            || content.contains("hello")
            || content.contains("hi")
            || content.contains("嗨")
        {
            RequestCategory::Greeting
        }
        // Question patterns
        else if content.contains("什么")
            || content.contains("如何")
            || content.contains("怎么")
            || content.contains("为什么")
            || content.contains("why")
            || content.contains("how")
            || content.contains("what")
            || content.contains("?")
            || content.contains("？")
        {
            RequestCategory::Question
        }
        // Request patterns
        else if content.contains("请")
            || content.contains("帮我")
            || content.contains("能不能")
            || content.contains("可以")
            || content.contains("help")
            || content.contains("please")
        {
            RequestCategory::Request
        } else {
            RequestCategory::Thinking
        }
    }
}
