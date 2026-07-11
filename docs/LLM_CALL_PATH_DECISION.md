# StoryMoss LLM 调用路径架构决策

> 决策日期: 2026-04-14  
> 决策状态: **已生效**  
> 相关文件: `src-frontend/src/services/modelService.ts`, `src-tauri/src/llm/commands.rs`, `src-frontend/src/services/tauri.ts`

---

## 背景

前端存在两套 LLM 调用路径：
1. **HTTP 直连** (`modelService.ts`): 前端直接通过 `fetch` 调用本地/远程 OpenAI 兼容 API，支持 SSE 流式输出、AbortController 取消、模型状态探测。
2. **Tauri 命令** (`llm/commands.rs`): 后端注册了 `llm_generate`、`llm_generate_stream`、`llm_cancel_generation`、`llm_test_connection` 等命令，但前端未使用。

## 决策

**保留 HTTP 直连 (`modelService.ts`) 作为前端唯一官方 LLM 调用路径。**

### 理由
- **流式输出成熟**: 前端已有完整的 SSE 解析逻辑、逐字渲染、错误处理。
- **取消机制完善**: `AbortController` 可以精确取消进行中请求，用户体验好。
- **状态探测已集成**: 设置页的模型状态灯（绿/红/加载）直接依赖 HTTP 探测 `/models` 和 `/chat/completions`。
- **Tauri 流式命令复杂**: 将 SSE 流式响应完整迁移到 Tauri 命令层需要重写事件推送、取消令牌管理，且当前后端实现未经验证。
- **无安全需求驱动**: 目前模型 API Key 和端点均存储在前端配置中，走 Tauri 层并不能带来额外的安全隔离。

### Tauri 侧 LLM 命令的处理

后端 `llm/commands.rs` 及相关模块（`llm/service.rs`、`llm/adapter.rs`）**保留但降级为内部/备用用途**：
- 不从前端新组件中调用这些命令。
- 允许后端其他模块（如 Skills、Agents 内部）在需要时通过 `LlmService` 直接调用。
- 已在前端 `services/tauri.ts` 中将 `llmGenerate*` 系列函数标记为 `@deprecated — 保留备用，请勿在新功能中使用`。

## 影响

- 前端新功能（如写作助手、知识蒸馏、评点家等）应统一通过 `modelService.ts` 调用 LLM。
- 若未来需要支持非 HTTP 协议的本地模型（如直接加载 GGUF），再评估是否启用 Tauri 层调用路径。

## 相关审计项

本决策对应 `docs/API_CONSISTENCY_AUDIT.md` 中 "P3 - 架构决策" 第 5 项。
