# StoryMoss 后台创作资产清单 × 智能创作流程断链审计（v0.22.4）

> **文档定位**：本文是 v0.22.4 针对「后台创作资产是否被智能创作流程充分利用」的专项审计。包含：
> 1. 后台全部创作资产清单与功能说明
> 2. 智能创作全流程与提示词注入点梳理
> 3. 资产-流程连接矩阵
> 4. 发现的断链/断环/不一致问题
> 5. 建设性修复建议（按优先级排序）
>
> **代码基准**：v0.22.4，2026-06-21
> **维护规则**：资产架构或创作流程有变动时，同步更新本文

---

## 目录

1. [后台创作资产清单](#1-后台创作资产清单)
2. [智能创作全流程梳理](#2-智能创作全流程梳理)
3. [资产 × 流程连接矩阵](#3-资产--流程连接矩阵)
4. [发现的断链与问题](#4-发现的断链与问题)
5. [建设性修复建议](#5-建设性修复建议)
6. [验证结果](#6-验证结果)

---

## 1. 后台创作资产清单

### 1.1 核心创作资产（代码级 + 数据库持久化）

| # | 资产类型 | 数量 | 模块/文件 | 数据库表 | 功能作用 | 状态 |
|---|---|---|---|---|---|---|
| 1 | **GenreProfile 体裁画像** | 43 | `templates/genres.json` 种子；`src/db/repositories_story_system.rs`；`src/strategy/genre_resolver.rs`；`src/strategy/asset_catalog.rs` | `genre_profiles` | 编码题材核心基调、节奏策略、反套路、参考表、典型结构、读者承诺、推荐风格/方法论/技能；是策略选择的第一入口 | Active |
| 2 | **StyleDNA 风格 DNA** | 52+ | `src/creative_engine/style/dna.rs`；`classic_styles.rs`；`classic_styles_extended.rs` | `style_dnas` | 六维量化写作风格（词汇/句法/修辞/视角/情感/对话）；约束 Writer 输出、支持风格混合与漂移检测 | Active |
| 3 | **Methodology 创作方法论** | 5 | `src/creative_engine/methodology/mod.rs`；`snowflake.rs`；`scene_structure.rs`；`hero_journey.rs`；`character_depth.rs`；`high_density_world_building.rs` | 无独立表，存于 `stories.methodology_id` + `methodology_step` | 将经典创作方法论编码为 system prompt 扩展；雪花法/场景结构/英雄之旅/人物深度/高密度世界构建 | Active |
| 4 | **Skill 技能** | 5 内置 + 用户自定义 | `src/skills/mod.rs`；`builtin.rs`；`executor.rs`；`registry.rs` | 无 DB 表，文件系统 + 运行时 | 可复用创作能力（文风增强/情节反转/文本排版/角色声音/情感节奏）；支持 Hook 生命周期 | Active |
| 5 | **BeatCard 桥段卡** | 31 | `src/creative_engine/beat_cards/mod.rs`；`registry.rs` | ~~`beat_cards`~~ 已于 Migration 94 删除 | 可复用叙事功能模板（跌落归来、弱者挑战、规则破解等），用于大纲/正文骨架 | Active（仅内存） |
| 6 | **StoryEngine 剧情引擎** | 21 | `src/creative_engine/story_engines/mod.rs` | ~~`story_engines`~~ 已删除 | 正交叙事动力（隐藏身份、重生回溯、契约绑定等），建议 2-4 个组合使用 | Active（仅内存） |
| 7 | **PressureRelationship 高压关系** | 13 | `src/creative_engine/pressure_relationships/mod.rs` | ~~`pressure_relationships`~~ 已删除 | 角色对位冲突放大器（真假继承人、师徒宗门、替身白月光等） | Active（仅内存） |
| 8 | **Workflow 工作流模板** | 若干 | `src/creative_engine/workflow/`；`src/workflow/` | `workflow_instances` | 可视化 DAG 创作流程节点与条件边 | Active 但**未在创作流程中执行** |

### 1.2 故事级数据资产（数据库表）

| # | 资产类型 | 数据库表 | 功能作用 |
|---|---|---|---|
| 9 | 世界观 | `world_buildings`, `world_rules`, `settings` | 故事级概念、规则、历史、文化、地点感官细节 |
| 10 | 角色 | `characters`, `character_relationships`, `character_states` | 角色档案、关系图谱、动态状态跟踪 |
| 11 | 伏笔 | `foreshadowing_tracker` |  setup/payoff/abandoned 状态，支持时间窗口与风险信号 |
| 12 | 故事大纲 | `story_outlines` | 三幕结构、情节点、预估场景数 |
| 13 | 知识图谱 | `kg_entities`, `kg_relations` | 从文本提取的实体关系图，支持向量嵌入与遗忘曲线 |
| 14 | 记忆 | `memory_items` | 长期压缩记忆片段 |
| 15 | 场景版本 | `scene_versions` | 版本链、diff、恢复 |
| 16 | 叙事分析 | `narrative_events`, `narrative_threads`, `narrative_structure`, `narrative_chunks` | LitSeg 叙事分析层 |
| 17 | 拆书参考 | `reference_books`, `reference_characters`, `reference_scenes` | 拆书分析后的参考素材 |

### 1.3 系统级资产

| # | 资产类型 | 模块/表 | 功能作用 |
|---|---|---|---|
| 18 | **PromptRegistry 提示词注册表** | `src/prompts/registry.rs`；`prompt_overrides` 表 | 79 个可覆盖提示词模板，覆盖 Writer/Inspector/Methodology/Creation/Intent/Audit 等 21 类 |
| 19 | **Model Capability Profile 模型能力画像** | `src/model_gateway/`；`model_capability_profile` 表 |  per-model TTFB/TPS/成功率/能力分，驱动网关路由 |
| 20 | **Intention Graph 意图图（SING）** | `src/intention_graph/`；`intention_nodes`, `asset_nodes`, `intention_asset_edges`, `asset_asset_edges`, `execution_graphs`, `execution_nodes` | 意图-资产异构图；支持 PPR 分层发现、动态 ReAct、执行计划 |
| 21 | **CapabilityRegistry 能力注册表** | `src/capabilities/` | Agent/Skill/MCP/系统命令的自描述能力目录 |
| 22 | **Agent 智能体** | `src/agents/` | Writer / Inspector / OutlinePlanner / StyleMimic / PlotAnalyzer / MemoryCompressor / Commentator / KnowledgeDistiller |

---

## 2. 智能创作全流程梳理

### 2.1 端到端主路径

```
用户输入（幕前）
    │
    ▼
smart_execute (commands/orchestrator.rs:31)
    │
    ├─ 创世分支 → GenesisPipeline (narrative/genesis.rs)
    │   ├─ ConceptGenerationStep      → narrative_story_concept_generate
    │   ├─ StrategySelectionStep      → StrategySelector
    │   ├─ FirstChapterGenerationStep → AgentOrchestrator::Full
    │   ├─ ParallelWorldOutlineCharacterStep
    │   │   ├─ world_building_prompt   → narrative_world_building_generate
    │   │   ├─ outline_prompt          → narrative_outline_generate
    │   │   └─ character_prompt        → narrative_character_generate
    │   ├─ SceneGenerationStep        → scene_prompt / narrative_scene_generate
    │   ├─ ForeshadowingGenerationStep→ foreshadowing_prompt
    │   └─ KnowledgeGraphGenerationStep
    │
    └─ 续写/编辑分支
        ├─ 加载轻量摘要（story/chapters/scenes/world/chars/foreshadowing/style/mcp）
        ├─ build_selected_strategy (commands/orchestrator.rs:897)
        ├─ infer_narrative_quartet (strategy/quartet_inference.rs:15)
        │
        ▼
    PlanContext → PlanGenerator / IntentionGraphPlanner
        │
        ▼
    ExecutionPlan → PlanExecutor
        │
        ├─ execute_writer → AgentOrchestrator::generate
        │   ├─ Full 模式：StoryContextBuilder + build_writer_prompt → Inspector → Rewrite
        │   └─ TimeSliced 模式：WriteTimeBundle → 单轮 LLM → 后台 Audit
        ├─ execute_inspector / execute_skill / execute_mcp_tool
        │
        ▼
    写后闭环：MemoryWriter + IngestPipeline（KG/伏笔/角色状态）
```

### 2.2 提示词注入关键节点

| 节点 | 文件 | 注入资产 |
|---|---|---|
| `build_writer_prompt` | `agents/service.rs:1751` | genre_profile、methodology、style_dna、canonical state、foreshadowing、narrative_quartet、redlines |
| `build_inspector_prompt` | `agents/service.rs:2324` | 世界观规则、体裁画像、方法论、角色状态、活跃冲突、叙事四元组（**存在重复注入块**） |
| `WriteTimeBundle::to_prompt` | `creative_engine/write_time_bundle.rs` | genre_profile、methodology、style_dna、narrative_quartet、secondary_genre_profile（v0.22.4 新增） |
| `narrative/prompts.rs` 创世 prompts | `narrative/prompts.rs` | 仅 story_title/genre/简介等基础字段 |
| `strategy_selector` prompt | `strategy/selector.rs:207` | 全部 `available_assets` payload |

### 2.3 模型网关流程

1. `AgentService::generate_for_request_with_request_id` 构建 `GatewayRequest`
2. 透传 `intent_verb` / `intent_object` / `asset_tags` / `discovered_asset_ids`
3. `TaskClassifier::classify_task` 按意图 + asset_tags 分类任务（LightTool/BalancedWork/HeavyCreation）
4. `GatewayExecutor::select_candidates` 三维打分（算力 50% + 偏好 30% + 适配 20%）+ asset_tags 重叠加分 + CapabilityProfile 质量/速度加权
5. 顺序执行 fallback

---

## 3. 资产 × 流程连接矩阵

### 3.1 已连接 ✅

| 资产 | 使用路径 | 说明 |
|---|---|---|
| GenreProfile | `build_selected_strategy` → `build_writer_prompt` / `WriteTimeBundle` | v0.22.4 新增 GenreResolver，复合题材可解析为多画像；主/次画像策略均注入 |
| StyleDNA | `build_writer_prompt` / `WriteTimeBundle` | 风格摘要注入，支持混合风格 |
| Methodology | `build_writer_prompt` / `WriteTimeBundle` / `build_inspector_prompt` | 按 methodology_id + step 动态选择 prompt，但 HDWB 阶段映射有 bug |
| Narrative Quartet（四元组） | `infer_narrative_quartet` → Writer/Inspector prompt | 仅 Vague/WithSeed 输入触发；高压关系/剧情引擎/桥段卡内容会渲染 |
| Foreshadowing | `StoryContextBuilder` / `WriteTimeBundle` | 待回收/逾期伏笔注入 Writer prompt |
| Character States | `StoryContextBuilder` / `asset_snapshot` | 角色位置/情绪/状态注入 |
| Knowledge Graph | `StoryContextBuilder::build_memory_pack` | 查询时作为上下文，写后更新 |
| Model Capability Profile | `GatewayExecutor::select_candidates` | TTFB/TPS/成功率参与候选排序 |
| PromptRegistry | 全 LLM 调用入口 | 79 个提示词全部可覆盖 |

### 3.2 连接不完整 ⚠️

| 资产 | 问题 |
|---|---|
| **Workflow** | 已定义、可选中，但 `PlanExecutor` / `AgentOrchestrator` 完全不执行 `workflow_id` |
| **Skill IDs（来自 strategy）** | `SelectedStrategy.skill_ids` 已生成，但未自动进入 `AgentTask.parameters` 或 PlanStep |
| **意图图发现的非执行型资产** | Methodology/StyleDna/BeatCard/StoryEngine/PressureRelation 仅作为 `asset_tags`/`discovered_asset_ids` 传递，未在 `build_writer_prompt` 中加载实际内容 |
| **模型网关 asset_tags 校准** | 只识别 `mcp_tool`/`system_command`/`genre_profile`/`creative_writing`，未利用 `methodology`/`style_dna`/`beat_card`/`story_engine`/`pressure_relationship` 等标签 |

### 3.3 系统性脱节 ❌（Genesis 创世 prompts）

| Prompt ID | 应收资产 | 实际注入 |
|---|---|---|
| `narrative_world_building_generate` | 体裁画像策略、HDWB 方法论、高压关系/引擎（世界观冲突源） | 仅 title/genre/description |
| `narrative_outline_generate` | 体裁画像、方法论、桥段卡、剧情引擎、高压关系 | 仅 title/genre/world_summary |
| `narrative_character_generate` | 体裁画像、人物深度方法论、高压关系、已有世界观 | 仅 title/genre/world_concept/description |
| `narrative_scene_generate` | 场景结构方法论、桥段卡、剧情引擎、高压关系、体裁画像 | 仅 title/genre/characters/outline_summary |
| `narrative_foreshadowing_generate` | 剧情引擎、桥段卡（埋设方向） | 仅 title/genre/outline_summary |

> **核心结论**：Genesis 后台阶段（世界观/大纲/角色/场景/伏笔）的生成 prompt 与 StrategySelector 选出的方法论、体裁画像、中文叙事四元组等资产**基本脱节**。`build_strategy_notes` 只在第一章生成时使用，后台并行步骤未复用。

---

## 4. 发现的断链与问题

### 4.1 P0/P1 高优先级问题

#### 🔴 P1-1：Genesis 创世 prompts 与后台资产脱节
- **位置**：`narrative/prompts.rs:45, 135, 204, 295, 382, 452`；`narrative/genesis.rs:769, 885, 996, ...`
- **现象**：`world_building_prompt` / `outline_prompt` / `character_prompt` / `scene_prompt` / `foreshadowing_prompt` 仅接收 title/genre/简介等字符串，未接收 `SelectedStrategy` 或 `build_strategy_notes`。
- **影响**：世界观生成不会调用「高密度世界构建法」；角色生成不会调用「人物深度模型」；大纲生成不会调用「雪花法/英雄之旅」；场景生成不会调用「场景结构规范」。体裁画像的 core_tone/pacing/anti_patterns/typical_structure 也未注入。
- **用户原话对应**："世界观创作并未关联引用后台资产世界观密集创作法"

#### 🔴 P1-2：高密度世界构建法（HDWB）阶段映射错误
- **位置**：`creative_engine/write_time_bundle.rs:307-310`；`agents/service.rs:2475`
- **现象**：无论 `methodology_step` 是 1/2/3/4，HDWB 都固定使用 `methodology_hdwb_seed`。
- **影响**：后续阶段的世界观续写/扩展无法获得「状态网扩张」「多线交织」「密度迭代」的对应约束。
- **连带问题**：注册表中 `methodology_hdwb_expansion` / `convergence` / `iteration` 是孤儿 key，代码从未 resolve。

#### 🟡 P1-3：`build_strategy_notes` 未注入四元组与方法论内容
- **位置**：`narrative/genesis.rs:124-186`
- **现象**：`build_strategy_notes` 确实加载了 GenreProfile 完整字段，但只打印 `methodology_id` 字符串，未展开方法论内容；未注入 `narrative_quartet`；未注入 StyleDNA 摘要。
- **影响**：第一章生成时策略注解是"半盲"的。

### 4.2 P2 中优先级问题

#### 🟡 P2-1：Inspector prompt 存在重复注入块
- **位置**：`agents/service.rs:2397-2503` 与 `2505-2561+`
- **现象**：
  - 体裁画像块出现两次；第二次多了 anti_patterns，但逻辑重复。
  - 方法论块出现两次；**第二次硬编码为 `methodology_snowflake_stepN`**，若用户选了 hero_journey/scene_structure/character_depth/hdwb 会注入错误的方法论。
  - 角色状态/活跃冲突块出现两次。
- **影响**：Inspector prompt 冗余，且方法论检查可能完全不匹配当前故事设定。

#### 🟡 P2-2：孤儿 methodology / memory 提示词
- **位置**：`prompts/registry.rs:2327-2393` 及 `methodology_character_analysis` / `methodology_scene_self_check` / `memory_knowledge_generation`
- **现象**：这些 key 在注册表中可覆盖，但代码没有任何 `resolve_prompt` 调用。
- **影响**：前端「提示词」面板显示可编辑，实际不生效，误导用户。

#### 🟡 P2-3：Pipeline review/refine 未注入创作资产
- **位置**：`pipeline/review.rs:190`；`pipeline/refine.rs:181`
- **现象**：审稿/修稿 prompt 仅注入 blueprint、角色列表、review_dimensions / review_feedback，未注入 genre_profile / style_dna / methodology / narrative_quartet。
- **影响**：审稿缺乏题材/风格/方法论对照标准。

#### 🟡 P2-4：Workflow 资产完全未被执行
- **位置**：`strategy/asset_catalog.rs:184`；`strategy/models.rs:142`
- **现象**：`AssetKind::Workflow`、`workflow_assets()`、`SelectedStrategy.workflow_id` 均已定义，但 `PlanExecutor` / `AgentOrchestrator` 未消费。
- **影响**：Workflow 系统沦为配置，无法驱动实际创作流程。

#### 🟡 P2-5：Strategy 推荐的 skill_ids 未自动执行
- **位置**：`strategy/models.rs:140`
- **现象**：`SelectedStrategy.skill_ids` 在 `build_selected_strategy` 中可填充，但未转换为 PlanStep 或 AgentTask 参数。
- **影响**：体裁画像推荐的技能没有自动触发。

### 4.3 P3 低优先级/观察项

#### 🟢 P3-1：模型网关资产感知较浅
- `TaskClassifier::adjust_by_asset_tags` 只覆盖 4 个标签；可扩展 methodology/style_dna/beat_card/story_engine/pressure_relationship/skill 等标签的校准。
- `GatewayExecutor` 的 tag 重叠加分是简单计数，未根据资产内容（如 StyleDNA 复杂度、Methodology 步骤数）调整 prompt 压缩或路由策略。

#### 🟢 P3-2：多助手聊天 prompts 未注入资产
- `agent_world_building` / `agent_character` / `agent_scene` / `agent_plot` 等仅通用 system prompt，未注入当前故事的 genre_profile / methodology / style_dna 摘要。

#### 🟢 P3-3：`narrative_story_concept_generate` 未利用体裁画像详细字段
- 虽已传入 `genre_profiles` ID 列表，但未传 core_tone / pacing 摘要，模型选择时缺少上下文。

#### 🟢 P3-4：测试失败（已定位）
- `creative_engine::methodology::tests::test_build_prompt_extension_scene_structure` 失败：注册表 `methodology_scene_structure` 与代码 fallback 内容不一致（注册表版本不含"目标场景"）。
- `strategy::asset_catalog::tests::test_load_all_assets_integration` 失败：`create_test_pool()` 未运行迁移，缺少 `genre_profiles` 表（已知 V092 基线问题）。

---

## 5. 建设性修复建议

### 5.1 P1 高优先级建议

#### S1：重构 Genesis 生成 prompts，统一接收 `SelectedStrategy`

修改 `narrative/prompts.rs` 中各 prompt builder 的签名：

```rust
// 改造前
pub fn world_building_prompt(mode: PromptMode, story_title: &str, genre: &str, context: &str) -> String;

// 改造后
pub fn world_building_prompt(
    mode: PromptMode,
    story_title: &str,
    genre: &str,
    context: &str,
    strategy: Option<&SelectedStrategy>,        // 新增
    strategy_notes: Option<&str>,               // 新增：genre_profile/methodology/style_dna 摘要
    quartet: Option<&str>,                      // 新增：narrative_quartet JSON 渲染
) -> String;
```

在 `GenesisPipeline` 的各步骤中：
1. 复用 `build_strategy_notes(ctx, genre)` 生成策略摘要；
2. 若 `methodology_id == "high_density_world_building"`，追加 `MethodologyEngine` 当前 phase 的完整 system prompt；
3. 调用 `serialize_quartet_for_prompt(strategy)` 生成四元组渲染文本；
4. 将以上三者注入对应生成 prompt 的 context section。

#### S2：修复 HDWB 阶段映射

在 `write_time_bundle.rs` 和 `agents/service.rs` 中统一方法论 prompt 选择逻辑：

```rust
"high_density_world_building" => {
    let phase = step.unwrap_or(1).clamp(1, 4);
    (format!("methodology_hdwb_{}", phase_name(phase)), "高密度世界构建".to_string())
}
```

其中 `phase_name(1)=seed, 2=expansion, 3=convergence, 4=iteration`。

同时确保 `MethodologyEngine::build_prompt_extension` 对 HDWB 也按 `WorldBuildingPhase` 返回对应 phase 内容（当前已支持，但 WriteTimeBundle 未使用）。

**建议进一步**：在 WriteTimeBundle 中，methodology_extension 优先走 `MethodologyEngine::build_prompt_extension`，而不是直接 resolve registry prompt，以保证所有方法论（包括未来新增）一致。

#### S3：增强 `build_strategy_notes`

将 `build_strategy_notes` 改造为 `build_genesis_strategy_context`：
- 注入 GenreProfile 完整字段（core_tone / pacing / anti_patterns / reference_tables / typical_structure / reader_promise）；
- 注入当前 methodology 的完整 system prompt（不是仅 ID）；
- 注入 StyleDNA 摘要；
- 注入 narrative_quartet（高压关系/剧情引擎/桥段卡/冲突场/主情绪）。

### 5.2 P2 中优先级建议

#### S4：清理 Inspector prompt 重复块

将 `build_inspector_prompt` 中第二次重复的体裁画像/方法论/角色状态/活跃冲突块删除，保留第一次（内容更完整的版本）。同时修复第二次方法论块硬编码雪花法的问题：统一使用第一次的映射表。

#### S5：处理孤儿提示词

对以下 key 做"接入或移除"决策：

| Key | 建议 |
|---|---|
| `methodology_hdwb_expansion` / `convergence` / `iteration` | **接入**：S2 修复后自然使用 |
| `methodology_character_analysis` | **接入** `character_prompt` 或 CharacterDepthModel 输出格式标注 |
| `methodology_scene_self_check` | **接入** scene prompt 输出格式要求 |
| `memory_knowledge_generation` | **接入** 知识图谱生成流程，或从注册表移除 |

#### S6：Pipeline review/refine 资产注入

在 `pipeline/review.rs` 和 `pipeline/refine.rs` 中：
- 通过 `CreativeAssetSnapshot` 或 `WriteTimeBundle` 加载 genre_profile / style_dna / methodology / narrative_quartet；
- 作为审稿维度上下文注入，使审稿能对照题材/风格/方法论标准。

#### S7：执行 `SelectedStrategy.workflow_id`

在 `PlanExecutor::execute_plan` 或 `AgentOrchestrator::generate` 中：
- 若 `plan_context.selected_strategy.workflow_id` 非空，加载对应 Workflow DAG；
- 将 DAG 展开为 `ExecutionPlan` 步骤替代或补充 PlanGenerator 输出。

#### S8：自动执行 Strategy 推荐的 skills

在 `smart_execute` 或 `PlanExecutor` 中：
- 将 `selected_strategy.skill_ids` 追加到 PlanStep 列表；
- 在 Writer 前后自动调用相关 skill（如 `character_voice` 在生成后检查对话一致性）。

### 5.3 P3 低优先级建议

#### S9：深化模型网关资产感知

扩展 `TaskClassifier::adjust_by_asset_tags`：
- `methodology` / `style_dna` → HeavyCreation（方法论/风格转换需要质量）
- `beat_card` / `story_engine` / `pressure_relationship` → BalancedWork 或 HeavyCreation
- `skill` → 按 skill category 映射

扩展 `GatewayExecutor` 评分：对携带复杂方法论/多风格 DNA 的请求，提升质量分权重。

#### S10：多助手聊天注入资产上下文

修改 `memory/multi_agent.rs` 中各 agent 的 system prompt 渲染，追加当前故事的 `genre_profile` / `methodology` / `style_dna` 摘要。

---

## 6. 验证结果

### 6.1 当前编译与测试状态

| 检查项 | 结果 |
|---|---|
| `cargo check` | ✅ 零错误（58 warnings 为既有） |
| `npx tsc --noEmit` | ✅ 零错误 |
| `cargo +nightly fmt -- --check` | ✅ 零差异 |
| `cargo test --lib` 全量 | 439 passed / 49 failed（49 failed 为已知 V092 基线） |

### 6.2 本次审计新增/确认的失败用例

| 测试 | 状态 | 根因 | 建议 |
|---|---|---|---|
| `strategy::asset_catalog::tests::test_load_all_assets_integration` | ❌ | `create_test_pool()` 未运行迁移，缺 `genre_profiles` 表 | 纳入 V092 基线统一修复 |
| `creative_engine::methodology::tests::test_build_prompt_extension_scene_structure` | ❌ | 注册表 `methodology_scene_structure` 与代码 fallback 内容不一致（缺"目标场景"） | 同步注册表内容与代码 fallback，或调整测试断言 |

### 6.3 本次审计未修改代码

所有结论均来自代码静态分析、多 agent 并行探索与 targeted test 验证。如需进入修复阶段，建议按 S1-S10 优先级分版本实施。

---

*最后更新: 2026-06-21 - v0.22.4 资产审计*
