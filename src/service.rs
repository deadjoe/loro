use crate::{config::Config, models::*, stats::StatsCollector, errors::LoroError};
use secrecy::ExposeSecret;
use anyhow::{Context, Result};
use axum::response::{IntoResponse, Response, Sse};
use futures::stream::{self, Stream, StreamExt};
use rand::seq::SliceRandom;
use reqwest::Client;
use serde_json::json;
use std::pin::Pin;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Static constants to reduce string allocations
const SYSTEM_ROLE: &str = "system";
const USER_ROLE: &str = "user";
const ASSISTANT_ROLE: &str = "assistant";
const CHUNK_OBJECT: &str = "chat.completion.chunk";
const STOP_REASON: &str = "stop";
const QUICK_SYSTEM_PROMPT: &str = "/no_think 你是一个AI语音助手。请用1-3个字的简短语气词回应用户，比如：'你好！'、'好的，'、'嗯，'、'让我想想，'，要自然像真人对话。只输出语气词，不要完整回答。";
const LARGE_SYSTEM_PROMPT: &str =
    "你是一个友好的AI语音助手，用自然对话的方式回应用户。回答要简洁明了，适合语音交互。";

pub struct LoroService {
    config: Config,
    client: Client,
    quick_stats: Arc<StatsCollector>,
    direct_stats: Arc<StatsCollector>,
}

impl LoroService {
    pub async fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.http_timeout_secs))
            .connect_timeout(Duration::from_secs(5)) // Increased for better reliability
            .pool_max_idle_per_host(20) // Increased for higher concurrency
            .pool_idle_timeout(Duration::from_secs(60)) // Increased for connection reuse
            .tcp_keepalive(Duration::from_secs(30)) // Added keepalive for better performance
            .tcp_nodelay(true) // Disable Nagle's algorithm for lower latency
            .no_proxy() // Disable proxy for direct localhost connections
            .build()
            .context("Failed to create HTTP client")?;

        info!(
            "Loro service initialized with small model: {}",
            config.small_model.model_name
        );
        info!("Large model: {}", config.large_model.model_name);

        Ok(Self {
            config: config.clone(),
            client,
            quick_stats: Arc::new(StatsCollector::new(config.stats_max_entries)),
            direct_stats: Arc::new(StatsCollector::new(config.stats_max_entries)),
        })
    }

    pub async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<Response> {
        let disable_quick = request.disable_quick_response;

        debug!(
            "Processing chat completion request, disable_quick: {}",
            disable_quick
        );

        let stream: Pin<Box<dyn Stream<Item = Result<String>> + Send>> = if disable_quick {
            Box::pin(self.stream_direct_response(request).await?)
        } else {
            Box::pin(self.stream_quick_response(request).await?)
        };

        let sse_stream = stream.map(|chunk| match chunk {
            Ok(data) => Ok::<_, anyhow::Error>(axum::response::sse::Event::default().data(data)),
            Err(e) => {
                error!("Stream error: {}", e);
                // 仅输出负载，由 axum SSE 封装 data: 前缀
                Ok(axum::response::sse::Event::default().data(format!("[ERROR: {}]", e)))
            }
        });

        Ok(Sse::new(sse_stream).into_response())
    }

    async fn stream_quick_response(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_start = Instant::now();
        let request_id = Uuid::new_v4().to_string();

        // Clone request data for concurrent access
        let messages = request.messages.clone();
        let model_name = request.model.clone();

        // Step 1: Get quick response first
        let quick_start = Instant::now();
        let quick_response = self.get_quick_response(&messages).await?;
        let quick_time = quick_start.elapsed().as_secs_f64();

        debug!(
            "Quick response generated in {:.3}s: '{}'",
            quick_time, quick_response
        );

        // Create first chunk with quick response
        let first_chunk = ChatCompletionChunk {
            id: format!("chatcmpl-{request_id}"),
            object: CHUNK_OBJECT.to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model_name,
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: Some(ASSISTANT_ROLE.to_string()),
                    content: Some(quick_response.clone()),
                },
                finish_reason: None,
            }],
        };

        // 构造 JSON 负载（不包含 SSE data: 前缀）
        let first_chunk_data = serde_json::to_string(&first_chunk)?;
        if first_chunk_data.len() > 1024 * 1024 {
            return Err(anyhow::anyhow!(
                "Response chunk too large: {} bytes",
                first_chunk_data.len()
            ));
        }

        // Step 2: Get large model stream with prefix
        let large_start = Instant::now();
        let large_stream = self
            .get_large_model_stream(request, Some(quick_response))
            .await?;

        let stats = Arc::clone(&self.quick_stats);
        let enhanced_stream = large_stream.map(|chunk_result| chunk_result);

        // 结束时记录统计并发送 [DONE]
        let end_event = {
            let stats = Arc::clone(&stats);
            async move {
                let total_time = request_start.elapsed().as_secs_f64();
                let large_time = large_start.elapsed().as_secs_f64();
                stats.add_request(quick_time, total_time, Some(quick_time), Some(large_time));
                Ok("[DONE]".to_string())
            }
        };

        // Combine quick response and large model stream
        let combined_stream = stream::once(async move { Ok(first_chunk_data) })
            .chain(enhanced_stream)
            .chain(stream::once(end_event));

        Ok(Box::pin(combined_stream))
    }

    async fn stream_direct_response(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_start = Instant::now();
        let large_stream = self.get_large_model_stream(request, None).await?;

        let stats = Arc::clone(&self.direct_stats);
        let first_time = Arc::new(std::sync::Mutex::new(None::<f64>));
        let first_time_clone = Arc::clone(&first_time);
        let enhanced_stream = large_stream.enumerate().map(move |(i, chunk_result)| {
            match chunk_result {
                Ok(chunk_data) => {
                    if i == 0 {
                        let mut guard = first_time_clone.lock().unwrap();
                        *guard = Some(request_start.elapsed().as_secs_f64());
                    }
                    Ok(chunk_data)
                }
                Err(e) => Err(e),
            }
        });

        let end_event = {
            let stats = Arc::clone(&stats);
            let first_time = Arc::clone(&first_time);
            async move {
                let total_time = request_start.elapsed().as_secs_f64();
                let first = first_time.lock().unwrap().unwrap_or(total_time);
                stats.add_request(first, total_time, None, Some(total_time));
                Ok("[DONE]".to_string())
            }
        };

        let final_stream = enhanced_stream.chain(stream::once(end_event));

        Ok(Box::pin(final_stream))
    }

    async fn get_quick_response(&self, messages: &[Message]) -> Result<String> {
        // Validate input - prevent panic
        if messages.is_empty() {
            return Err(anyhow::anyhow!("Messages array cannot be empty"));
        }

        // Try small model first
        match self.call_small_model(messages).await {
            Ok(response) => {
                if response.chars().count() <= 6 && self.is_appropriate_quick_response(&response) {
                    return Ok(response);
                }
            }
            Err(e) => {
                warn!("Small model failed: {}, using fallback", e);
            }
        }

        // Fallback to predefined responses
        let last_message = messages
            .last()
            .expect("Messages array should not be empty (already checked)");
        let category = last_message.categorize();
        let responses = category.get_responses();
        let mut rng = rand::thread_rng();
        let response = responses
            .choose(&mut rng)
            .expect("Predefined responses array should never be empty");
        Ok(response.to_string())
    }

    async fn call_small_model(&self, messages: &[Message]) -> Result<String> {
        if messages.is_empty() {
            return Err(anyhow::anyhow!("Messages array cannot be empty"));
        }

        let last_message = messages
            .last()
            .expect("Messages array should not be empty (already checked)");
        if last_message.content.trim().is_empty() {
            return Err(anyhow::anyhow!("Message content cannot be empty"));
        }

        // Use module-level constants to reduce allocations

        let prompt_messages = vec![
            HashMap::from([(SYSTEM_ROLE.to_string(), QUICK_SYSTEM_PROMPT.to_string())]),
            HashMap::from([(USER_ROLE.to_string(), last_message.content.clone())]),
        ];

        // Create request body appropriate for the target service
        let request_body = if self.config.small_model.base_url.contains("11434") {
            // Ollama-compatible request
            // Reuse constants to avoid string allocations
            let ollama_messages = vec![
                json!({
                    "role": SYSTEM_ROLE,
                    "content": QUICK_SYSTEM_PROMPT
                }),
                json!({
                    "role": USER_ROLE,
                    "content": last_message.content
                }),
            ];
            json!({
                "model": self.config.small_model.model_name,
                "messages": ollama_messages,
                "stream": false,
                "keep_alive": "10m",
                "options": {
                    "temperature": 0.0,
                    "num_predict": 3,
                    "top_k": 1,
                    "top_p": 0.1,
                    "repeat_penalty": 1.0
                }
            })
        } else {
            // Full OpenAI-compatible request
            serde_json::to_value(OpenAIRequest {
                model: self.config.small_model.model_name.clone(),
                messages: prompt_messages,
                max_tokens: Some(10),
                temperature: Some(0.3),
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                stop: None,
                stream: false,
                extra_body: None, // Remove extra_body for OpenAI compatibility
            })?
        };

        // Determine endpoint based on base URL (Ollama vs OpenAI compatible)
        let endpoint = if self.config.small_model.base_url.contains("11434") {
            format!("{}/api/chat", self.config.small_model.base_url)
        } else {
            format!("{}/chat/completions", self.config.small_model.base_url)
        };

        let mut request_builder = self
            .client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Only add Authorization header if API key is not "none" (for local services like Ollama)
        if self.config.small_model.api_key.expose_secret() != "none" {
            request_builder = request_builder.header(
                "Authorization",
                format!("Bearer {}", self.config.small_model.api_key.expose_secret()),
            );
        }

        // Apply retry mechanism for small model calls
        let request_builder_clone = request_builder
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("Failed to clone request builder"))?;
        let timeout_duration = Duration::from_secs(self.config.small_model_timeout_secs);

        let response = execute_with_retry(
            move || {
                let builder = match request_builder_clone.try_clone() {
                    Some(b) => b,
                    None => {
                        return Box::pin(async {
                            Err(anyhow::anyhow!("Failed to clone request builder in retry"))
                        })
                    }
                };
                let timeout_dur = timeout_duration;
                Box::pin(async move {
                    timeout(timeout_dur, builder.send())
                        .await
                        .map_err(|_e| {
                            // 转为类型化超时错误
                            anyhow::Error::from(LoroError::Timeout { timeout_secs: timeout_dur.as_secs() })
                        })?
                        .map_err(|e| anyhow::Error::from(LoroError::HttpClient(e)))
                })
            },
            self.config.max_retries,
            "small_model_request",
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::Error::from(LoroError::ApiError {
                provider: "small".to_string(),
                status: status.as_u16(),
                message: error_text,
            }));
        }

        // Parse response based on API provider
        let response_content = if self.config.small_model.base_url.contains("11434") {
            // Ollama response format
            let ollama_response: OllamaResponse = response
                .json()
                .await
                .context("Failed to parse Ollama response")?;
            ollama_response.message.content
        } else {
            // OpenAI response format
            let openai_response: OpenAIResponse = response
                .json()
                .await
                .context("Failed to parse small model response")?;

            openai_response
                .choices
                .first()
                .and_then(|choice| choice.message.as_ref())
                .and_then(|msg| msg.content.as_ref())
                .ok_or_else(|| anyhow::anyhow!("No content in small model response"))?
                .to_string()
        };

        Ok(response_content.trim().to_string())
    }

    fn is_appropriate_quick_response(&self, text: &str) -> bool {
        // Should be short and conversational
        if text.chars().count() > 6 {
            return false;
        }
        // Should not contain complex sentences
        let complex_punctuation_count = text.chars().filter(|&c| "。！？，".contains(c)).count();
        if complex_punctuation_count > 1 {
            return false;
        }
        true
    }

    async fn get_large_model_stream(
        &self,
        request: ChatCompletionRequest,
        _prefix: Option<String>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // Enhance messages with voice assistant context - pre-allocate for performance
        let mut enhanced_messages = Vec::with_capacity(request.messages.len() + 1);
        // Use module-level constants for common strings

        enhanced_messages.push(HashMap::from([
            ("role".to_string(), SYSTEM_ROLE.to_string()),
            ("content".to_string(), LARGE_SYSTEM_PROMPT.to_string()),
        ]));

        // Pre-allocate capacity to avoid reallocations
        enhanced_messages.reserve(request.messages.len());

        for msg in &request.messages {
            enhanced_messages.push(HashMap::from([
                ("role".to_string(), msg.role.clone()),
                ("content".to_string(), msg.content.clone()),
            ]));
        }

        let is_ollama = self.config.large_model.base_url.contains("11434");

        // Build request body and endpoint depending on provider
        let (endpoint, body_value, add_auth) = if is_ollama {
            let mut msgs: Vec<serde_json::Value> = Vec::with_capacity(request.messages.len() + 1);
            msgs.push(json!({"role": SYSTEM_ROLE, "content": LARGE_SYSTEM_PROMPT}));
            for m in &request.messages {
                msgs.push(json!({"role": m.role, "content": m.content}));
            }
            let body = json!({
                "model": self.config.large_model.model_name,
                "messages": msgs,
                "stream": true,
                "keep_alive": "10m"
            });
            (format!("{}/api/chat", self.config.large_model.base_url), body, false)
        } else {
            let request_body = OpenAIRequest {
                model: self.config.large_model.model_name.clone(),
                messages: enhanced_messages,
                max_tokens: request.max_tokens.or(Some(150)),
                temperature: Some(request.temperature),
                top_p: None,
                frequency_penalty: None,
                presence_penalty: None,
                stop: request.stop.clone(),
                stream: true,
                extra_body: _prefix.map(|p| json!({"prefix": p})),
            };
            (
                format!("{}/chat/completions", self.config.large_model.base_url),
                serde_json::to_value(request_body)?,
                true,
            )
        };

        let mut request_builder = self.client.post(endpoint)
            .header("Content-Type", "application/json")
            .json(&body_value);

        if add_auth && self.config.large_model.api_key.expose_secret() != "none" {
            request_builder = request_builder.header(
                "Authorization",
                format!("Bearer {}", self.config.large_model.api_key.expose_secret()),
            );
        }

        // Apply retry mechanism for large model calls
        let request_builder_clone = request_builder
            .try_clone()
            .ok_or_else(|| anyhow::anyhow!("Failed to clone request builder"))?;
        let response = execute_with_retry(
            move || {
                let builder = match request_builder_clone.try_clone() {
                    Some(b) => b,
                    None => {
                        return Box::pin(async {
                            Err(anyhow::anyhow!("Failed to clone request builder in retry"))
                        })
                    }
                };
                Box::pin(async move {
                    builder
                        .send()
                        .await
                        .map_err(|e| anyhow::Error::from(LoroError::HttpClient(e)))
                })
            },
            self.config.max_retries,
            "large_model_request",
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Large model API error {}: {}",
                status,
                error_text
            ));
        }

        let byte_stream = response.bytes_stream();
        let request_id = Uuid::new_v4().to_string();
        let model_name = request.model.clone();

        // Process upstream stream with buffering for incomplete chunks (SSE or Ollama JSON lines)
        let buffer = Arc::new(std::sync::Mutex::new(String::new()));
        let stream = byte_stream
            .map(move |chunk_result| {
                let request_id = request_id.clone();
                let model_name = model_name.clone();
                let buffer = Arc::clone(&buffer);
                let is_ollama = is_ollama;

                match chunk_result {
                    Ok(bytes) => {
                        let mut buffer_guard = match buffer.lock() {
                            Ok(guard) => guard,
                            Err(e) => {
                                error!("SSE buffer lock poisoned: {}", e);
                                return futures::stream::iter(vec![Err(anyhow::anyhow!(
                                    "SSE buffer lock poisoned"
                                ))]);
                            }
                        };

                        // Append new data to buffer
                        buffer_guard.push_str(&String::from_utf8_lossy(&bytes));
                        
                        let mut results = Vec::new();
                        let mut remaining_data = String::new();
                        
                        // Process complete lines from buffer
                        for line in buffer_guard.lines() {
                            let line = line.trim();
                            if !line.is_empty() {
                                if is_ollama {
                                    // Ollama streaming: each line is a standalone JSON object
                                    match Self::process_ollama_line_static(line, &request_id, &model_name) {
                                        Ok(Some(chunk)) => results.push(Ok(chunk)),
                                        Ok(None) => {}
                                        Err(_e) => {
                                            // Likely incomplete JSON, keep for next chunk
                                            remaining_data = line.to_string();
                                        }
                                    }
                                } else {
                                    // SSE 格式：形如 data: {...}
                                    if line.starts_with("data: ") || line == "[DONE]" {
                                        match Self::process_sse_line_static(line, &request_id, &model_name) {
                                            Ok(Some(chunk)) => results.push(Ok(chunk)),
                                            Ok(None) => {}
                                            Err(_e) => {
                                                // 可能是半行，保留
                                                remaining_data = line.to_string();
                                            }
                                        }
                                    } else {
                                        // 不完整行，保留
                                        remaining_data = line.to_string();
                                    }
                                }
                            }
                        }
                        
                        // Update buffer with remaining incomplete data
                        *buffer_guard = remaining_data;
                        
                        futures::stream::iter(results)
                    }
                    Err(e) => {
                        // Create error chunk similar to Python version
                        let error_chunk = ChatCompletionChunk {
                            id: format!("chatcmpl-{request_id}"),
                            object: CHUNK_OBJECT.to_string(),
                            created: chrono::Utc::now().timestamp(),
                            model: model_name.clone(),
                            choices: vec![ChoiceDelta {
                                index: 0,
                                delta: MessageDelta {
                                    role: None,
                                    content: Some(" [抱歉，出现了问题]".to_string()),
                                },
                                finish_reason: Some(STOP_REASON.to_string()),
                            }],
                        };

                        if let Ok(json_str) = serde_json::to_string(&error_chunk) {
                            if json_str.len() > 1024 * 1024 {
                                futures::stream::iter(vec![Err(anyhow::anyhow!(
                                    "Error chunk too large"
                                ))])
                            } else {
                                futures::stream::iter(vec![Ok(json_str)])
                            }
                        } else {
                            futures::stream::iter(vec![Err(anyhow::anyhow!("Stream error: {}", e))])
                        }
                    }
                }
            })
            .flatten();

        Ok(Box::pin(stream))
    }

    pub fn process_ollama_line_static(
        line: &str,
        request_id: &str,
        model_name: &str,
    ) -> Result<Option<String>> {
        // 每一行应为一个 JSON 对象
        let resp: crate::models::OllamaResponse = serde_json::from_str(line)?;
        let content = resp.message.content;
        if content.is_empty() {
            return Ok(None);
        }
        let chunk = ChatCompletionChunk {
            id: format!("chatcmpl-{}", request_id),
            object: CHUNK_OBJECT.to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model_name.to_string(),
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta { role: None, content: Some(content) },
                finish_reason: None,
            }],
        };
        let json_str = serde_json::to_string(&chunk)?;
        if json_str.len() > 1024 * 1024 { return Ok(None); }
        Ok(Some(json_str))
    }
    pub fn process_sse_line_static(
        line: &str,
        request_id: &str,
        model_name: &str,
    ) -> Result<Option<String>> {
        // Handle SSE format: "data: {json}" or "data: [DONE]"
        if let Some(json_data) = line.strip_prefix("data: ") {
            if json_data.trim() == "[DONE]" {
                return Ok(None);
            }

            // Skip empty data lines and event lines
            let json_data = json_data.trim();
            if json_data.is_empty() {
                return Ok(None);
            }

            // Parse the JSON chunk with improved error handling
            match serde_json::from_str::<OpenAIResponse>(json_data) {
                Ok(openai_chunk) => {
                    if let Some(choice) = openai_chunk.choices.first() {
                        // Handle both message and delta fields for compatibility
                        let (content, role, finish_reason) = if let Some(delta) = &choice.delta {
                            (
                                delta.content.as_ref(),
                                delta.role.as_ref(),
                                choice.finish_reason.as_ref(),
                            )
                        } else if let Some(message) = &choice.message {
                            (
                                message.content.as_ref(),
                                message.role.as_ref(),
                                choice.finish_reason.as_ref(),
                            )
                        } else {
                            (None, None, choice.finish_reason.as_ref())
                        };

                        if let Some(content) = content {
                            // Only process chunks with actual content
                            if !content.is_empty() {
                                let chunk = ChatCompletionChunk {
                                    id: format!("chatcmpl-{request_id}"),
                                    object: CHUNK_OBJECT.to_string(),
                                    created: chrono::Utc::now().timestamp(),
                                    model: model_name.to_string(),
                                    choices: vec![ChoiceDelta {
                                        index: 0,
                                        delta: MessageDelta {
                                            role: role.cloned(),
                                            content: Some(content.clone()),
                                        },
                                        finish_reason: finish_reason.cloned(),
                                    }],
                                };

                                let json_str = serde_json::to_string(&chunk)?;
                                if json_str.len() > 1024 * 1024 {
                                    return Ok(None);
                                }
                                return Ok(Some(json_str));
                            }
                        }

                        // Handle finish_reason without content (end of stream)
                        if finish_reason.is_some() && content.is_none() {
                            let chunk = ChatCompletionChunk {
                                id: format!("chatcmpl-{request_id}"),
                                object: CHUNK_OBJECT.to_string(),
                                created: chrono::Utc::now().timestamp(),
                                model: model_name.to_string(),
                                choices: vec![ChoiceDelta {
                                    index: 0,
                                    delta: MessageDelta {
                                        role: None,
                                        content: None,
                                    },
                                    finish_reason: finish_reason.cloned(),
                                }],
                            };

                            // Optimize string formatting with safety checks
                            let json_str = serde_json::to_string(&chunk)?;
                            if json_str.len() > 1024 * 1024 {
                                return Ok(None);
                            }
                            return Ok(Some(json_str));
                        }
                    }
                }
                Err(e) => {
                    // Don't return an error, just log and skip this chunk
                    debug!("Skipping malformed SSE chunk: {}, data: {}", e, json_data);
                    return Ok(None);
                }
            }
        }

        // Skip non-data lines (like event: lines)
        Ok(None)
    }

    pub async fn get_metrics(&self) -> serde_json::Value {
        let quick_stats = self.quick_stats.get_stats();
        let direct_stats = self.direct_stats.get_stats();

        let quick_avg = self.quick_stats.get_avg_first_response_time();
        let direct_avg = self.direct_stats.get_avg_first_response_time();
        let improvement = if direct_avg > 0.0 && quick_avg > 0.0 {
            direct_avg - quick_avg
        } else {
            0.0
        };

        json!({
            "quick_response_mode": quick_stats,
            "direct_mode": direct_stats,
            "comparison": {
                "quick_mode_requests": self.quick_stats.get_request_count(),
                "direct_mode_requests": self.direct_stats.get_request_count(),
                "avg_first_response_improvement": improvement
            }
        })
    }

    pub async fn reset_metrics(&self) {
        self.quick_stats.reset();
        self.direct_stats.reset();
        info!("Metrics reset successfully");
    }
}

// Retry helper function
async fn execute_with_retry<F, T>(
    mut operation: F,
    max_retries: u32,
    operation_name: &str,
) -> Result<T>
where
    F: FnMut() -> futures::future::BoxFuture<'static, Result<T>>,
{
    let mut attempt = 0;
    let mut last_error: Option<anyhow::Error> = None;

    while attempt <= max_retries {
        attempt += 1;
        match operation().await {
            Ok(result) => {
                if attempt > 1 {
                    info!("{} succeeded after {} attempts", operation_name, attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                last_error = Some(e);
                if attempt <= max_retries {
                    let delay = Duration::from_millis((100 * attempt.pow(2)) as u64);
                    warn!(
                        "{} attempt {} failed, retrying in {:?}: {}",
                        operation_name,
                        attempt,
                        delay,
                        last_error.as_ref().unwrap()
                    );
                    tokio::time::sleep(delay).await;
                } else {
                    error!(
                        "{} failed after {} attempts: {}",
                        operation_name,
                        max_retries,
                        last_error.as_ref().unwrap()
                    );
                }
            }
        }
    }

    Err(last_error.unwrap())
}
