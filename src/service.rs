use crate::{config::Config, models::*, stats::StatsCollector};
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
                Ok(axum::response::sse::Event::default().data(format!("data: [ERROR: {e}]\n\n")))
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
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model_name,
            choices: vec![ChoiceDelta {
                index: 0,
                delta: MessageDelta {
                    role: Some("assistant".to_string()),
                    content: Some(quick_response.clone()),
                },
                finish_reason: None,
            }],
        };

        // Optimize string formatting for better performance
        let json_str = serde_json::to_string(&first_chunk)?;
        let mut first_chunk_data = String::with_capacity(json_str.len() + 8);
        first_chunk_data.push_str("data: ");
        first_chunk_data.push_str(&json_str);
        first_chunk_data.push_str("\n\n");

        // Step 2: Get large model stream with prefix
        let large_start = Instant::now();
        let large_stream = self.get_large_model_stream(request, Some(quick_response)).await?;

        let stats = Arc::clone(&self.quick_stats);
        let enhanced_stream = large_stream.enumerate().map(move |(i, chunk_result)| {
            match chunk_result {
                Ok(chunk_data) => {
                    if i == 0 {
                        // First chunk from large model - record stats
                        let large_time = large_start.elapsed().as_secs_f64();
                        let total_time = request_start.elapsed().as_secs_f64();

                        stats.add_request(
                            quick_time,
                            total_time,
                            Some(quick_time),
                            Some(large_time),
                        );
                    }
                    Ok(chunk_data)
                }
                Err(e) => Err(e),
            }
        });

        // Combine quick response and large model stream
        let combined_stream = stream::once(async move { Ok(first_chunk_data) })
            .chain(enhanced_stream)
            .chain(stream::once(async { Ok("data: [DONE]\n\n".to_string()) }));

        Ok(Box::pin(combined_stream))
    }

    async fn stream_direct_response(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_start = Instant::now();
        let large_stream = self.get_large_model_stream(request, None).await?;

        let stats = Arc::clone(&self.direct_stats);
        let enhanced_stream = large_stream.enumerate().map(move |(i, chunk_result)| {
            match chunk_result {
                Ok(chunk_data) => {
                    if i == 0 {
                        // First chunk
                        let first_response_time = request_start.elapsed().as_secs_f64();
                        let total_time = first_response_time; // Same for direct mode

                        stats.add_request(
                            first_response_time,
                            total_time,
                            None,
                            Some(first_response_time),
                        );
                    }
                    Ok(chunk_data)
                }
                Err(e) => Err(e),
            }
        });

        let final_stream =
            enhanced_stream.chain(stream::once(async { Ok("data: [DONE]\n\n".to_string()) }));

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
        let last_message = messages.last().unwrap(); // Safe because we checked above
        let category = last_message.categorize();
        let responses = category.get_responses();
        let mut rng = rand::thread_rng();
        let response = responses.choose(&mut rng).unwrap();
        Ok(response.to_string())
    }

    async fn call_small_model(&self, messages: &[Message]) -> Result<String> {
        if messages.is_empty() {
            return Err(anyhow::anyhow!("Messages array cannot be empty"));
        }

        let last_message = messages.last().unwrap(); // Safe due to check above
        if last_message.content.trim().is_empty() {
            return Err(anyhow::anyhow!("Message content cannot be empty"));
        }

        let prompt_messages = vec![
            HashMap::from([
                ("role".to_string(), "system".to_string()),
                ("content".to_string(), "你是一个AI语音助手。请用1-3个字的简短语气词回应用户，比如：'你好！'、'好的，'、'嗯，'、'让我想想，'，要自然像真人对话。只输出语气词，不要完整回答。".to_string()),
            ]),
            HashMap::from([
                ("role".to_string(), "user".to_string()),
                ("content".to_string(), last_message.content.clone()),
            ]),
        ];

        let request_body = OpenAIRequest {
            model: self.config.small_model.model_name.clone(),
            messages: prompt_messages,
            max_tokens: Some(10),
            temperature: Some(0.3),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            extra_body: Some(json!({"enable_thinking": false})),
        };

        let response = timeout(
            Duration::from_secs(self.config.small_model_timeout_secs),
            self.client
                .post(format!(
                    "{}/chat/completions",
                    self.config.small_model.base_url
                ))
                .header(
                    "Authorization",
                    format!("Bearer {}", self.config.small_model.api_key),
                )
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send(),
        )
        .await
        .context("Small model request timeout")?
        .context("Failed to send small model request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Small model API error {}: {}",
                status,
                error_text
            ));
        }

        let openai_response: OpenAIResponse = response
            .json()
            .await
            .context("Failed to parse small model response")?;

        let content = openai_response
            .choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .and_then(|msg| msg.content.as_ref())
            .ok_or_else(|| anyhow::anyhow!("No content in small model response"))?;

        Ok(content.trim().to_string())
    }

    fn is_appropriate_quick_response(&self, text: &str) -> bool {
        // Should be short and conversational
        if text.len() > 6 {
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
        prefix: Option<String>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // Enhance messages with voice assistant context - pre-allocate for performance
        let mut enhanced_messages = Vec::with_capacity(request.messages.len() + 1);
        enhanced_messages.push(HashMap::from([
            ("role".to_string(), "system".to_string()),
            ("content".to_string(), "你是一个友好的AI语音助手，用自然对话的方式回应用户。回答要简洁明了，适合语音交互。".to_string()),
        ]));

        for msg in &request.messages {
            enhanced_messages.push(HashMap::from([
                ("role".to_string(), msg.role.clone()),
                ("content".to_string(), msg.content.clone()),
            ]));
        }

        let mut extra_body = json!({"enable_thinking": false});
        if let Some(ref prefix) = prefix {
            extra_body["prefix"] = json!(prefix);
        }

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
            extra_body: Some(extra_body),
        };

        let response = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.config.large_model.base_url
            ))
            .header(
                "Authorization",
                format!("Bearer {}", self.config.large_model.api_key),
            )
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send large model request")?;

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

        // Process SSE stream - handle each chunk individually for now
        // TODO: Implement proper buffering for incomplete SSE chunks
        let stream = byte_stream
            .map(move |chunk_result| match chunk_result {
                Ok(bytes) => {
                    let data = String::from_utf8_lossy(&bytes);
                    let mut results = Vec::new();
                    
                    // Process each line in the received chunk
                    for line in data.lines() {
                        let line = line.trim();
                        if !line.is_empty() {
                            match Self::process_sse_line_static(line, &request_id, &model_name) {
                                Ok(Some(chunk)) => results.push(Ok(chunk)),
                                Ok(None) => {} // Skip empty chunks
                                Err(e) => {
                                    warn!("SSE parsing error: {}", e);
                                    // Continue processing instead of failing the entire stream
                                }
                            }
                        }
                    }
                    futures::stream::iter(results)
                }
                Err(e) => futures::stream::iter(vec![Err(anyhow::anyhow!("Stream error: {}", e))]),
            })
            .flatten();

        Ok(Box::pin(stream))
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
                            (delta.content.as_ref(), delta.role.as_ref(), choice.finish_reason.as_ref())
                        } else if let Some(message) = &choice.message {
                            (message.content.as_ref(), message.role.as_ref(), choice.finish_reason.as_ref())
                        } else {
                            (None, None, choice.finish_reason.as_ref())
                        };

                        if let Some(content) = content {
                            // Only process chunks with actual content
                            if !content.is_empty() {
                                let chunk = ChatCompletionChunk {
                                    id: format!("chatcmpl-{request_id}"),
                                    object: "chat.completion.chunk".to_string(),
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

                                let chunk_data =
                                    format!("data: {}\n\n", serde_json::to_string(&chunk)?);
                                return Ok(Some(chunk_data));
                            }
                        }
                        
                        // Handle finish_reason without content (end of stream)
                        if finish_reason.is_some() && content.is_none() {
                            let chunk = ChatCompletionChunk {
                                id: format!("chatcmpl-{request_id}"),
                                object: "chat.completion.chunk".to_string(),
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

                            // Optimize string formatting
                            let json_str = serde_json::to_string(&chunk)?;
                            let mut chunk_data = String::with_capacity(json_str.len() + 8);
                            chunk_data.push_str("data: ");
                            chunk_data.push_str(&json_str);
                            chunk_data.push_str("\n\n");
                            return Ok(Some(chunk_data));
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
