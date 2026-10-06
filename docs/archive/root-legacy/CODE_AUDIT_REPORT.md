# StoryMoss (草苔) v3.5.0 代码审计与优化计划

> 审计日期: 2026-04-22
> 审计范围: 全项目前后端代码对照功能设计检查
> 测试状态: Rust 139 tests / 前端 21 tests 全部通过

---

## 一、已确认正常运行的功能 ✅

| 功能模块 | 实现状态 | 说明 |
|---------|---------|------|
| **LLM 适配层** | ✅ 完整 | OpenAI / Anthropic / Ollama 三适配器，同步+流式双模式 |
| **Agent 系统** | ✅ 完整 | 8 种 Agent（Writer/Inspector/OutlinePlanner/StyleMimic/PlotAnalyzer/MemoryCompressor/Commentator/KnowledgeDistiller） |
| **手工续写** | ✅ 正常 | `writer_agent_execute` 正确传递 `current_content` + `selected_text` 到 Writer Agent |
| **自动续写** | ⚠️ 有缺陷 | 循环调用 Writer Agent 正常，但**未传递当前内容作为上下文** |
| **小说创建向导** | ✅ 完整 | 世界观→角色谱→文风→首个场景，四步 JSON 输出 |
| **提示词模板引擎** | ✅ 完整 | `TemplateEngine` + `PromptLibrary`，支持变量替换和条件渲染 |
| **创作方法论引擎** | ✅ 完整 | 雪花法/场景节拍/英雄之旅/人物深度，可注入系统提示词 |
| **StyleDNA 系统** | ✅ 完整 | 六维模型 + 10 种经典作家 DNA + 提示词注入 |
| **自适应学习系统** | ⚠️ 部分 | 反馈记录/偏好挖掘/个性化提示词构建完整，但**未影响实际 LLM 调用参数** |
| **创作工作流引擎** | ✅ 完整 | 7 阶段闭环（构思→大纲→场景→写作→审阅→迭代→入库） |
| **四层记忆系统** | ✅ 完整 | IngestPipeline + QueryPipeline + KnowledgeGraph + VectorStore |
| **拆书功能** | ✅ 完整 | 解析→分块→LLM 分析→保存→一键转故事 |
| **任务系统** | ✅ 完整 | 调度器/心跳/执行器注册表/进度推送 |
| **付费订阅** | ✅ 完整 | 配额检查/消费/分层 Agent 质量 |
| **前端 AI 感知层** | ✅ 完整 | SmartHintSystem 纯前端分析，零后端延迟 |
| **版本/修订/批注** | ✅ 完整 | 场景版本、修订模式、文本批注、评论线程 |
| **测试** | ✅ 全部通过 | Rust 139 tests + 前端 21 tests |

---

## 二、发现的问题 🔴

### 🔴 P0 - 严重缺陷（影响核心功能）

#### 1. 自动续写丢失当前内容上下文
**位置**: `src-tauri/src/agents/commands.rs:400-403`
```rust
let _current_content = chapter_repo.get_by_id(&request.chapter_id)
    .map_err(|e| e.to_string())?
    .map(|c| c.content.unwrap_or_default())
    .unwrap_or_default();
```
- `_current_content` 前缀下划线 = **未使用**
- `build_agent_context` 中 `current_content = None`，由调用方填充，但 `auto_write` 没有填充
- **后果**: AI 续写时完全不知道前文写了什么，生成内容可能与故事脱节

#### 2. 后端流式输出已实现，前端完全未使用
**位置**: `src-tauri/src/llm/commands.rs:46-59`（后端已实现）
- 后端 `llm_generate_stream` 通过 SSE 逐字推送，事件名 `llm-stream-chunk-{request_id}`
- 前端没有任何地方调用 `llm_generate_stream`
- `chat_completion` 命令中也是 `"stream": false`
- `useStreamingGeneration` 只是**本地打字机动画效果**，先等完整响应再逐字显示
- **后果**: 用户必须等待完整生成后才能看到任何文字，长文本时体验差

#### 3. 关键 TODO 未实现
| TODO | 位置 | 影响 |
|------|------|------|
| LLM 取消生成 | `llm/commands.rs:95` | 用户无法取消正在进行的 LLM 调用 |
| Agent 状态跟踪 | `agents/commands.rs:223` | 无法查询 Agent 任务执行状态 |
| 图像生成模型 | `config/commands.rs:398` | 预留功能未实现 |

### 🟡 P1 - 重要缺陷（影响质量与体验）

#### 4. AgentOrchestrator 未集成到续写流程
**位置**: `src-tauri/src/agents/orchestrator.rs`
- 虽然完整实现了 Writer→Inspector→Writer 质量反馈闭环
- 但 `auto_write` 和 `writer_agent_execute` 都没有调用它
- **后果**: 创作方法论中宣传的"质检反馈循环"在实际续写中不起作用

#### 5. 连续性引擎与伏笔追踪未集成到写作流程
**位置**: `src-tauri/src/creative_engine/continuity.rs` + `foreshadowing.rs`
- `ContinuityEngine::check_scene_continuity` 和 `ForeshadowingTracker` 模块存在
- 但 `AgentService::execute_writer` 中完全没有调用
- **后果**: AI 写作时不会检查角色位置一致性、世界观规则冲突、伏笔回收

#### 6. 自适应学习策略未影响实际 LLM 参数
**位置**: `src-tauri/src/creative_engine/adaptive/generator.rs`
- `AdaptiveGenerator::build_strategy` 会根据用户偏好计算 `temperature` 调整（如 -0.05 / +0.05）
- 但 `AgentService::execute_writer` 中固定传 `Some(0.8)` temperature
- **后果**: "越写越懂"的学习成果只体现在提示词文本中，未真正调节生成参数

#### 7. 自动续写后未触发知识图谱 Ingest
**位置**: `src-tauri/src/agents/commands.rs:383-519`
- `update_scene` 在内容更新后会自动触发 `IngestPipeline`
- 但 `auto_write` 通过事件追加内容到前端，没有保存到数据库，更不会触发 Ingest
- **后果**: 自动续写生成的大量新内容不会进入知识图谱，"越写越懂"效果打折扣

### 🟢 P2 - 一般缺陷（可优化）

#### 8. 部分 Prompt 为英文，与中文写作定位不符
- `OpenAiAdapter::build_messages` 系统提示: "You are a professional creative writing assistant."
- `InspectorAgent::build_prompt` 全英文
- **后果**: 对中文模型（如 Qwen/DeepSeek）可能产生语言混杂

#### 9. Token 使用量估算过于粗糙
**位置**: `src-tauri/src/llm/service.rs:234`
```rust
tokens_used: full_text.len() as i32 / 2, // 粗略估计
```
- 未使用 tiktoken 或近似算法，对中文尤其不准确
- **后果**: 成本估算和用量监控不准

#### 10. 质量评分过于简单
**位置**: `src-tauri/src/agents/service.rs:707-714`
```rust
fn calculate_quality_score(&self, content: &str) -> f32 {
    let length_score = (content.len() as f32 / 500.0).min(1.0);
    let sentence_count = content.split(['。', '！', '？']).count() as f32;
    let variety_score = (sentence_count / 5.0).min(1.0);
    (length_score * 0.4 + variety_score * 0.6).min(1.0)
}
```
- 只基于文本长度和句子数量，不检查内容质量
- `parse_inspection_result` 也是简单字符串匹配（"90"→0.9）

#### 11. StreamingText 组件孤立
- `StreamingText.tsx` 实现完整但未被任何主页面引用
- `FrontstageToolbar` 的 `onRequestGeneration` 直接触发 `writer_agent_execute` 而非流式组件

#### 12. 自动续写仍使用旧版 ChapterRepository
- 项目核心已迁移到 Scene 模型，但 `auto_write` 仍读取 `ChapterRepository`
- 场景化叙事的戏剧结构信息未传递给续写 Agent

---

## 三、完善与优化计划 📋

### Phase A: 修复严重缺陷（P0）

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| A1 | **自动续写传递当前内容** | `agents/commands.rs` | 修改 `auto_write` 将 `_current_content` 注入 `AgentContext.current_content` |
| A2 | **前端接入真实流式生成** | 新增/修改前端 Hook | 创建 `useLlmStream` Hook，调用 `llm_generate_stream`，监听 `llm-stream-chunk-*` 事件 |
| A3 | **实现 LLM 取消机制** | `llm/commands.rs` + `llm/service.rs` | 在 `LlmService` 中加入 `Arc<Mutex<HashMap<String, AbortHandle>>>` 管理进行中的流式任务 |
| A4 | **实现 Agent 状态跟踪** | `agents/commands.rs` | 将 `TASK_HANDLES` 扩展为包含状态信息（running/completed/failed） |

### Phase B: 集成核心引擎（P1）

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| B1 | **AgentOrchestrator 集成到续写** | `agents/commands.rs` | `auto_write` 和 `writer_agent_execute` 可选调用 `execute_write_with_inspection` |
| B2 | **连续性检查集成到 Writer** | `agents/service.rs` | `execute_writer` 生成后调用 `ContinuityEngine::check_scene_continuity` |
| B3 | **自适应策略影响 LLM 参数** | `agents/service.rs` | `execute_writer` 中根据 `AdaptiveGenerator::build_strategy` 的结果设置 temperature/top_p |
| B4 | **自动续写后自动 Ingest** | `agents/commands.rs` | `auto_write` 完成后将追加内容保存并触发 `IngestPipeline` |
| B5 | **自动续写迁移到 Scene 模型** | `agents/commands.rs` | 将 `ChapterRepository` 替换为 `SceneRepository`，传递场景结构信息 |

### Phase C: 体验与质量优化（P2）

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| C1 | **中文化系统 Prompt** | `llm/openai.rs` + `agents/inspector.rs` | 将英文系统提示改为中文 |
| C2 | **Token 估算精确化** | `llm/service.rs` | 引入基于字符的近似算法（中文≈1字1token，英文≈4字3token） |
| C3 | **质量评分增强** | `agents/service.rs` | 增加词汇多样性、对话比例、描写密度等维度 |
| C4 | **StreamingText 组件接入** | `FrontstageApp.tsx` | 将 `StreamingText` 替换/接入当前的 generatedText 显示逻辑 |

---

## 四、实施记录

### 2026-04-22 - Phase A 实施中
- [ ] A1: 自动续写传递当前内容
- [ ] A2: 前端接入真实流式生成
- [ ] A3: 实现 LLM 取消机制
- [ ] A4: 实现 Agent 状态跟踪

### 待实施
- Phase B: 集成核心引擎
- Phase C: 体验与质量优化

---

*本报告由 AI 代码审计生成，用于指导项目优化方向。*
