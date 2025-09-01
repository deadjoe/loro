use loro::service::LoroService;

#[test]
fn test_process_sse_line_static_returns_json_payload_only() {
    // 构造一行 OpenAI 风格的 SSE 数据
    let line = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}";
    let out = LoroService::process_sse_line_static(line, "rid", "model").unwrap();
    assert!(out.is_some());
    let payload = out.unwrap();
    // 不应包含 data: 前缀，由上层 axum 封装
    assert!(!payload.starts_with("data:"));
    // 应该是合法的 JSON
    let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert!(v.is_object());
}

#[test]
fn test_process_ollama_line_static_parses_json_lines() {
    // 模拟 Ollama 行式 JSON
    let line = r#"{"model":"qwen","created_at":"2024-01-01","message":{"role":"assistant","content":"你好"},"done":false}"#;
    let out = LoroService::process_ollama_line_static(line, "rid", "model").unwrap();
    assert!(out.is_some());
    let payload = out.unwrap();
    let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(v["object"], "chat.completion.chunk");
}

// 快速响应长度逻辑在已有测试覆盖（responses <= 6 chars）。此处不测私有方法。
