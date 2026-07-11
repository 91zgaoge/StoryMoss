# StoryMoss 阶段一修复验证报告（QA-Stage1）

> 验证人：A5  
> 日期：2026-06-13  
> 目标：确认 A1–A4 对「生成无响应/无输出」问题的修复覆盖度，并补充关键回归测试。

---

## 1. 阶段一覆盖度检查表

| # | 修复目标 | 状态 | 依据（文件/函数） |
|---|---------|------|------------------|
| 1 | 候选阶段并发与超时重构 | **已覆盖** | `src-tauri/src/agents/orchestrator.rs`：`AgentOrchestrator::generate_candidates` / `generate_candidates_inner`。本地模型固定 `effective_count=1`；远端取 `candidate_count.max(1).min(2)`；单个候选超时硬上限本地 60s/远端 120s；总超时 `per_candidate * count + 30` 再 `min(90s)`；候选强制 `join_all` 并行；`retries_override=0`。 |
| 2 | LLM 调用层超时与取消加固 | **已覆盖** | `src-tauri/src/llm/adapter.rs`：`send_with_connection_timeout` / `read_body_with_generation_timeout` 分阶段标记 `CONNECTION_TIMEOUT_MARKER` / `GENERATION_TIMEOUT_MARKER`。`src-tauri/src/llm/service.rs`：`execute_generation` 对连接超时重试 1 次、生成/其他超时不再重试；`generate_stream` 有 30–120s 启动超时 + 60s chunk 超时；`cancel_generation` / `cancelled_requests` 支持协作式取消。`src-tauri/src/error.rs` 新增 `LlmConnectionTimeout` / `LlmGenerationTimeout` / `Cancellation`。 |
| 3 | 写作上下文准备 spawn_blocking 化 | **已覆盖** | `src-tauri/src/agents/service.rs`：`prepare_writer_context_inner` 将场景查询、连续性检查、CanonicalState 聚合分别包入 `tokio::task::spawn_blocking`；外层套 60s 整体超时。`src-tauri/src/creative_engine/context_builder.rs`：`build` 把 `build_core_sync` 整体移入 `spawn_blocking`。 |
| 4 | 提示词构建减少重复 IO | **已覆盖** | `src-tauri/src/agents/service.rs` 引入进程级只读缓存 `WRITER_APP_CONFIG`、`WRITER_GENRE_PROFILES`、`WRITER_STYLE_DNAS`；`build_writer_prompt` 通过 `writer_app_config()` 读取缓存，避免每次从磁盘/数据库加载。 |
| 5 | SQLite 高频路径 spawn_blocking 化 | **已覆盖** | `src-tauri/src/scene_commands.rs`：`update_scene` 核心 DB 查询/更新移入 `spawn_blocking`。`src-tauri/src/creation_commands.rs`：`create_story_with_wizard` 的两个事务段均使用 `spawn_blocking`。`StoryContextBuilder` 中大量 repository 调用也已在线程池执行。 |
| 6 | 全局 Mutex 替换 | **已覆盖（核心路径）** | `src-tauri/src/lib.rs`：`DB_POOL` 改用 `std::sync::OnceLock`。`src-tauri/src/llm/service.rs`：`LLM_SERVICE` 改为 `OnceCell<Arc<LlmService>>`，`get_llm_service()` 直接 clone。`src-tauri/src/creative_engine/context_builder.rs`：`ContextCache` 使用 `tokio::sync::RwLock` + `try_read`/`try_write`。遗留全局 Mutex（`APP_CONFIG`、`SKILL_MANAGER` 等）不在本次热路径上。 |
| 7 | 前端生成状态可取消与反馈 | **已覆盖** | `src-frontend/src/frontstage/FrontstageApp.tsx`：`handleCancelGeneration` 调用 `agent_cancel_all_tasks`、停止 `typewriterFrameRef` rAF、清理 `isGenerating` 与 `backendActivityStore` 残留活动。`src-frontend/src/hooks/useBackendActivityListener.ts` 新增精确阶段映射（准备上下文/候选生成/Inspector/改写/最终输出/保存记忆）。 |
| 8 | 前端输入路径减负 | **已覆盖** | `src-frontend/src/frontstage/autoSave.ts`：`scheduleAutoSave` 支持 payload getter，使用 `requestIdleCallback` 与 `startTransition`，避免输入关键路径同步序列化/IPC。 |
| 9 | 关闭/替换高频心跳 | **已覆盖** | `src-frontend/src/frontstage/components/FrontstageBottomBar.tsx` 移除 1s `setInterval` 心跳，进度脉冲改用 CSS `@keyframes`。`useBackendActivityListener` 基于事件监听而非轮询。后端心跳仅在生成期间运行，生成结束 `AbortOnDrop` 终止。 |

---

## 2. 新增 / 补充测试清单

| 测试位置 | 测试内容 | 作用 |
|---------|---------|------|
| `src-tauri/src/config/settings_tests.rs` | `test_is_private_url_recognizes_local_addresses` / `test_is_private_url_rejects_public_addresses` | 验证 `is_private_url` 对 localhost/127.0.0.1/::1/10.x/172.16-31.x/192.168.x 的判定，及公网地址的排除。 |
| `src-tauri/src/config/settings_tests.rs` | `test_default_writer_concurrency_values` | 确认 `writer_local_concurrency=1`、`writer_remote_concurrency=2`。 |
| `src-tauri/src/config/settings_tests.rs` | `test_default_candidate_timeout_values` | 确认默认 `candidate_timeout_seconds=120`、`candidate_timeout_local_seconds=60`、`candidate_count=1`、`candidate_max_retries=0`。 |
| `src-tauri/src/llm/service.rs` | `test_connection_timeout_is_retriable_but_generation_timeout_is_not` | 验证 `LlmService::is_retriable_error`：连接超时可重试，生成超时/通用超时不可重试。 |
| `src-tauri/src/llm/service.rs` | `test_timeout_error_codes` | 验证 `LlmConnectionTimeout`、`LlmGenerationTimeout`、`LlmTimeout`、`Cancellation` 的错误码。 |
| `src-tauri/src/agents/orchestrator.rs` | `test_workflow_config_clamps_candidate_count` | 验证 `WorkflowConfig::from_app_config` 将 `candidate_count` 限制在 2 以内。 |
| `src-tauri/src/agents/orchestrator.rs` | `test_candidate_total_timeout_never_exceeds_90s` | 按 `generate_candidates` 的公式验证本地 1 候选、远端默认 1 候选、2 候选配置下总超时均 ≤ 90s。 |
| `src-frontend/src/frontstage/__tests__/FrontstageApp.test.tsx` | `取消生成应调用 agent_cancel_all_tasks 并清理前端状态` | 模拟生成中点击取消按钮，断言 `agent_cancel_all_tasks` 被调用，输入框恢复可编辑、发送按钮恢复。 |

---

## 3. 测试结果汇总

| 测试命令 | 结果 |
|---------|------|
| `cd src-tauri && cargo check --all-targets` | ✅ 通过（3 个 pre-existing `dead_code` warning，与修复前一致） |
| `cd src-tauri && cargo test --lib` | ✅ **341 passed**, 0 failed, 0 ignored（修复前 333 passed） |
| `cd src-tauri && cargo test --all-targets` | ✅ **341 passed**, 0 failed, 0 ignored |
| `cd src-frontend && npx tsc --noEmit` | ✅ 通过 |
| `cd src-frontend && npm run test:run` | ✅ **117 passed**, 3 skipped, 0 failed（修复前 116 passed） |

> 新增 Rust 测试 8 个，新增前端测试 1 个，全部通过。

---

## 4. 遗漏问题与建议（建议阶段二优先处理）

以下问题**不在阶段一 9 项目标内**，但初始审计 P0/P1/P2 中标记为可能造成卡顿/阻塞，当前代码中仍未修复：

| 模块 | 问题 | 风险 | 建议阶段二处理 |
|------|------|------|----------------|
| `src-tauri/src/anti_ai/mod.rs` + `commands/anti_ai.rs` | `anti_ai_review` 是同步 `#[tauri::command]`，直接在 async runtime worker 上执行大文本五维审查。 | 用户触发 Anti-AI 审查时可能阻塞生成任务的 tokio worker。 | 改为 `async` command，内部 `spawn_blocking`；或提供流式/分块审查。 |
| `src-tauri/src/book_deconstruction/service.rs::upload_and_analyze` | 第 96 行在 async command 中直接同步调用 `parse_book(file_path, None)?`；只有后续 executor 中才用 `spawn_blocking`。 | 大文件（PDF/EPUB）解析会阻塞 tokio worker。 | 将 `upload_and_analyze` 中的 `parse_book` 移入 `spawn_blocking`，或把解析完全交给任务执行器。 |
| `src-tauri/src/vector/lancedb_store.rs::hybrid_search` | RRF 融合逻辑在 async 函数中同步执行。当前 `top_k` 较小，开销低，但随数据量增大会成为 CPU 阻塞点。 | 向量搜索路径潜在的同步阻塞。 | 对 RRF 排序/截断使用 `spawn_blocking`，或确保调用点不占用 writer worker。 |
| `src-tauri/src/agents/orchestrator.rs::generate` 成功后 | `MemoryWriter::write` + `IngestPipeline::ingest` 通过 `tauri::async_runtime::spawn` 后台执行，无取消令牌、无并发/背压控制。 | 高频连续生成时后台 ingest 任务堆积，与下一轮 writer 竞争资源，可能导致响应变慢。 | 为后台任务添加取消令牌（与生成取消联动）和最大并发/队列限制（如 Semaphore）。 |

---

## 5. 是否推荐进入阶段二

**推荐进入阶段二。**

理由：
1. 阶段一 9 项目标均已实现并通过代码审查；核心「生成无输出/无响应」路径的超时、取消、并发、spawn_blocking 化已覆盖。
2. 新增 9 个回归测试全部通过，后端测试从 333 → 341，前端测试从 116 → 117。
3. 编译、类型检查、全量单元测试均绿色。
4. 阶段二可针对上述 4 个遗漏的性能/阻塞热点进行专项治理，避免影响阶段一已修复的体验。
