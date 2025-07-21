<p align="center">
  <img src="logo.png" alt="loro Logo" width="200"/>
</p>

# loro 

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Tests](https://img.shields.io/badge/tests-30%20passing-green.svg)]()
[![Coverage](https://img.shields.io/badge/coverage-80%2B%25-green.svg)]()

Loro is a high-performance AI voice assistant API service built in Rust, implementing a dual-model strategy to optimize response latency. The system uses a small model to generate immediate acknowledgment responses while a large model processes the complete response in parallel, significantly improving user experience in voice interactions.

## ✨ Key Features

- **Dual-Model Concurrent Strategy**: Small model for instant feedback + Large model for complete responses
- **Streaming Responses**: Zero-copy streaming transmission with Server-Sent Events (SSE)
- **Performance Monitoring**: Real-time latency statistics and comparative analysis
- **Voice Assistant Optimization**: Specifically designed for voice interaction scenarios
- **OpenAI Compatibility**: Fully compatible with OpenAI ChatCompletion API
- **Production Ready**: Comprehensive error handling, structured logging, and 80%+ test coverage


## 📋 Quick Start

### Prerequisites

- Rust 1.70+
- Valid AI model API keys (SiliconFlow, OpenAI, or compatible providers)

### Installation and Setup

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd loro
   ```

2. **Configure environment variables**
   ```bash
   # Create environment configuration
   export SMALL_MODEL_API_KEY="your-small-model-api-key"
   export LARGE_MODEL_API_KEY="your-large-model-api-key"
   
   # Optional: customize endpoints and models
   export SMALL_MODEL_BASE_URL="https://api.siliconflow.cn/v1"
   export SMALL_MODEL_NAME="Qwen/Qwen2-1.5B-Instruct"
   export LARGE_MODEL_BASE_URL="https://api.siliconflow.cn/v1"  
   export LARGE_MODEL_NAME="deepseek-ai/DeepSeek-V2.5"
   ```

3. **Build and run**
   ```bash
   # Development mode
   cargo run
   
   # Production mode (optimized)
   cargo run --release
   ```

4. **Verify installation**
   ```bash
   # Run comprehensive test suite
   cargo test
   
   # Run example client
   cargo run --example client
   ```

### Configuration Options

The service supports extensive configuration through environment variables:

```bash
# Required: Model API Keys
SMALL_MODEL_API_KEY=your-small-model-key
LARGE_MODEL_API_KEY=your-large-model-key

# Optional: Model Endpoints
SMALL_MODEL_BASE_URL=https://api.siliconflow.cn/v1  # Default
LARGE_MODEL_BASE_URL=https://api.siliconflow.cn/v1  # Default
SMALL_MODEL_NAME=Qwen/Qwen2-1.5B-Instruct         # Default
LARGE_MODEL_NAME=deepseek-ai/DeepSeek-V2.5         # Default

# Optional: Server Configuration  
HOST=0.0.0.0                    # Default: 0.0.0.0
PORT=8000                       # Default: 8000
LOG_LEVEL=info                  # Default: info

# Optional: Performance Tuning
HTTP_TIMEOUT_SECS=30           # Default: 30 (5-300)
SMALL_MODEL_TIMEOUT_SECS=5     # Default: 5 (1-30)  
MAX_RETRIES=3                  # Default: 3 (0-10)
STATS_MAX_ENTRIES=10000        # Default: 10000 (100-100000)
```

## 🛠️ API Reference

### Endpoints

- `POST /v1/chat/completions` - OpenAI-compatible chat completion (supports streaming)
- `GET /` - Service information and status
- `GET /health` - Health check endpoint
- `GET /metrics` - Performance metrics and statistics
- `POST /metrics/reset` - Reset performance metrics

### Usage Examples

**Quick Response Mode (Default)**:
```bash
curl -X POST "http://localhost:8000/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "loro-voice-assistant",
    "messages": [{"role": "user", "content": "Hello, how are you?"}],
    "stream": true
  }'
```

**Direct Mode (Bypass Quick Response)**:
```bash
curl -X POST "http://localhost:8000/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "loro-voice-assistant", 
    "messages": [{"role": "user", "content": "Hello, how are you?"}],
    "stream": true,
    "disable_quick_response": true
  }'
```

**Request Parameters**:
- `model`: Model identifier (any string, ignored in current implementation)
- `messages`: Array of message objects with `role` and `content`
- `stream`: Boolean, defaults to `true` (non-streaming mode not implemented)
- `max_tokens`: Integer, 1-8192 (optional)
- `temperature`: Float, 0.0-2.0 (default: 0.7)
- `disable_quick_response`: Boolean, bypasses dual-model strategy (optional)

## 🏗️ Architecture

### Core Components

- **Web Framework**: axum 0.7 + tokio async runtime
- **HTTP Client**: reqwest with connection pooling
- **Serialization**: serde with zero-copy deserialization  
- **Logging**: tracing with structured logging
- **Configuration**: dotenvy for environment management
- **Error Handling**: thiserror for structured error types

### Dual-Model Strategy

1. **Concurrent Execution**: Both models start simultaneously using `tokio::join!`
2. **Quick Response**: Small model generates 1-3 character acknowledgments
3. **Complete Response**: Large model processes full response in parallel
4. **Stream Merging**: Quick response sent immediately, followed by large model output
5. **Message Categorization**: Automatic detection of greetings, questions, requests

### Message Processing Flow

```
User Input → Request Validation → Dual Model Strategy
                                      ↓
    Small Model (Quick Response) ← tokio::join! → Large Model (Complete Response)
                                      ↓
    Quick Response Sent ← Stream Merger → Complete Response Streamed
                                      ↓
                              Performance Metrics Updated
```

## 🧪 Testing

### Running Tests

```bash
# Run all tests (30 total)
cargo test

# Run with single thread (avoids environment variable conflicts)
cargo test -- --test-threads=1

# Run specific test categories
cargo test --test integration_test  # Integration tests
cargo test --test end_to_end_test   # End-to-end tests

# Run with output
cargo test -- --nocapture
```


### Performance Benchmarking

```bash
# Terminal 1: Start the service
cargo run --release

# Terminal 2: Run benchmark client
cargo run --example client

# View metrics
curl http://localhost:8000/metrics
```

## 📊 Monitoring

### Performance Metrics

Access detailed performance data via `/metrics` endpoint:

```json
{
  "quick_response_mode": {
    "total_requests": 100,
    "first_response_latency": {
      "avg": 0.045, "min": 0.028, "max": 0.089,
      "p50": 0.041, "p95": 0.076
    },
    "total_response_latency": {
      "avg": 1.234, "min": 0.867, "max": 2.145,
      "p50": 1.156, "p95": 1.987
    }
  },
  "direct_mode": {
    "total_requests": 50,
    "first_response_latency": {
      "avg": 0.678, "min": 0.445, "max": 1.234,
      "p50": 0.634, "p95": 1.087
    }
  },
  "comparison": {
    "quick_mode_requests": 100,
    "direct_mode_requests": 50,
    "avg_first_response_improvement": 0.633
  }
}
```

### Key Metrics

- **First Response Latency**: Time to first chunk (critical for voice UX)
- **Total Response Latency**: Complete response generation time
- **Quick Response Time**: Small model processing time
- **Large Model Time**: Large model processing time
- **Request Counts**: Separate tracking for each mode

## 🔧 Development

### Project Structure

```
loro/
├── src/
│   ├── main.rs          # Server entry point and HTTP handlers
│   ├── lib.rs           # Library exports
│   ├── config.rs        # Environment configuration management
│   ├── models.rs        # OpenAI-compatible data structures
│   ├── service.rs       # Core dual-model service logic
│   ├── stats.rs         # Performance statistics collection
│   └── errors.rs        # Structured error types
├── tests/
│   ├── integration_test.rs  # Integration and unit tests
│   └── end_to_end_test.rs   # End-to-end system tests
├── examples/
│   └── client.rs        # Example client with benchmarking
└── references/          # Original Python implementation (local only)
    ├── main.py         # Reference server implementation
    └── client.py       # Reference client implementation
```

### Development Workflow

1. **Code Quality**:
   ```bash
   cargo fmt              # Format code
   cargo clippy           # Lint and suggestions
   cargo test             # Run test suite
   cargo doc --open       # Generate documentation
   ```

2. **Performance Profiling**:
   ```bash
   cargo run --release    # Optimized build
   cargo bench            # Benchmarks (if implemented)
   ```

3. **Debugging**:
   ```bash
   RUST_LOG=debug cargo run          # Verbose logging
   RUST_BACKTRACE=1 cargo run        # Stack traces
   ```

### Contributing Guidelines

- **Testing**: All new features must include tests
- **Documentation**: Update README for API changes
- **Performance**: Consider impact on response latency
- **Compatibility**: Maintain OpenAI API compatibility
- **Error Handling**: Use structured error types from `errors.rs`

### Adding New Model Providers

To integrate additional AI model providers:

1. **Configuration**: Add new environment variables in `config.rs`
2. **Request Format**: Update `OpenAIRequest` structure if needed
3. **Response Parsing**: Modify SSE parsing in `service.rs` 
4. **Testing**: Add provider-specific tests
5. **Documentation**: Update configuration section

## 🚀 Deployment

### Production Considerations

- **Environment**: Set `RUST_LOG=info` for production logging
- **Resources**: Allocate sufficient memory for model responses
- **Monitoring**: Set up external monitoring for `/health` endpoint
- **Security**: Configure proper firewall rules and TLS termination
- **Scaling**: Consider load balancing for high-traffic scenarios

### Docker Deployment (Future)

```dockerfile
# Multi-stage build for optimized production image
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/loro /usr/local/bin/loro
EXPOSE 8000
CMD ["loro"]
```

## 🤝 Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

### Development Setup

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## 📄 License

This project is licensed under the AGPL-3.0 License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

This project is a Rust reimplementation of the original Python BlastOff LLM voice assistant, designed to achieve significantly better performance and reliability while maintaining the innovative dual-model strategy for optimized voice interactions.

## 📞 Support

For questions, issues, or contributions:
- Open an issue on GitHub
- Review the test suite for usage examples
