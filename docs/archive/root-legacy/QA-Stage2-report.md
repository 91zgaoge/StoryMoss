# StoryMoss 阶段二修复验证报告（QA-Stage2）

> 验证人：B5
> 日期：2026-06-13
> 目标：在阶段一解决「生成无输出」问题后，确认 B1–B4 对前端响应与大数据量场景的 8 项优化已完整落地，并补充关键回归测试。

---

## 1. 阶段二覆盖度检查表

| # | 修复目标 | 状态 | 依据（文件/函数） |
|---|---------|------|------------------|
| 1 | 字数统计增量化 | **已覆盖** | `src-frontend/src/frontstage/FrontstageApp.tsx`：B1 将全文字数从「全量 scenes content reduce」改为「当前章节字数增量 diff」更新（`totalWordCount` + `currentChapterPrevWordCountRef`）。`src-frontend/src/hooks/useExecutionState.ts`：B2 引入 `get_story_word_count`，由后端 SQL `COALESCE(SUM(LENGTH(content)), 0)` 聚合返回 `total_chars`，避免全量 IPC。 |
| 2 | 场景/章节数据分页与延迟加载 | **已覆盖** | 后端：`src-tauri/src/db/repositories.rs` 新增 `SceneRepository::get_by_story_paged` / `count_by_story` / `total_content_length_by_story` 及对应 `ChapterRepository` 分页方法；`src-tauri/src/scene_commands.rs` 新增 `get_story_scenes_paged`、`get_story_word_count`；`src-tauri/src/commands/chapter.rs` 新增 `get_story_chapters_paged`；`src-tauri/src/handlers.rs` 注册上述命令。前端：`src-frontend/src/hooks/useScenes.ts` 新增 `useScenesPaged`；`src-frontend/src/hooks/useChapters.ts` 新增 `useChaptersPaged`；`src-frontend/src/services/api/stories.ts` 新增 `getStoryScenesPaged`、`getStoryChaptersPaged`、`getStoryWordCount`。 |
| 3 | 合并 sync-event 失效 | **已覆盖** | `src-frontend/src/hooks/useSyncStore.ts`：`dataRefresh` 分支中 `case 'all'` 使用单一 `queryClient.invalidateQueries({ predicate: ... })`，将 B1 之前 9 次独立 `invalidateQueries` 合并为一次 predicate 批量刷新；若后端附带 `affected_resources`，只失效相关 key。 |
| 4 | LanceDB 查询优化 | **已覆盖** | `src-tauri/src/vector/lancedb_store.rs`：`search` 使用 `only_if_expr(col("story_id").eq(lit(story_id)))` 参数化过滤；`text_search` 对用户输入做 `escape_like_pattern` 并构造 `text LIKE prefix%` 前缀匹配，避免全表扫描与 SQL 注入；`hybrid_search` 的 RRF 融合逻辑移入 `tokio::task::spawn_blocking`，避免阻塞异步运行时。 |
| 5 | Embedding 批处理 | **已覆盖** | `src-tauri/src/embeddings/provider.rs`：`OpenAIEmbeddingProvider::embed` 先查缓存，缺失文本按 `max_batch_size=100` 分块，调用 OpenAI `/v1/embeddings` 原生批量接口；`OllamaEmbeddingProvider::embed` 检测 `/api/embed` 批量能力并回退单条，同样按 `max_batch_size` 分块。 |
| 6 | 前端状态拆分 | **已覆盖** | `src-frontend/src/stores/generationStore.ts`：新建 Zustand store，独立承载 `isGenerating`、`generationStatus`、`orchestratorStatus`。`src-frontend/src/stores/bootstrapStore.ts`：新建 store 承载 `bootstrapProgress`。`src-frontend/src/frontstage/FrontstageApp.tsx` 从 store 读取/写入，减少单点状态变化导致的整树重渲染。 |
| 7 | 文思分析异步化 | **已覆盖** | `src-frontend/src/frontstage/ai-perception/textAnalyzer.worker.ts`：将 `analyzeText` 移入 Web Worker，支持任务取消与结果缓存。`src-frontend/src/frontstage/ai-perception/asyncTextAnalyzer.ts`：提供 `analyzeTextAsync(htmlContent, signal?)`，Worker 不可用时自动降级为同步分析，AbortSignal 可取消任务。`src-frontend/src/frontstage/ai-perception/SmartHintSystem.tsx`：使用 `analyzeTextAsync` 并持有 `AbortController` 取消上一次未完成的分析。 |
| 8 | RichTextEditor HTML 序列化节流 | **已覆盖** | `src-frontend/src/frontstage/components/RichTextEditor.tsx`：`onUpdate` 中先用 `editor.getText()` 更新轻量状态，再用 `setTimeout(..., 200)` 防抖序列化完整 HTML；卸载时 flush 最终 HTML。 |

---

## 2. 新增 / 补充测试清单

| 测试位置 | 测试内容 | 作用 |
|---------|---------|------|
| `src-tauri/src/db/repositories_tests.rs` | `test_scene_total_content_length_matches_scene_contents` | 验证 `SceneRepository::total_content_length_by_story` 与场景 `content` 字符总数一致；删除场景后聚合结果同步减少。 |
| `src-frontend/src/hooks/__tests__/useSyncStore.bug.spec.ts` | `DataRefresh { resource_type: "all" } batches invalidation into a single invalidateQueries call` | 断言 `dataRefresh/all` 只触发一次 `queryClient.invalidateQueries`，且 predicate 正确命中该 storyId 下所有受控 key。 |
| `src-frontend/src/hooks/__tests__/useSyncStore.bug.spec.ts` | `DataRefresh { resource_type: "all", affected_resources } only invalidates specified resources` | 断言 `affected_resources` 存在时，仅失效指定资源，其他资源 key 不命中。 |
| `src-frontend/src/frontstage/ai-perception/__tests__/asyncTextAnalyzer.test.ts` | `returns a valid PerceptionResult when Worker is unavailable` | 验证 jsdom/SSR 降级路径返回完整分析结果。 |
| `src-frontend/src/frontstage/ai-perception/__tests__/asyncTextAnalyzer.test.ts` | `rejects immediately when the AbortSignal is already aborted` | 验证已取消信号立即 reject。 |
| `src-frontend/src/frontstage/ai-perception/__tests__/asyncTextAnalyzer.test.ts` | `cancels an in-flight task and notifies the Worker` | 模拟 Worker，验证取消时发送 `cancel` 消息且 Promise reject。 |
| `src-frontend/src/frontstage/ai-perception/__tests__/asyncTextAnalyzer.test.ts` | `resolves when Worker returns a result before cancellation` | 模拟 Worker 正常回传结果，验证 Promise resolve。 |
| `src-frontend/src/frontstage/components/__tests__/RichTextEditor.debounce.test.tsx` | `debounces onChange callback by 200ms after editor updates` | 使用 Vitest fake timers，验证连续 `onUpdate` 只触发一次 `onChange`，且 200ms 后最终调用。 |

> 本次验证过程中发现并修复了一个生产代码小 bug：`asyncTextAnalyzer.ts` 的 abort handler 原先仅清理 pending task 并通知 Worker，未 reject Promise，导致取消后 Promise 永远悬停。已补充 `reject(signal!.reason)`，确保 AbortSignal 能真正取消任务。

---

## 3. 测试结果汇总

| 测试命令 | 结果 |
|---------|------|
| `cd src-tauri && cargo check --all-targets` | ✅ 通过（3 个 pre-existing `dead_code` warning，与修复前一致） |
| `cd src-tauri && cargo test --lib` | ✅ **344 passed**, 0 failed, 0 ignored（修复前 343 passed） |
| `cd src-tauri && cargo test --all-targets` | ✅ **344 passed**, 0 failed, 0 ignored |
| `cd src-frontend && npx tsc --noEmit` | ✅ 通过 |
| `cd src-frontend && npm run test:run` | ✅ **124 passed**, 3 skipped, 0 failed（修复前 117 passed） |

> 新增 Rust 测试 1 个，新增前端测试 6 个，全部通过。

---

## 4. 遗漏问题与阶段三建议

以下问题**不在阶段二 8 项目标内**，但初始审计或阶段一遗留建议中标记为可能造成卡顿/阻塞/内存压力，当前代码中仍未完全解决：

| 模块 | 问题 | 风险 | 建议阶段三处理 |
|------|------|------|----------------|
| `src-tauri/src/commands/anti_ai.rs` + `src-tauri/src/anti_ai/mod.rs` | `anti_ai_review` 仍是同步 `#[tauri::command]`，直接在 async runtime worker 上执行大文本五维审查。 | 用户触发 Anti-AI 审查时可能阻塞生成任务的 tokio worker。 | 改为 `async` command，内部 `spawn_blocking`；或提供流式/分块审查。 |
| `src-tauri/src/book_deconstruction/service.rs::upload_and_analyze` | 第 96 行在 async command 中直接同步调用 `parse_book(file_path, None)?`；虽然 executor 中已用 `spawn_blocking`，但上传路径仍阻塞。 | 大文件（PDF/EPUB）解析会阻塞 tokio worker。 | 将 `upload_and_analyze` 中的 `parse_book` 移入 `spawn_blocking`。 |
| `src-tauri/src/memory/writer.rs` + `src-tauri/src/memory/ingest.rs` | `MemoryWriter::write` 与 `IngestPipeline::ingest` 通过 `tauri::async_runtime::spawn` 后台执行，无取消令牌、无并发/背压控制。 | 高频连续生成时后台 ingest 任务堆积，与下一轮 writer 竞争资源。 | 为后台任务添加取消令牌（与生成取消联动）和最大并发/队列限制（如 Semaphore）。 |
| `src-frontend/src/components/KnowledgeGraph/KnowledgeGraphView.tsx` | 使用 `reactflow` 全量渲染节点/边，无虚拟化或 viewport 裁剪。 | 知识图谱节点数大时渲染与交互卡顿。 | 增加节点数量上限、LOD（层级细节）、viewport 外裁剪或分页加载。 |
| `src-tauri/src/memory/tokenizer.rs` | 仅有 CJK bigram 分词器，未接入真实 tokenizer（如 tiktoken、Qwen tokenizer），也没有全局上下文预算管理。 | LLM 上下文长度估算不准确，可能导致截断或 token 超限。 | 引入模型对应的真实 tokenizer，并在 context builder / agent 中增加上下文预算与截断策略。 |

补充说明：
- **前端事件聚合**：`src-frontend/src/hooks/useBackendActivityListener.ts` 已将多个进度事件合并为单一主 activity，该项已在既有代码中实现，不在阶段三建议列表中重复提出。
- **阶段二 8 项目标均已在代码层面落地**，但 LanceDB/Ollama 等涉及真实外部服务的集成场景未在本次验证中运行（遵循约束，仅通过单元测试/mock 覆盖）。

---

## 5. 是否推荐进入阶段三

**推荐进入阶段三。**

理由：
1. 阶段二 8 项目标均已实现并通过代码审查；前端响应与大数据量路径的分页、聚合、状态拆分、异步化、节流已覆盖。
2. 新增 7 个回归测试全部通过，后端测试从 343 → 344，前端测试从 117 → 124。
3. 编译、类型检查、全量单元测试均绿色。
4. 阶段三可针对上述 5 个遗留热点（Anti-AI 同步审查、拆书上传解析阻塞、Ingest 取消/背压、知识图谱虚拟化、真实 tokenizer 与上下文预算）进行专项治理，避免影响阶段二已修复的体验。
