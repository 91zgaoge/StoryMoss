# StoryMoss 智能创作流程 × 后台资产 参考文档

> **文档定位**：本文是 StoryMoss 后端"智能创作流程"与"后台资产"的**权威参考**。
> 每次阅读项目代码时，从此文档入手即可快速掌握：资产有哪些、流程怎么走、资产在哪个环节注入、各自发挥什么作用。
>
> **代码基准**：v0.17.x（含 P0-P3 审计优化），2026-06-20
> **维护规则**：创作流程或资产架构有变动时，同步更新本文

> ⚠️ **历史参考说明**：GenesisPipeline 已于 v0.27.0 起被 Agency 多代理创作框架（`src-tauri/src/agency/`）完全取代并从代码库移除，本文档中创世分支相关内容（如 `narrative/genesis.rs`、快速/后台两阶段管线）均为历史参考。新流程见 `ARCHITECTURE.md` 的「Agency 多代理创作框架（创世 2.0）」章节与设计文档 `docs/plans/2026-07-17-agency-multi-agent-framework-design.md`。

---

## 目录

1. [全局架构总览](#1-全局架构总览)
2. [智能创作流程主线（端到端）](#2-智能创作流程主线端到端)
3. [后台资产清单与功能](#3-后台资产清单与功能)
4. [资产 × 流程注入矩阵](#4-资产--流程注入矩阵)
5. [Prompt 装配详解](#5-prompt-装配详解)
6. [写后闭环：记忆与采摘](#6-写后闭环记忆与采摘)
7. [审计与质量反馈](#7-审计与质量反馈)
8. [Pro/Free 资产分层](#8-profree-资产分层)
9. [P0-P3 优化变更索引](#9-p0-p3-优化变更索引)

---

## 1. 全局架构总览

StoryMoss 的智能创作遵循 **"用户输入 = 意图，不是命令"** 理念。用户在幕前输入一句话，系统自主理解意图、调度全套创作资产、生成专业内容。

```
用户输入（幕前）
    │
    ▼
smart_execute ────────────────────────────────────────────────────┐
    │                                                               │
    ├─ 创世分支 → GenesisPipeline（概念+首章+后台完善）              │
    │                                                               │
    └─ 续写/智能创作分支                                             │
        │                                                           │
        ├─ STEP 1-4: 加载轻量摘要（story/chapters/scenes/world/     │
        │            chars/foreshadowing/style/mcp）                 │
        ├─ build_selected_strategy（P3-2 自动匹配 GenreProfile）    │
        ├─ infer_narrative_quartet（启发式四元组推断）               │
        │                                                           │
        ▼                                                           │
    PlanContext ──▶ PlanGenerator（模型驱动，自由理解）              │
        │              └─ 消费 CapabilityRegistry 全量               │
        ▼                                                           │
    ExecutionPlan ──▶ PlanExecutor（忠实执行器）                     │
        │                                                           │
        ├─ execute_writer                                            │
        │   ├─ StoryContextBuilder::build（全量资产装配）            │
        │   ├─ narrative_quartet 注入 task.parameters                │
        │   ├─ 场景路由：选中文本→Full / 续写→TimeSliced【默认】     │
        │   └─ orchestrator.generate(task, mode)                    │
        │                                                           │
        ├─ execute_inspector / execute_skill / execute_mcp_tool      │
        │                                                           │
        ▼                                                           │
    AgentOrchestrator                                                │
        ├─ TimeSliced: WriteTimeBundle（精选资产）→ 单轮 LLM         │
        │              → 后台 AuditExecutor                         │
        ├─ Full: Writer→Inspector→Rewrite 闭环 + apply_writing_skills│
        └─ Fast: 单轮 Writer + apply_writing_skills                 │
        │                                                           │
        ▼                                                           │
    写后闭环                                                         │
        ├─ MemoryWriter::write（场景提交摘要 + memory_items）        │
        └─ IngestPipeline（KG 实体/关系 + P0-1 伏笔 + P0-2 角色状态）│
                                                                    │
    下一次续写时 ← CanonicalStateManager 读取这些写入 ──────────────┘
```

**两条写作路径的关键差异**：

| 维度 | TimeSliced（默认续写） | Full（改写/创世/向导） |
|------|------------------------|------------------------|
| 速度 | 30-60s（单轮 LLM） | 120-270s（多轮闭环） |
| 上下文来源 | WriteTimeBundle（精选） | StoryContextBuilder + build_writer_prompt（全量） |
| Inspector | 无同步 Inspector | Writer→Inspector→Rewrite 闭环 |
| Skills | 跳过 | apply_writing_skills（5 技能链） |
| 审计 | 后台异步 AuditExecutor | 同步 Inspector 驱动改写 |
| 资产覆盖 | P1-1 接入精选子集（4 类） | 全量 18 段注入 |

---

## 2. 智能创作流程主线（端到端）

### 2.1 入口：`smart_execute`

**文件**：`commands/orchestrator.rs:31`

- 外层包裹 `tokio::time::timeout`（默认 180s，可配 `smart_execute_total_timeout_secs`）
- 超时调用 `LlmService::cancel_all_generations()` 取消所有 LLM 生成
- 分两条互斥路径：**创世分支**（`is_novel_creation_intent` 命中 → `GenesisPipeline`）与 **续写分支**

### 2.2 上下文加载（STEP 1-4）

`smart_execute_inner`（`:88`）手工拉取"轻量摘要"喂给 PlanContext：

| 步骤 | 文件:行 | 加载内容 | 用途 |
|------|---------|----------|------|
| STEP 1 | `:126-159` | `StoryRepository::get_all` + `ChapterRepository::get_by_story` | story_id、chapter_count、current_content_preview（末章尾部 6000 字） |
| STEP 2 | `:409-493` | `SceneRepository::get_by_story` | scene_count、scenes_summary（最近 10 场景）、current_scene_stage、story_progress（just_started→resolution 五档） |
| STEP 3 | `:497-560` | `WorldBuildingRepository`（importance≥7）+ `CharacterRepository` + `ForeshadowingTracker::get_unresolved`（前 5） | world_building_summary、character_list、foreshadowing_status |
| STEP 4 | `:563-633` | `StoryStyleConfigRepository` → StyleBlendConfig 或 story.style_dna_id + `MCP_CONNECTIONS` 工具列表 | style_dna_info、mcp_tools_available（前 5） |

**注意**：这些摘要**只用于计划生成**，不直接进写作 prompt。写作 prompt 的资产由后续 `StoryContextBuilder` 和 `WriteTimeBundle` 独立加载。

### 2.3 策略构建：`build_selected_strategy`（P3-2 增强）

**文件**：`commands/orchestrator.rs:897-993`

**功能**：从 story 字段读取资产配置，供四元组推断使用。

**P3-2 新增**：当 story 未显式设定 `genre_profile_id`/`methodology_id`/`style_dna_id` 时，按 `story.genre` 自动匹配 GenreProfile（纯 DB 查询，无 LLM 延迟），让未配置资产的用户也能享受四元组增强。

流程：
1. 检查 story 三字段是否全为 None → 若是，按 genre 查 `GenreProfileRepository::get_by_name`
2. 读取 `genre_profile_id`（优先 story 显式设定，回退自动匹配）、`methodology_id`、`style_dna_id`
3. 查 GenreProfile 取 `canonical_name` + `reader_promise`
4. 调 `infer_narrative_quartet`（启发式补全四元组）

### 2.4 四元组推断：`infer_narrative_quartet`

**文件**：`strategy/quartet_inference.rs:15`

**功能**：基于题材画像的 `canonical_name` 和 `reader_promise`，通过启发式规则表（非 LLM）补全中文叙事四件套：

| 字段 | 来源 | 功能 |
|------|------|------|
| `emotional_payoff`（主情绪） | `reader_promise` 首个逗号分隔词 | 本作的核心情绪回报承诺 |
| `conflict_arena`（冲突场） | `default_arena_for(canonical_genre)` 规则表 | 故事的主冲突类型（生存/修仙/都市权谋…） |
| `pressure_relationship_id` | `recommend_pressure_relationship` 规则表 | 13 种高压关系之一（师徒/宿敌/青梅…） |
| `story_engine_ids` | `recommend_story_engines` 规则表 | 21 种剧情引擎（≤2 个正交组合） |
| `beat_card_ids` | `recommend_beat_card` 规则表 | 桥段卡（叙事功能模板） |

**触发条件**：仅当用户输入清晰度为 `Vague` 或 `WithSeed` 时（`needs_quartet_inference()`），`WithFullConcept` 时跳过。

### 2.5 计划生成：`PlanGenerator`

**文件**：`planner/mod.rs:151-508`

**性质**：完全模型驱动，无预设分类。LLM 自由理解意图、自选能力组合。

**进入 plan 生成 prompt 的内容**：
- 系统状态（story/chapter/scene/word_count/story_progress）
- 场景摘要（最近 10 场景）、世界观（截断 200 字）、角色（前 5）、活跃伏笔（前 5）
- 风格信息、策略文本（genre_profile_id/methodology_id/style_dna_ids/skill_ids）
- MCP 工具（前 5）、当前内容预览、用户输入
- **CapabilityRegistry 全量**（`to_llm_context()`，>4000 字时降级为硬编码 15 行核心清单）

**LLM 输出**：JSON `{understanding, steps[], fallback_message}`，每个 step 含 `capability_id`/`purpose`/`parameters`/`depends_on`

**验证**：丢弃 `capability_id` 不在注册表的 step；prose 关键词命中时强制把首步 `outline_planner` 改为 `writer`

### 2.6 计划执行：`PlanExecutor`

**文件**：`planner/executor.rs`

- 先查 `PlanTemplateLibrary`（成功计划的复用）；未命中则调 `PlanGenerator`
- `execute_plan`：拓扑分批（`swarm::topological_sort`），同批并行、跨批串行，单步 90s 超时
- `execute_step` 按 `capability_id` 前缀派发：agent → `execute_writer`/`execute_inspector`/…；`builtin.*` → `execute_skill`；`mcp.*` → `execute_mcp_tool`

### 2.7 写作执行：`execute_writer`

**文件**：`planner/executor.rs:716-868`

这是核心枢纽，负责：

1. **全量上下文构建**：调 `build_agent_context` → `StoryContextBuilder::build`（见 §3.1）
2. **四元组注入**：若 `plan_context.selected_strategy` 存在，调 `serialize_quartet_for_prompt` 序列化（含 P0-4 完整 payload），注入 `enriched_params["narrative_quartet"]`
3. **场景智能路由**（v0.14.3）：
   - 优先级：`params["mode"]` > `AppConfig.generation_mode` > 场景路由
   - `has_selected_text`（选中文本改写）→ **Full**（需质检循环）
   - 续写/新章首段 → **TimeSliced**（速度优先，问题靠后台审计修正）
4. 调 `orchestrator.generate(task, mode)`

### 2.8 生成模式分流：`AgentOrchestrator`

**文件**：`agents/orchestrator.rs:303`

#### TimeSliced（默认续写路径）

**文件**：`agents/orchestrator.rs:608-842`

1. `QuickPreflightChecker::check`（角色非空检查）
2. `WriteTimeBundle::load_sync`（精选资产，见 §5.2）
3. **P1-1 四元组注入**：从 `task.parameters["narrative_quartet"]` 提取，经 `render_narrative_quartet_section` 渲染，设置 `bundle.narrative_quartet`
4. `bundle.to_prompt()` + 用户指令 → 直接 `llm_service.generate_for_task`（绕过 `build_writer_prompt`）
5. 跳过 Inspector/Rewrite/Skills
6. spawn `AuditExecutor::run_audit`（后台异步审计）
7. 每 5 章 spawn `InsightExecutor`（深度洞察）

#### Full（质检闭环路径）

**文件**：`agents/orchestrator.rs:844-1244`

1. 候选生成（2 候选并行，若风格指纹/内容>100 字）
2. Inspector 闭环（≤ `max_feedback_loops` 轮）：
   - `service.execute_task(Inspector)` → `build_inspector_prompt`（P1-3 增强版）
   - `parse_inspector_style_analysis` 提取风格分 + 叙事分
   - `StyleChecker::check` / `check_blend`（规则法验证）
   - 双轨达标判断：style≥0.70 && narrative≥rewrite_threshold，或 composite≥skip_rewrite_threshold
   - 不达标 → `build_rewrite_feedback_dual` → Writer 改写
3. `apply_writing_skills`（5 技能链，见 §3.2）

#### Fast（实时补全路径）

**文件**：`agents/orchestrator.rs:536-594`

单轮 `execute_writer_raw` + `apply_writing_skills`，无 Inspector。

---

## 3. 后台资产清单与功能

### 3.1 上下文装配中心：`StoryContextBuilder::build`

**文件**：`creative_engine/context_builder.rs:303-569`

**功能**：Full/Fast 路径的全量资产装配。缓存 5 分钟。

**加载的资产**：

| # | 查询 | 资产类别 | 注入到 AgentContext |
|---|------|----------|---------------------|
| 1 | `fetch_story` | 故事基础 | `story.*`（title/genre/tone/pacing/genre_profile_id/description） |
| 2 | `fetch_characters` | 角色 | `narrative.characters`（含 personality/goals/appearance/gender/age） |
| 3 | `fetch_all_scenes` | 场景 | previous_chapters（最近 5）、current_scene |
| 4 | `fetch_world_rules` | 世界观 | `world.world_rules`（前 5 条规则） |
| 5 | `fetch_writing_style` | 写作风格 | `style.writing_style_*` |
| 6 | `fetch_relevant_entities` | **知识图谱** | `world.scene_structure` 的"【相关设定】"段（前 10 实体） |
| 7 | `fetch_style_blend` | **风格混合** | `style.style_blend`（scene override 优先 → story active） |
| 8 | `build_outline_context` | 大纲 | `narrative.outline_context` |
| 9 | `build_narrative_structure_context` | **叙事结构** | `narrative.narrative_structure`（act/position/dramatic_function） |
| 10 | `fetch_active_threads` | **活跃线索** | `narrative.active_threads`（未回收伏笔 + 角色弧光） |
| 11 | `compute_style_dna_extension` | **StyleDNA** | `style.style_dna_extension`（预计算文本，blend 优先） |
| 12 | `MemoryOrchestrator::build_memory_pack` | **记忆包** | `memory.memory_pack`（working/episodic/semantic） |

并行计算 `compute_personalizer_extension_async`（Pro 个性化偏好）。

### 3.2 技能系统（5 个内置技能）

**文件**：`skills/builtin.rs`（定义）、`agents/orchestrator.rs:1247-1458`（执行）

| 技能 | 定义 | 执行点 | 触发条件 | 功能 |
|------|------|--------|----------|------|
| `emotion_pacing` | `skills/builtin.rs:186` | `orchestrator.rs:1275` | **始终触发**（链首） | 情感曲线/节奏改写 |
| `style_enhancer` | `skills/builtin.rs:27` | `orchestrator.rs:1302` | **始终触发**（链次） | 文风增强 |
| `character_voice` | `skills/builtin.rs:130` | `orchestrator.rs:1339` | **P1-2**：引号对数≥3（密集对话时） | 角色语音一致性 |
| `plot_twist` | `skills/builtin.rs:58` | `orchestrator.rs:1364` | **P1-2**：dramatic_function 含"转折/高潮/反转" | 情节反转增强 |
| `text_formatter` | `skills/builtin.rs:89` | `orchestrator.rs:1389` | **P1-2**：连续空行≥2 或单行>500 字 | 段落/标点排版 |

**执行路径**：仅 Full 和 Fast 模式的 `apply_writing_skills` 调用。TimeSliced 跳过。

### 3.3 能力注册表与 MCP

**文件**：`capabilities/mod.rs`

| 资产 | 定义 | 注册 | 消费 | 功能 |
|------|------|------|------|------|
| Agent 能力（writer/inspector/outline_planner/style_mimic/plot_analyzer） | `capabilities/mod.rs:351-546` | `init_registry()` 静态 | PlanGenerator `to_llm_context()` + PlanExecutor 派发 | LLM 可选择调用的 Agent |
| 系统命令能力（create_story/chapter/character/update_*/query_kg） | `capabilities/mod.rs:549-941` | `init_registry()` 静态 | 同上 | LLM 可选择调用的系统操作 |
| 内置 MCP 工具（filesystem/text_processing/web_search） | `mcp/server.rs:156-194` | **P0-3**：`lib.rs:643-665` 自动注册进 CapabilityRegistry | PlanGenerator 可选 → PlanExecutor `execute_mcp_tool` | 外部工具调用 |
| 外部 MCP 服务器 | `mcp/client.rs` | `commands/mcp.rs:22-33` 连接时自动注册 | 同上 | 用户连接的外部工具 |

### 3.4 创作方法论（5 种）

**文件**：`creative_engine/methodology/`

> **注意**：`prompts/methodologies/` 旧目录已于 P2-1 删除。方法论仅存在于 `creative_engine/methodology/`。

| 方法论 | 定义文件 | 功能 |
|--------|----------|------|
| 雪花写作法（Snowflake） | `methodology/snowflake.rs` | 10 步自顶向下扩展 |
| 场景结构（SceneStructure） | `methodology/scene_structure.rs` | GCD/RDD 6 节拍 |
| 英雄之旅（HeroJourney） | `methodology/hero_journey.rs` | Campbell 12 阶段英雄弧 |
| 人物深度（CharacterDepth） | `methodology/character_depth.rs` | 6 维人物模型 |
| 高密度世界构建（HighDensityWorldBuilding） | `methodology/high_density_world_building.rs` | 状态驱动/桥接节点/事件回流 |

**注入**：`MethodologyEngine::build_prompt_extension`（`methodology/mod.rs:101`）→ `build_writer_prompt:1961`（**Pro only**）

### 3.5 风格系统

| 资产 | 定义 | 功能 | 注入路径 |
|------|------|------|----------|
| StyleDNA 六维模型 | `creative_engine/style/dna.rs:17` | 词汇/句法/修辞/视角/情感/对话六维量化 | `build_writer_prompt:1975-2024`（P3-1：单 DNA 全用户，混合 Pro-only） |
| 52 种内置风格 | `classic_styles.rs:9` + `classic_styles_extended.rs` | 金庸/张爱玲/海明威…52 位作家/风格 | seed 进 DB，用户可在 story 上选定 |
| StyleBlend 风格混合 | `creative_engine/style/blend.rs:48` | 2-5 种 DNA 按权重融合（主导+辅助） | `build_writer_prompt:1978-2008`（**Pro only**） |
| StyleFingerprint 风格指纹 | `creative_engine/style/fingerprint.rs:18` | 从当前文本实时提取句长/N-gram/锚点 | `build_writer_prompt:2107-2154`（全用户） |
| Living Author Guard | `creative_engine/style/living_author_guard.rs` | 替换在世作者名为工艺滑块指令 | `build_writer_prompt:2266-2282`（全用户） |

### 3.6 网文资产（v0.17 中文叙事增强）

| 资产 | 数据来源 | 注入路径 | 功能 |
|------|----------|----------|------|
| 体裁画像 GenreProfile | `templates/genres.json` seed + 用户可编辑 | 3 处：genesis 策略笔记 / TimeSliced 反模式 / 四元组 emotional_payoff | 题材专家策略（core_tone/pacing/anti_patterns/typical_structure/reader_promise） |
| 桥段卡 BeatCard | `builtin_beat_cards()` 内存（≥30 张） | 四元组序列化（P0-4 完整 payload） | 可复用叙事功能模板（function/when_to_use/remix_hint/avoid） |
| 剧情引擎 StoryEngine | `builtin_story_engines()` 内存（21 种） | 四元组序列化（P0-4 完整 payload） | 正交叙事动力（payoff/best_payoff/avoid/pairs_well_with） |
| 高压关系 PressureRelationship | `builtin_pressure_relationships()` 内存（13 种） | 四元组序列化（P0-4 完整 payload） | 高压关系放大器（pressure_source/works_with） |

> **P2-3**：`beat_cards`/`story_engines`/`pressure_relationships` 三张 DB 表已删除（从未读写），资产仅存内存 `builtin_*()`。

### 3.7 状态资产

| 资产 | 定义 | 读取点 | 功能 |
|------|------|--------|------|
| CanonicalStateManager | `canonical_state/manager.rs:20` | `build_writer_prompt:2167`（Full）+ `CreativeAssetSnapshot`（TimeSliced/Inspector） | 叙事阶段/活跃冲突/待回收伏笔/逾期伏笔/角色状态聚合快照 |
| KnowledgeGraphRepository | `db/repositories.rs` | `context_builder.rs:1060`（实体）+ `canonical_state/manager.rs:318`（关系→冲突） | KG 实体进 scene_structure；KG 关系进活跃冲突 |
| ForeshadowingTracker + PayoffLedger | `creative_engine/foreshadowing.rs` + `payoff_ledger.rs` | `canonical_state/manager.rs:354` → 快照 → prompt | 伏笔 setup/payoff/overdue 追踪 |
| character_states 表 | `canonical_state/manager.rs:169` 读取 | 快照 → `build_writer_prompt:2224` 【角色当前状态】 | 角色当前位置/情绪/目标 |

> **P0-1**：ingest 现在自动将分析出的伏笔写入 `foreshadowing_tracker`（此前断环）。
> **P0-2**：ingest 现在自动将角色状态写入 `character_states`（此前表为空）。

### 3.8 记忆与自适应学习

| 资产 | 定义 | 功能 | 注入路径 |
|------|------|------|----------|
| MemoryOrchestrator | `memory/orchestrator.rs:125` | 三层记忆包（working/episodic/semantic） | `context_builder.rs:470` → `build_writer_prompt:1783`（Full） |
| PromptPersonalizer | `creative_engine/adaptive/personalizer.rs:14` | 从反馈挖掘的偏好生成个性化扩展 | `build_writer_prompt:2073`（**Pro only**） |
| AdaptiveGenerator | `creative_engine/adaptive/generator.rs:54` | 按叙事阶段动态调整 temperature/max_tokens | `service.rs:1136`（全路径，影响 LLM 调用参数） |
| FeedbackRecorder | `creative_engine/adaptive/feedback.rs:65` | 记录用户 accept/reject/modify 决策 | `commands/intent.rs:69`（反馈入口） |
| PreferenceMiner | `creative_engine/adaptive/miner.rs:33` | 从反馈历史挖掘偏好模式 | 喂给 PromptPersonalizer |

> **QueryPipeline**（`memory/query.rs`）五阶段管线是死代码（保留仅为 `SearchResult`/`VectorStore` 类型）。实时记忆走 `MemoryOrchestrator::build_memory_pack`。

### 3.9 提示词模板

> **P2-1**：`PromptManager`、`PromptEvolver` 已删除。

| 模板 | 定义 | 注册 | 实时调用 | 功能 |
|------|------|------|----------|------|
| `writer_system` | `prompts/engine.rs:80` | `registry.rs:56` | `service.rs:1838` | Writer 系统提示 |
| `writer_continue` | `engine.rs:131` | `registry.rs:73` | `service.rs:2294` | 续写用户指令模板 |
| `writer_rewrite` | `engine.rs:154` | `registry.rs:81` | `service.rs:2290` | 改写选中内容模板 |
| `inspector_system` | `engine.rs:170` | `registry.rs:89` | `service.rs:2315` | Inspector 系统提示（P1-3 增强） |
| `outline_planner` | `engine.rs:250` | `registry.rs:105` | `execute_outline_planner` | 大纲生成模板 |
| `commentator_system` | `engine.rs:306` | `registry.rs:113` | **P2-2**：`service.rs:1575` | 评点家模板（改用 registry，支持前端覆盖） |
| `style_checker_system` | `engine.rs:276` | `registry.rs:97` | 注册未调用（实时走规则法） | 风格检查模板（预留） |

---

## 4. 资产 × 流程注入矩阵

图例：✓ = 进入该路径的 LLM prompt；✗ = 不进入；⚠️ = 有条件/部分进入

| 资产类别 | Full 续写 | TimeSliced 续写【默认】 | Plan 生成 | Inspector |
|----------|:---:|:---:|:---:|:---:|
| **Skills**（5 技能链） | ✓ emotion_pacing+style_enhancer 始终；其余 3 个场景触发 | ✗ 跳过 | ⚠️ 仅 capability 列表列出 id | ✗ |
| **MCP 工具** | ✗ | ✗ | ✓ mcp_tools_available 前 5 | ✗ |
| **方法论**（5 种） | ✓ Pro only | ✗ | ⚠️ 仅 id，标"只读上下文" | ✗ |
| **StyleDNA 单一** | ✓ P3-1 全用户 | ⚠️ 经 style_dna_summary（一句话摘要） | ⚠️ 仅 style_dna_info 字符串 | ⚠️ 经 style_dna_extension |
| **StyleBlend 混合** | ✓ Pro only | ✗ | ✗ | ✗ |
| **风格指纹** | ✓ | ✗ | ✗ | ✗ |
| **写作策略 config** | ✓ | ✗ | ✗ | ✗ |
| **体裁画像 GenreProfile** | ✓ 完整注入 | ⚠️ 仅 anti_patterns | ⚠️ 经 strategy（P3-2 自动匹配） | ✗ |
| **桥段卡/引擎/高压关系** | ⚠️ 经四元组（P0-4 完整 payload） | ⚠️ 经四元组（P1-1 接通） | ⚠️ 仅 id 不展开 | ✗ |
| **知识图谱** | ✓ 实体进 scene_structure | ✗ | ✗ | ✗ |
| **规范状态**（阶段/冲突） | ✓ | ⚠️ 经 narrative_phase_guidance（P1-1） | ⚠️ 仅 story_progress 字符串 | ⚠️ 经 CreativeAssetSnapshot（P1-3） |
| **伏笔 / 回收账本** | ✓ pending+overdue | ⚠️ 经 pending/overdue foreshadowings（P1-1） | ⚠️ 仅前 5 content | ⚠️ 经 CreativeAssetSnapshot（P1-3） |
| **角色状态** | ✓（P0-2 写入闭环） | ✗ | ✗ | ✗ |
| **记忆包** | ✓ | ✗ | ✗ | ✗ |
| **用户个性化** | ✓ Pro only | ✗ | ✗ | ✗ |
| **叙事结构**（act/position） | ✓ | ✗ | ✗ | ✗ |
| **前文摘要** | ✓ | ✗ | ✗ | ✗ |
| **世界观规则** | ✓ | ✓（contract_redlines） | ⚠️ 截断 200 字 | ⚠️（P1-3） |

---

## 5. Prompt 装配详解

### 5.1 Full 模式：`build_writer_prompt`（18 段注入）

**文件**：`agents/service.rs:1730-2301`

| # | Section | 来源 | 版本 | 行号 |
|---|---------|------|------|------|
| 1 | `writer_system` 模板 | PromptRegistry | 全部 | `:1838` |
| 2 | 【写作策略约束】（run_mode/conflict/pace/freedom） | `AppConfig.writing_strategy` | 全部 | `:1843-1897` |
| 3 | 【体裁画像策略】（core_tone/pacing/anti_patterns/typical_structure） | `genre_profile_id` → GenreProfileRepository | 全部 | `:1900-1936` |
| 4 | 【创作方法论约束】（5 种方法论） | `methodology_id` → MethodologyEngine | **Pro only** | `:1939-1969` |
| 5 | StyleDNA / StyleBlend | `style_dna_extension` / `style_blend` / `style_dna_id` | **P3-1**：单 DNA 全用户，混合 Pro | `:1971-2024` |
| 6 | 【写作风格约束】（name/desc/vocab/sentence/rules） | `writing_style_*` | **P3-1** 全用户 | `:2027-2058` |
| 7 | 【作品简介】 | `story.description` | **P3-1** 全用户 | `:2061-2068` |
| 8 | 【个性化偏好】 | `personalizer_extension`（PromptPersonalizer） | **Pro only** | `:2073-2105` |
| 9 | 【风格指纹】（句长/N-gram/锚点） | `StyleFingerprint::from_text` | 全部 | `:2107-2154` |
| 10 | 【叙事阶段指导】 | `CanonicalStateManager.get_snapshot_sync` | 全部 | `:2157-2249` |
| 11 | 【当前活跃冲突】 | 同上 | 全部 | 同上 |
| 12 | 【待回收伏笔】 | 同上 | 全部 | 同上 |
| 13 | 【⚠️ 逾期伏笔】 | 同上 | 全部 | 同上 |
| 14 | 【角色当前状态】 | 同上（P0-2 写入闭环） | 全部 | 同上 |
| 15 | 【中文叙事四件套】 | `task.parameters["narrative_quartet"]` → `render_narrative_quartet_section` | 全部 | `:2253-2261` |
| 16 | Living Author Guard | `sanitize_style_brief` | 全部 | `:2266-2282` |
| 17 | 用户指令（`writer_continue` 或 `writer_rewrite`） | PromptRegistry | 全部 | `:2284-2296` |

### 5.2 TimeSliced 模式：`WriteTimeBundle::to_prompt`（P1-1 增强后 10 段）

**文件**：`creative_engine/write_time_bundle.rs:283+`

| # | Section | 来源 | 行号 |
|---|---------|------|------|
| 1 | 【合同红线】（MASTER_SETTING，截断 800 字） | `StoryContractRepository` | `:300+` |
| 2 | 【角色核心】（姓名 + 状态 + 性格） | `CharacterRepository` | `:320+` |
| 3 | 【场景大纲】（dramatic_goal/conflict_type/setting） | `SceneRepository` | `:340+` |
| 4 | 【体裁反模式】（anti_patterns） | `GenreProfileRepository` | `:360+` |
| 5 | 【风格指引】（题材自适应，部分题材为空） | override | `:370+` |
| 6 | 【叙事阶段】（P1-1） | `CreativeAssetSnapshot` | `:371` |
| 7 | 【待回收伏笔】top 3（P1-1） | `CreativeAssetSnapshot` | `:376` |
| 8 | 【⚠️ 逾期伏笔】top 1（P1-1） | `CreativeAssetSnapshot` | `:389` |
| 9 | 【主导风格】一句话摘要（P1-1） | `CreativeAssetSnapshot` | `:402` |
| 10 | 【中文叙事四件套】（P1-1） | `task.parameters` → `render_narrative_quartet_section` | `:407` |

### 5.3 Inspector Prompt：`build_inspector_prompt`（P1-3 增强）

**文件**：`agents/service.rs:2303-2386`

| Section | 来源 | 功能 |
|---------|------|------|
| `inspector_system` 模板 | PromptRegistry | 基础系统提示（title/genre/characters/content） |
| 【世界观规则】（P1-3） | `ctx.world.world_rules` | 检查内容是否违反设定 |
| 【待回收伏笔】top 3（P1-3） | `CreativeAssetSnapshot` | 检查本章是否推进/回收 |
| 【逾期伏笔】top 2（P1-3） | `CreativeAssetSnapshot` | 重点检查是否已回收 |
| 【当前叙事阶段】（P1-3） | `CreativeAssetSnapshot` | 阶段一致性检查 |
| 【目标风格约束】（P1-3） | `ctx.style.style_dna_extension` | 风格一致性检查 |

### 5.4 统一资产加载网关：`CreativeAssetSnapshot`（P3-3）

**文件**：`creative_engine/asset_snapshot.rs:19-106`

**功能**：消除 Full 与 TimeSliced 路径的重复资产加载逻辑，提供统一加载器。

**加载内容**：
1. `CanonicalStateManager::get_snapshot_sync`（叙事阶段 + 冲突 + 伏笔 + 角色状态）
2. `StyleDnaRepository::get_by_id` → 风格 DNA 一句话摘要

**消费者**：
- `WriteTimeBundle::load_sync`（TimeSliced 路径）
- `build_inspector_prompt`（Inspector 路径）

---

## 6. 写后闭环：记忆与采摘

生成成功后（所有模式），`AgentOrchestrator::generate`（`:353-482`）异步执行：

### 6.1 MemoryWriter::write_with_cancel

**文件**：`memory/writer.rs:80-152`

- 提取内容摘要（前 200 字）
- 更新 `scene_commits.summary_text`
- 创建 `memory_items` 工作记忆条目

### 6.2 IngestPipeline::ingest_with_cancel（P0-1 + P0-2）

**文件**：`memory/ingest.rs:262-310`

| 步骤 | 功能 | P0 修复 |
|------|------|---------|
| Step 1: `analyze_content` | LLM 分析实体/关系/情感/伏笔/主题 | — |
| Step 1b: `persist_foreshadowings` | 将 setup 型伏笔写入 `foreshadowing_tracker`（去重） | **P0-1** 接通断环 |
| Step 1c: `persist_character_states` | 将角色 location/emotion/goal 写入 `character_states`（保留既有 secrets） | **P0-2** 接通断环 |
| Step 2: `generate_knowledge` | 生成结构化知识档案 | — |
| Step 3: `save_narrative_events` | 叙事事件链持久化 | — |
| Step 4: `convert_entities`/`link_entities`/`convert_relations` | KG 实体/关系落库 | — |

**闭环意义**：P0-1/P0-2 写入的数据，在下一次续写时被 `CanonicalStateManager::get_snapshot_sync` 读取，注入 `build_writer_prompt` 的【角色当前状态】和【待回收/逾期伏笔】段——形成"写→记→读→写"的完整闭环。

---

## 7. 审计与质量反馈

### 7.1 同步 Inspector（Full 模式）

**文件**：`agents/orchestrator.rs:908-1199`

- Inspector 用 `build_inspector_prompt`（P1-3 增强版）检查内容
- 7 维评分（style/narrative/memory/character/foreshadow/pacing/continuity）
- 不达标触发 Rewrite 闭环
- **这是唯一驱动自动改写的路径**

### 7.2 异步 AuditExecutor（TimeSliced 模式）

**文件**：`task_system/audit_executor.rs:97`

- 11 维审计（logic/character/continuity/foreshadow/pacing/style/memory/desire/payoff/aftertaste/opening_clarity）
- 结果写 `TextAnnotationRepository` + emit `AnnotationCreated` 事件
- **P1-4**：发现 high 严重性问题时，发射 `AuditRewriteSuggested` SyncEvent，前端可提示用户是否修订（保持用户控制权）
- **fire-and-forget**：不阻塞生成返回，不自动改写

### 7.3 AuditService（规则法，手动触发）

**文件**：`audit/mod.rs:56`

- 5 维评分（continuity/character/style/pacing/payoff）
- 仅由 Tauri 命令手动触发，不在创作主流程内
- 标注 `#![allow(dead_code)]`

---

## 8. Pro/Free 资产分层

**P3-1 重构后的分层**：

| 资产 | Free | Pro |
|------|------|-----|
| 写作策略约束 | ✓ | ✓ |
| 体裁画像 | ✓ | ✓ |
| 单一 StyleDNA | **✓**（P3-1 移出 is_pro） | ✓ |
| 风格指纹 | ✓ | ✓ |
| 写作风格详细设定 | **✓**（P3-1 移出 is_pro） | ✓ |
| 作品简介 | **✓**（P3-1 移出 is_pro） | ✓ |
| 规范状态（阶段/冲突/伏笔/角色） | ✓ | ✓ |
| 叙事四元组 | ✓ | ✓ |
| Living Author Guard | ✓ | ✓ |
| 创作方法论（5 种） | ✗ | ✓ |
| StyleBlend 风格混合 | ✗ | ✓ |
| 个性化偏好 | ✗ | ✓ |
| Skills（5 技能链） | ✗（仅 Full/Fast） | ✓ |

---

## 9. P0-P3 优化变更索引

| 编号 | 变更 | 文件 | 核心改动 |
|------|------|------|----------|
| P0-1 | ingest→伏笔闭环 | `memory/ingest.rs:272,926` | `persist_foreshadowings` 自动登记伏笔 |
| P0-2 | character_states 写入 | `memory/ingest.rs:279,1005` | `persist_character_states` 自动写角色状态 |
| P0-3 | 内置 MCP 自动注册 | `lib.rs:643` | `try_lock` 读取 `BUILTIN_MCP_SERVER` 工具并注册 |
| P0-4 | 四元组完整展开 | `quartet_inference.rs:303` + `service.rs:2962` | 序列化增加 category/when_to_use/pairs_well_with/works_with |
| P1-1 | TimeSliced 接入精选资产 | `write_time_bundle.rs:49` + `orchestrator.rs:681` | 新增 5 字段 + 四元组注入 |
| P1-2 | 接通 3 个休眠技能 | `orchestrator.rs:1324` | 场景智能触发 character_voice/plot_twist/text_formatter |
| P1-3 | Inspector 全量上下文 | `service.rs:2318` | 注入世界观/伏笔/叙事阶段/风格 |
| P1-4 | 审计触发 Rewrite 建议 | `events.rs:171` + `audit_executor.rs:181` | `AuditRewriteSuggested` SyncEvent |
| P2-1 | 删除死代码 | 删除 `prompts/methodologies/`、`evolution/`、`state/`、`PromptManager`、`PromptEvolver` | 5 目录/文件 + 4 个 mod 声明 |
| P2-2 | 消除 prompt 旁路 | `service.rs:1575` | commentator 改用 registry 模板 |
| P2-3 | 删除死表 | `connection.rs` Migration 94 | DROP beat_cards/story_engines/pressure_relationships |
| P3-1 | Pro/Free 精细化分层 | `service.rs:1971-2068` | 单 StyleDNA + 写作风格 + 作品简介移出 is_pro |
| P3-2 | 资产自动匹配 | `orchestrator.rs:904` | 按 genre 自动匹配 GenreProfile |
| P3-3 | 统一资产注入网关 | `creative_engine/asset_snapshot.rs`（新） | `CreativeAssetSnapshot` 消除重复加载 |

---

## 快速查阅索引

**我要理解续写流程** → §2.1-2.8
**我要知道某资产是否进 prompt** → §4 注入矩阵
**我要看 Full 模式 prompt 有哪些段** → §5.1
**我要看 TimeSliced 模式 prompt 有哪些段** → §5.2
**我要理解伏笔/角色状态的闭环** → §6.2 + §3.7
**我要理解四元组怎么来的** → §2.3-2.4 + §3.6
**我要理解技能何时触发** → §3.2
**我要理解 Pro/Free 差异** → §8
**我要查找某项 P0-P3 改动** → §9

---

*本文档基于 v0.17.x 代码库快照（2026-06-20），由 5 路并行 Agent 全量追踪 + 人工综合生成。创作流程或资产架构有变动时，同步更新本文。*
