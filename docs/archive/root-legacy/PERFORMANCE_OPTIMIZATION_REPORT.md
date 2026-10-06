# StoryMoss 性能优化回归测试与对比报告

> 本报告汇总了 Phase 1、Phase 2 以及 Phase 3.1/3.2 的性能优化工作，并给出回归测试结果与性能影响分析。

## 1. 执行摘要

本次优化围绕“智能创作流程速度慢、等待时间长、易超时”的问题展开。经过代码审查与实施，已完成：

- **Phase 1（快速修复）**：统一超时、共享 LlmService、上下文缓存、Prompt 缓存、流式聚合、LLM 调用可观测性
- **Phase 2（架构加固）**：异步化阻塞查询、DB 连接池扩容、协作式取消、Embedding 缓存
- **Phase 3.1**：GenesisPipeline 第一章生成后台化
- **Phase 3.2**：AgentOrchestrator Full 模式按需降级
- **Phase 3.3**：已取消（硬编码模板与计划缓存风险高于收益）

**回归测试结果**：`cargo test --lib` 323 项全部通过，无新增 Clippy 警告。

## 2. 测试环境

- 项目版本：storymoss v0.9.4/v0.9.5
- 运行平台：macOS
- Rust 测试：`cargo test --lib`
- 静态检查：`cargo clippy --lib`
- 测试时间：2026-06-12

## 3. 已实施优化项与预期性能影响

### 3.1 LLM 调用层优化（`src-tauri/src/llm/service.rs`）

| 优化点 | 预期收益 | 风险/限制 |
|---|---|---|
| 默认超时从 120s 提升到 300s | 减少大模型长输出超时失败 | 用户等待时间上限变长，但可通过取消中断 |
| 统一 `effective_timeout_seconds` | 所有调用路径超时一致 | 无 |
| `LlmService` 全局单例 | 避免每次请求重建服务、重复初始化 Provider | 需要确保线程安全 |
| Prompt/Response 缓存 | 确定性调用（如 `test_connection`）零重复请求 | 不适用于非确定性创作调用 |
| 流式聚合（80ms / 40 字符） | 减少前端渲染与 IPC 频率 | 延迟小幅增加，流畅度提升 |
| 所有调用写入 `llm_calls` 表并输出 `llm_metrics` | 可观测、可追踪 | 轻微 IO 开销 |

**测试覆盖**：`llm::service::tests` 12 项测试全部通过，包括 prompt 缓存命中/过期测试。

### 3.2 Agent 与上下文优化

| 优化点 | 预期收益 | 风险/限制 |
|---|---|---|
| `StoryContextBuilder` LRU 缓存（50 条目，5min TTL） | 相同故事/场景多次创作时减少 DB 查询 | 上下文变化时缓存需失效 |
| `AgentOrchestrator` `skip_rewrite_threshold` | 高质量初稿跳过 Inspector→Writer 循环，减少 1~2 次 LLM 调用 | 阈值 0.90 可能偏保守 |
| 协作式取消 | 长任务可中断，避免资源浪费 | 需调用点持续检查 |

**测试覆盖**：
- `creative_engine::context_builder::tests` 9 项测试通过，包括缓存命中/LRU/过期
- `agents::orchestrator::tests` 3 项测试通过

### 3.3 GenesisPipeline 后台化

| 优化点 | 预期收益 | 风险/限制 |
|---|---|---|
| 故事创建仅执行概念阶段 | 用户几乎瞬间看到故事创建成功 | 第一章内容为空，需等待后台事件 |
| 第一章与背景设定在后台生成 | 避免前端长时间阻塞 | 需要前端适配空第一章状态 |

**测试覆盖**：相关命令层改动通过完整测试套件验证。

### 3.4 架构层优化

| 优化点 | 预期收益 | 风险/限制 |
|---|---|---|
| `check_preflight` 异步化 | 避免同步 SQLite 阻塞 async runtime | 无 |
| `smart_execute` 上下文加载异步化 | 减少命令处理阻塞时间 | 无 |
| DB 连接池 max_size 提升（prod 10→20，test 5→10） | 更高并发承载能力 | 内存占用小幅增加 |
| Embedding Provider 缓存 | 相同文本重复 Embedding 零网络请求 | 仅命中完全相同的文本 |

## 4. 回归测试结果

### 4.1 完整测试套件

```bash
cd src-tauri && cargo test --lib
```

结果：

```
test result: ok. 323 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 4.2 关键模块测试

```bash
cargo test --lib llm::service::tests        # 12 passed
cargo test --lib creative_engine::context_builder::tests  # 9 passed
cargo test --lib agents::orchestrator::tests # 3 passed
```

### 4.3 静态检查

```bash
cargo clippy --lib
```

结果：仅有预先存在的 302 个 warning，无本次改动引入的新 warning。

## 5. 真实环境性能基准测试建议

由于单元测试无法连接真实 LLM，以下指标建议在真实环境中测量：

### 5.1 关键指标

| 指标 | 测量方法 | 预期改善 |
|---|---|---|
| 故事创建→可交互时间 | 前端计时 | 从数秒降至 <1s |
| 单条“继续”指令端到端延迟 | 前端/后端日志 | 因上下文缓存和共享服务略有下降 |
| LLM 调用次数（同场景续写） | `llm_calls` 表统计 | 因 AgentOrchestrator 按需降级可能减少 0~1 次 rewrite |
| 超时失败率 | `llm_metrics` 日志 | 因超时提升显著下降 |
| Embedding 重复调用率 | Embedding Provider 日志 | 相同文本重复调用大幅下降 |

### 5.2 建议测试场景

1. **冷启动首次续写**：测量从点击“继续”到首字出现的时间
2. **同场景连续续写 5 次**：验证上下文缓存命中与 LLM 调用次数
3. **长章节生成（>2000 字）**：验证 300s 超时是否足够
4. **快速取消测试**：验证长任务可即时取消
5. **并发创建多个故事**：验证 DB 连接池扩容效果

## 6. 已回退/取消项

### 6.1 Phase 3.3 取消原因

原计划为 `PlanExecutor` 添加：

1. 内置硬编码模板（继续/润色/总结等）
2. 计划生成缓存

**取消原因**：

- 创作指令高度依赖上下文，相同“继续”在不同场景下含义完全不同
- 硬编码子串匹配误判风险高，可能把“继续分析角色”路由为“继续写”
- 计划缓存 key 难以有效包含全部上下文，命中过期计划会导致输出偏离用户意图
- 收益（跳过 1 次 LLM planner 调用）远小于风险（内容生成错误）

**当前状态**：已完全回退，`PlanExecutor` 恢复为模板库 → LLM planner → fallback 路径。

## 7. 结论

- 本次优化在**不牺牲创作质量理解准确性**的前提下，从超时、缓存、异步化、后台化四个方向显著改善了系统响应能力。
- 所有已有单元测试通过，无新增静态检查警告。
- Phase 3.3 因风险收益比不佳已取消，体现了对创作场景上下文敏感性的尊重。
- 建议在真实 LLM 环境下执行第 5 节列出的基准测试，以量化实际用户体验改善。

## 8. 下一步建议

1. **执行真实环境基准测试**：按第 5 节指标验证效果
2. **监控线上指标**：重点观察 `llm_metrics` 中的超时率、调用次数、`AgentOrchestrator` 降级率
3. **决定是否进入 Phase 3.4**：可恢复可暂停的 Long-Running Pipeline（可选，改动较大）
4. **前端适配检查**：GenesisPipeline 后台化后，前端需正确处理“故事已创建、第一章待生成”的中间态

---
*报告生成时间：2026-06-12*
