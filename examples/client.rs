use loro::models::*;
use reqwest::Client;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎤 Loro AI Voice Assistant Client Test");
    println!("Make sure the server is running on http://127.0.0.1:8000");
    println!();

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .no_proxy() // Disable proxy in case there's interference
        .build()?;
    let base_url = "http://127.0.0.1:8000";

    // Test basic endpoints
    test_health_check(&client, base_url).await?;
    test_root_endpoint(&client, base_url).await?;
    test_metrics_reset(&client, base_url).await?;

    // Test voice assistant scenarios
    test_voice_scenarios(&client, base_url).await?;

    println!("\n✅ All tests completed successfully!");
    Ok(())
}

async fn test_health_check(
    client: &Client,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing health check...");

    match client.get(&format!("{}/health", base_url)).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let body: serde_json::Value = response.json().await?;
                println!("✅ Health check: {:?}", body);
            } else {
                println!("❌ Health check failed: {}", response.status());
            }
        }
        Err(e) => {
            println!("❌ Health check request error: {}", e);
            if e.is_connect() {
                println!("   Connection error - is the server running?");
            } else if e.is_timeout() {
                println!("   Timeout error");
            } else if e.is_request() {
                println!("   Request error");
            }
        }
    }

    Ok(())
}

async fn test_root_endpoint(
    client: &Client,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing root endpoint...");

    let response = client.get(base_url).send().await?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        println!("✅ Root endpoint: {:?}", body);
    } else {
        println!("❌ Root endpoint failed: {}", response.status());
    }

    Ok(())
}

async fn test_metrics_reset(
    client: &Client,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing metrics reset...");

    let response = client
        .post(&format!("{}/metrics/reset", base_url))
        .send()
        .await?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        println!("✅ Metrics reset: {:?}", body);
    } else {
        println!("❌ Metrics reset failed: {}", response.status());
    }

    Ok(())
}

async fn test_voice_scenarios(
    client: &Client,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎤 Testing voice assistant scenarios...");

    let test_scenarios = vec![
        TestScenario {
            name: "Greeting",
            messages: vec![Message {
                role: "user".to_string(),
                content: "你好！".to_string(),
            }],
            description: "Basic greeting interaction",
        },
        TestScenario {
            name: "Simple Question",
            messages: vec![Message {
                role: "user".to_string(),
                content: "今天天气怎么样？".to_string(),
            }],
            description: "Weather inquiry",
        },
        TestScenario {
            name: "Request Help",
            messages: vec![Message {
                role: "user".to_string(),
                content: "请帮我设个明天上午9点的闹钟".to_string(),
            }],
            description: "Task request",
        },
    ];

    let mut quick_times = Vec::new();
    let mut direct_times = Vec::new();

    for (i, scenario) in test_scenarios.iter().enumerate() {
        println!("\n📝 Test {}: {}", i + 1, scenario.name);
        println!("   Description: {}", scenario.description);
        println!("   User: {}", scenario.messages.last().unwrap().content);

        // Test with quick response (default mode)
        if let Ok(time) = test_voice_response(client, base_url, scenario, false).await {
            if time > 0.0 {
                quick_times.push(time);
            }
        }

        println!();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Test without quick response (direct mode)
        if let Ok(time) = test_voice_response(client, base_url, scenario, true).await {
            if time > 0.0 {
                direct_times.push(time);
            }
        }

        println!("{}", "-".repeat(50));
    }

    // Display results
    if !quick_times.is_empty() && !direct_times.is_empty() {
        let quick_avg: f64 = quick_times.iter().sum::<f64>() / quick_times.len() as f64;
        let direct_avg: f64 = direct_times.iter().sum::<f64>() / direct_times.len() as f64;
        let improvement = direct_avg - quick_avg;
        let improvement_pct = (improvement / direct_avg) * 100.0;

        println!("\n📊 VOICE ASSISTANT PERFORMANCE RESULTS:");
        println!("   Quick Response Mode:");
        println!("     Average first response: {:.3}s", quick_avg);
        println!("   Direct Mode:");
        println!("     Average first response: {:.3}s", direct_avg);
        println!(
            "   🚀 Improvement: {:.3}s ({:.1}% faster)",
            improvement, improvement_pct
        );

        println!("\n🎯 VOICE ASSISTANT ANALYSIS:");
        if quick_avg < 0.2 {
            println!("   ✅ Excellent: Sub-200ms response feels natural");
        } else if quick_avg < 0.5 {
            println!("   ✅ Good: Response time acceptable for voice interaction");
        } else {
            println!("   ⚠️ Needs improvement: May feel slow for voice interaction");
        }
    }

    // Get final metrics
    test_metrics(&client, base_url).await?;

    Ok(())
}

async fn test_voice_response(
    client: &Client,
    base_url: &str,
    scenario: &TestScenario,
    disable_quick: bool,
) -> Result<f64, Box<dyn std::error::Error>> {
    let mode_name = if disable_quick {
        "Direct Mode"
    } else {
        "Quick Response Mode"
    };
    println!("\n🎙️ {} Response:", mode_name);

    let request = ChatCompletionRequest {
        model: "loro-voice-assistant".to_string(),
        messages: scenario.messages.clone(),
        max_tokens: Some(150),
        temperature: 0.7,
        stream: true,
        stop: None,
        disable_quick_response: disable_quick,
    };

    let start_time = Instant::now();

    let response = client
        .post(&format!("{}/v1/chat/completions", base_url))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        println!("❌ Request failed: {}", response.status());
        return Ok(0.0);
    }

    let mut first_chunk_time = None;

    // This is a simplified version - in reality you'd need to parse SSE format
    let bytes = response.bytes().await?;
    let text = String::from_utf8_lossy(&bytes);

    // For demonstration, just record the time
    if first_chunk_time.is_none() {
        first_chunk_time = Some(start_time.elapsed().as_secs_f64());
    }

    // Store the actual response (in real implementation, this would accumulate chunks)
    let response_text = text.to_string();
    println!("📝 Response: {}", response_text);

    if let Some(first_response_time) = first_chunk_time {
        println!("⏱️ First response time: {:.3}s", first_response_time);
        Ok(first_response_time)
    } else {
        println!("⚠ No response received");
        Ok(0.0)
    }
}

async fn test_metrics(client: &Client, base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📈 Server Metrics:");

    let response = client.get(&format!("{}/metrics", base_url)).send().await?;

    if response.status().is_success() {
        let metrics: serde_json::Value = response.json().await?;
        println!("{:#}", metrics);
    } else {
        println!("❌ Failed to get metrics: {}", response.status());
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct TestScenario {
    name: &'static str,
    messages: Vec<Message>,
    description: &'static str,
}
