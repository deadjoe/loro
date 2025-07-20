# Loro - AI语音助手快速响应系统

## 项目概述

Loro是一个基于Rust的高性能AI语音助手API服务，采用双模型策略优化响应延迟。该项目是对原Python版本BlastOff LLM的重写，旨在提供更高的性能和稳定性。

## 核心特性

- **双模型并发策略**: 小模型生成快速语气词立即响应，大模型并行生成完整回答
- **流式响应**: 零拷贝流式传输，降低内存使用
- **性能监控**: 实时延迟统计和性能对比
- **语音助手优化**: 专门针对语音交互场景优化
- **OpenAI兼容API**: 完全兼容OpenAI ChatCompletion接口

## 技术架构

### 核心组件
- **Web框架**: axum + tokio (替代FastAPI + asyncio)
- **HTTP客户端**: reqwest (替代AsyncOpenAI)
- **序列化**: serde (替代Pydantic)
- **日志**: tracing (替代logging)
- **配置**: dotenvy (替代dotenv)

### 性能目标
- 响应延迟减少30-50%
- 并发能力提升3-5倍
- 内存使用减少50-70%
- 启动时间提升10倍以上

## 开发计划

### Phase 1: 核心实现
1. ✅ 项目初始化和环境配置
2. 🔄 数据结构和API定义
3. ⏳ 双模型并发调用逻辑
4. ⏳ 流式响应处理
5. ⏳ 性能统计和监控

### Phase 2: 测试和优化
1. ⏳ 单元测试和集成测试
2. ⏳ 性能基准测试
3. ⏳ 代码审查和优化
4. ⏳ 文档完善

### Phase 3: 高级特性
1. ⏳ 配置热重载
2. ⏳ 健康检查增强
3. ⏳ 指标导出
4. ⏳ 容器化部署

## 参考代码

原Python实现保存在 `references/` 目录:
- `references/main.py` - 主程序
- `references/client.py` - 测试客户端

## 开发规范

- 代码质量: 使用clippy和rustfmt保证代码质量
- 测试驱动: 每个功能模块都要有对应测试
- 性能优先: 关注零拷贝、避免不必要分配
- 类型安全: 充分利用Rust类型系统
- 错误处理: 使用Result类型，避免panic

## 配置参数

### 小模型配置
- `SMALL_MODEL_API_KEY`: 小模型API密钥
- `SMALL_MODEL_BASE_URL`: 小模型API基础URL
- `SMALL_MODEL_NAME`: 小模型名称

### 大模型配置  
- `LARGE_MODEL_API_KEY`: 大模型API密钥
- `LARGE_MODEL_BASE_URL`: 大模型API基础URL
- `LARGE_MODEL_NAME`: 大模型名称

### 服务配置
- `HOST`: 服务监听地址 (默认: 0.0.0.0)
- `PORT`: 服务监听端口 (默认: 8000)
- `LOG_LEVEL`: 日志级别 (默认: info)

## API端点

- `POST /v1/chat/completions` - OpenAI兼容的聊天完成接口
- `GET /` - 服务状态信息
- `GET /health` - 健康检查
- `GET /metrics` - 性能指标
- `POST /metrics/reset` - 重置指标

## 快速开始

```bash
# 设置环境变量
cp .env.example .env
# 编辑 .env 文件配置API密钥

# 运行服务
cargo run

# 测试API
cargo test
```