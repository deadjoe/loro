# Loro - AI语音助手快速响应系统

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Loro是一个基于Rust的高性能AI语音助手API服务，采用双模型策略优化响应延迟。该项目通过小模型生成快速语气词立即响应，同时大模型并行生成完整回答，显著提升用户体验。

## 🌟 核心特性

- **双模型并发策略**: 小模型快速响应 + 大模型完整回答
- **流式响应**: 零拷贝流式传输，降低内存使用和延迟
- **性能监控**: 实时延迟统计和两种模式性能对比
- **语音助手优化**: 专门针对语音交互场景设计
- **OpenAI兼容**: 完全兼容OpenAI ChatCompletion API接口
- **高性能**: Rust实现，相比Python版本有显著性能提升

## 🚀 性能优势

相比原Python实现的预期性能提升：

| 指标 | 改进幅度 |
|------|----------|
| 响应延迟 | 减少30-50% |
| 并发能力 | 提升3-5倍 |
| 内存使用 | 减少50-70% |
| 启动时间 | 提升10倍以上 |

## 📋 快速开始

### 前置要求

- Rust 1.70+
- 有效的AI模型API密钥

### 安装和配置

1. **克隆项目**
   ```bash
   git clone <repository-url>
   cd loro
   ```

2. **配置环境变量**
   ```bash
   cp .env.example .env
   # 编辑 .env 文件，填入你的API密钥
   ```

3. **编译和运行**
   ```bash
   cargo run
   ```

4. **测试API**
   ```bash
   cargo test
   cargo run --example client
   ```

### 环境变量配置

```bash
# 小模型配置（用于快速响应）
SMALL_MODEL_API_KEY=your-small-model-api-key
SMALL_MODEL_BASE_URL=https://api.siliconflow.cn/v1
SMALL_MODEL_NAME=Qwen/Qwen2-1.5B-Instruct

# 大模型配置（用于完整回答）
LARGE_MODEL_API_KEY=your-large-model-api-key
LARGE_MODEL_BASE_URL=https://api.siliconflow.cn/v1
LARGE_MODEL_NAME=deepseek-ai/DeepSeek-V2.5

# 服务配置
HOST=0.0.0.0
PORT=8000
LOG_LEVEL=info
```

## 🛠️ API接口

### 主要端点

- `POST /v1/chat/completions` - OpenAI兼容的聊天完成接口
- `GET /` - 服务状态信息
- `GET /health` - 健康检查
- `GET /metrics` - 性能指标
- `POST /metrics/reset` - 重置指标

### 使用示例

**快速响应模式（默认）**:
```bash
curl -X POST "http://localhost:8000/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "loro-voice-assistant",
    "messages": [{"role": "user", "content": "你好！"}],
    "stream": true
  }'
```

**直接模式（对比测试）**:
```bash
curl -X POST "http://localhost:8000/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "loro-voice-assistant", 
    "messages": [{"role": "user", "content": "你好！"}],
    "stream": true,
    "disable_quick_response": true
  }'
```

## 🏗️ 技术架构

### 核心组件

- **Web框架**: axum + tokio (替代FastAPI + asyncio)
- **HTTP客户端**: reqwest (替代AsyncOpenAI)
- **序列化**: serde (替代Pydantic)
- **日志**: tracing (替代logging)
- **配置**: dotenvy (替代dotenv)

### 双模型策略

1. **第一步**: 小模型生成1-3字的语气词（如"好的，"、"让我想想，"）立即返回
2. **第二步**: 大模型并行生成完整回答，流式传输
3. **合并**: 将快速响应和完整回答无缝连接

### 性能统计

系统自动收集以下性能指标：
- 首次响应时间
- 总响应时间  
- 快速响应时间
- 大模型响应时间
- 请求数量统计

## 🧪 测试

### 运行测试

```bash
# 单元测试
cargo test

# 集成测试
cargo test --test integration_test

# 客户端测试
cargo run --example client
```

### 性能基准测试

启动服务后，运行客户端测试查看性能对比：

```bash
# 终端1: 启动服务
cargo run

# 终端2: 运行测试客户端  
cargo run --example client
```

## 📊 监控指标

通过 `/metrics` 端点获取详细性能数据：

```json
{
  "quick_response_mode": {
    "total_requests": 10,
    "first_response_latency": {"avg": 0.12, "min": 0.08, "max": 0.18},
    "total_response_latency": {"avg": 1.45, "min": 1.12, "max": 1.89}
  },
  "direct_mode": {
    "total_requests": 10, 
    "first_response_latency": {"avg": 0.85, "min": 0.72, "max": 1.12}
  },
  "comparison": {
    "avg_first_response_improvement": 0.73
  }
}
```

## 🔧 开发

### 项目结构

```
loro/
├── src/
│   ├── main.rs          # 服务入口
│   ├── lib.rs           # 库入口  
│   ├── config.rs        # 配置管理
│   ├── models.rs        # 数据模型
│   ├── service.rs       # 核心服务逻辑
│   └── stats.rs         # 性能统计
├── tests/               # 测试用例
├── examples/            # 示例代码
├── references/          # 原Python代码参考
└── CLAUDE.md           # 开发计划和目标
```

### 代码规范

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 编写测试覆盖所有核心功能
- 关注性能和内存使用

## 📈 性能优化

### 已实现的优化

- **零拷贝流式传输**: 避免不必要的内存分配
- **并发请求处理**: tokio异步运行时真并行
- **连接池复用**: HTTP客户端连接复用
- **编译时优化**: Rust编译器自动优化

### 调优建议

- 根据硬件调整tokio线程数
- 合理设置HTTP超时时间
- 监控内存使用情况
- 定期清理性能统计数据

## 🤝 贡献

欢迎提交Issues和Pull Requests来改进项目。

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE) 文件。

## 🙏 致谢

本项目基于原Python版本BlastOff LLM重新设计实现，感谢原项目的创意和设计思路。